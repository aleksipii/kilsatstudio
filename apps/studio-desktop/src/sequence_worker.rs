//! One bounded background lane for project preparation, preview and export.
use motion_audio::{AudioBuffer, WaveformPeak};
use motion_export::{ArtContext, Report, SequencePreview, SequencePreviewFrame};
use motion_lipsync::cache::{AlignmentCache, AlignmentInput, AlignmentStatus};
use motion_scene::SequenceTimeline;
use motion_script::Script;
use motion_tts::{CancellationToken, KokoroConfig};
use motion_voice::export::RenderedScene;
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
};

pub struct Prepared {
    pub script: Arc<Script>,
    pub rendered: Arc<Vec<RenderedScene>>,
    pub art: Arc<ArtContext>,
    pub preview: SequencePreview,
    pub audio: Arc<AudioBuffer>,
    pub timeline: SequenceTimeline,
    pub peaks: Vec<WaveformPeak>,
    pub alignment_statuses: BTreeMap<u64, AlignmentStatus>,
}
pub struct PrepareRequest {
    pub generation: u64,
    pub script: Script,
    pub root: PathBuf,
    pub config: KokoroConfig,
    pub alignments: String,
    pub art: ArtContext,
    pub cancel: CancellationToken,
}
pub enum Task {
    Prepare(Box<PrepareRequest>),
    Frame {
        generation: u64,
        sample_frame: u64,
        prepared: Arc<Prepared>,
        cancel: CancellationToken,
    },
    Export {
        generation: u64,
        prepared: Arc<Prepared>,
        destination: PathBuf,
        ffmpeg: Option<PathBuf>,
        cancel: CancellationToken,
    },
}
pub enum Update {
    PrepareProgress {
        generation: u64,
        completed: usize,
        total: usize,
    },
    Prepared {
        generation: u64,
        result: Result<Arc<Prepared>, String>,
    },
    Frame {
        generation: u64,
        sample_frame: u64,
        result: Result<SequencePreviewFrame, String>,
    },
    ExportProgress {
        generation: u64,
        progress: motion_export::Progress,
    },
    Exported {
        generation: u64,
        result: Result<Report, String>,
    },
}
impl Task {
    fn token(&self) -> &CancellationToken {
        match self {
            Self::Prepare(r) => &r.cancel,
            Self::Frame { cancel, .. } | Self::Export { cancel, .. } => cancel,
        }
    }
    fn panic_result(&self) -> Update {
        let error = Err("Project worker panicked; check inputs before retrying".into());
        match self {
            Self::Prepare(r) => Update::Prepared {
                generation: r.generation,
                result: error,
            },
            Self::Frame {
                generation,
                sample_frame,
                ..
            } => Update::Frame {
                generation: *generation,
                sample_frame: *sample_frame,
                result: Err("Project frame worker panicked".into()),
            },
            Self::Export { generation, .. } => Update::Exported {
                generation: *generation,
                result: Err("Project export worker panicked".into()),
            },
        }
    }
}
#[derive(Default)]
struct State {
    updates: VecDeque<Update>,
    active: Option<CancellationToken>,
}
impl State {
    fn progress(&mut self, update: Update) {
        // At most one progress snapshot and one final result. Progress never blocks
        // cancellation or completion if the UI stops polling.
        self.updates.retain(|u| {
            !matches!(
                u,
                Update::PrepareProgress { .. } | Update::ExportProgress { .. }
            )
        });
        if self.updates.len() < 3 {
            self.updates.push_back(update);
        }
    }
}
pub struct Worker {
    tx: mpsc::SyncSender<Task>,
    state: Arc<Mutex<State>>,
}
impl Worker {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel::<Task>(1);
        let state = Arc::new(Mutex::new(State::default()));
        let output = state.clone();
        std::thread::spawn(move || {
            while let Ok(task) = rx.recv() {
                let panic_result = task.panic_result();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    execute(task, &|update| {
                        if let Ok(mut state) = output.lock() {
                            state.progress(update);
                        }
                    })
                }))
                .unwrap_or(panic_result);
                if let Ok(mut state) = output.lock() {
                    state.updates.push_back(result);
                }
            }
        });
        Self { tx, state }
    }
    pub fn submit(&self, task: Task) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Project worker state unavailable")?;
        if state.active.is_some() {
            return Err(
                "Project worker busy; poll its result before submitting another task".into(),
            );
        }
        state.active = Some(task.token().clone());
        if let Err(error) = self.tx.try_send(task) {
            state.active = None;
            return Err(format!("Project queue: {error}"));
        }
        Ok(())
    }
    pub fn poll(&self) -> Option<Update> {
        let mut state = self.state.lock().ok()?;
        let update = state.updates.pop_front()?;
        if matches!(
            update,
            Update::Prepared { .. } | Update::Frame { .. } | Update::Exported { .. }
        ) {
            state.active = None;
        }
        Some(update)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Ok(state) = self.state.lock() {
            if let Some(cancel) = &state.active {
                cancel.cancel();
            }
        }
    }
}
fn execute(task: Task, progress: &impl Fn(Update)) -> Update {
    match task {
        Task::Prepare(request) => Update::Prepared {
            generation: request.generation,
            result: prepare(*request, progress).map(Arc::new),
        },
        Task::Frame {
            generation,
            sample_frame,
            prepared,
            cancel,
        } => Update::Frame {
            generation,
            sample_frame,
            result: prepared
                .preview
                .render_frame(sample_frame, 512, &cancel)
                .map_err(|e| e.to_string()),
        },
        Task::Export {
            generation,
            prepared,
            destination,
            ffmpeg,
            cancel,
        } => {
            let report = |p| {
                progress(Update::ExportProgress {
                    generation,
                    progress: p,
                })
            };
            let result = match ffmpeg {
                Some(ffmpeg) => motion_export::export_mp4_sequence(
                    &prepared.script,
                    &prepared.rendered,
                    Some(&prepared.art),
                    &destination,
                    &ffmpeg,
                    &cancel,
                    report,
                ),
                None => motion_export::export_png_sequence(
                    &prepared.script,
                    &prepared.rendered,
                    Some(&prepared.art),
                    &destination,
                    &cancel,
                    report,
                ),
            };
            Update::Exported {
                generation,
                result: result.map_err(|e| e.to_string()),
            }
        }
    }
}
fn prepare(request: PrepareRequest, progress: &impl Fn(Update)) -> Result<Prepared, String> {
    request.cancel.check().map_err(|e| e.to_string())?;
    if request.alignments.len() > 100_000 {
        return Err("Project alignment JSON exceeds 100 KB".into());
    }
    let inputs: BTreeMap<u64, AlignmentInput> = if request.alignments.trim().is_empty() {
        BTreeMap::new()
    } else {
        serde_json::from_str(&request.alignments)
            .map_err(|e| format!("Project alignment JSON: {e}"))?
    };
    let timeline = SequenceTimeline::compile(&request.script).map_err(|e| e.to_string())?;
    let source = timeline
        .spans()
        .iter()
        .try_fold(0u64, |n, span| n.checked_add(span.duration_samples))
        .ok_or("Project source duration overflow")?;
    if timeline.total_samples() > 24_000 * 600 || source > 24_000 * 1200 {
        return Err("Project exceeds ten-minute output or twenty-minute source limit".into());
    }
    let jobs =
        motion_voice::dialogue_jobs(&request.script, &request.root).map_err(|e| e.to_string())?;
    if jobs.len() > 10_000 {
        return Err("Project dialogue work budget exceeded".into());
    }
    if let Some(id) = inputs.keys().find(|id| !jobs.iter().any(|j| j.id == **id)) {
        return Err(format!("Unknown project alignment dialogue ID {id}"));
    }
    let provider = motion_tts::KokoroProvider {
        config: request.config.clone(),
    };
    let cache = AlignmentCache::new(request.config.cache_directory.join("alignments"));
    let mut rendered = Vec::with_capacity(request.script.scenes.len());
    let mut alignment_statuses = BTreeMap::new();
    let mut completed = 0;
    for (index, scene) in request.script.scenes.iter().enumerate() {
        request.cancel.check().map_err(|e| e.to_string())?;
        let local = inputs
            .iter()
            .filter(|(id, _)| jobs.iter().any(|j| j.scene == scene.id && j.id == **id))
            .map(|(id, input)| (*id, input.clone()))
            .collect();
        let prepared = motion_voice::scene_audio::prepare_scene_with_alignment_inputs(
            &request.script,
            index,
            &request.root,
            &provider,
            &local,
            &cache,
            &request.cancel,
            |p| {
                progress(Update::PrepareProgress {
                    generation: request.generation,
                    completed: completed + p.completed,
                    total: jobs.len(),
                })
            },
        )
        .map_err(|e| e.to_string())?;
        completed += prepared.prepared.clips.len();
        alignment_statuses.extend(prepared.alignment_statuses);
        rendered.push(prepared.prepared.rendered);
    }
    let audio = motion_voice::sequence::mix_sequence(&timeline, &rendered, &request.cancel)
        .map_err(|e| e.to_string())?;
    let peaks = motion_audio::waveform_peaks(&audio, audio.frames().div_ceil(700).max(1))
        .map_err(|e| e.to_string())?;
    request.cancel.check().map_err(|e| e.to_string())?;
    let script = Arc::new(request.script);
    let art = Arc::new(request.art);
    let rendered = Arc::new(rendered);
    let preview = SequencePreview::new(
        script.clone(),
        rendered.clone(),
        Some(art.clone()),
        &request.cancel,
    )
    .map_err(|e| e.to_string())?;
    Ok(Prepared {
        script,
        art,
        rendered,
        preview,
        audio: Arc::new(audio),
        timeline,
        peaks,
        alignment_statuses,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    fn request(generation: u64) -> PrepareRequest {
        PrepareRequest {
            generation,
            script: motion_script::parse("characters: {}\nscenes: [{id: a, duration: 0.002, events: []}, {id: b, duration: 0.003, events: []}]").unwrap(),
            root: std::env::temp_dir(),
            config: KokoroConfig {
                python: "/never-execute/python".into(), runtime_script: "/never-execute/runtime".into(),
                model_config: "/never-execute/config".into(), model_weights: "/never-execute/model".into(),
                voices_directory: "/never-execute/voices".into(), cache_directory: std::env::temp_dir().join("sequence-test-unused-cache"),
                model_version: "test".into(), timeout: Duration::from_secs(1),
            },
            alignments: String::new(), art: ArtContext::default(), cancel: CancellationToken::default(),
        }
    }
    fn finish(worker: &Worker) -> Update {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(update) = worker.poll() {
                if matches!(
                    update,
                    Update::Prepared { .. } | Update::Frame { .. } | Update::Exported { .. }
                ) {
                    return update;
                }
            }
            assert!(Instant::now() < deadline, "worker timed out");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn empty_speech_multiscene_prepares_without_external_tools() {
        let worker = Worker::new();
        worker.submit(Task::Prepare(Box::new(request(42)))).unwrap();
        let Update::Prepared { generation, result } = finish(&worker) else {
            panic!("wrong update")
        };
        let prepared = result.unwrap();
        assert_eq!(generation, 42);
        assert_eq!(prepared.rendered.len(), 2);
        assert_eq!(prepared.timeline.total_samples(), 120);
        assert_eq!(prepared.audio.frames(), 120);
        assert!(prepared.audio.samples.iter().all(|s| *s == 0.0));
        assert!(prepared.alignment_statuses.is_empty());
    }
    #[test]
    fn queue_is_bounded_until_completion_is_consumed_and_cancel_keeps_generation() {
        let worker = Worker::new();
        let first = request(99);
        first.cancel.cancel();
        worker.submit(Task::Prepare(Box::new(first))).unwrap();
        assert!(worker
            .submit(Task::Prepare(Box::new(request(100))))
            .is_err());
        let Update::Prepared { generation, result } = finish(&worker) else {
            panic!("wrong update")
        };
        assert_eq!(generation, 99);
        assert!(result.err().unwrap().contains("cancelled"));
        worker
            .submit(Task::Prepare(Box::new(request(100))))
            .unwrap();
        assert!(matches!(
            finish(&worker),
            Update::Prepared {
                generation: 100,
                result: Ok(_)
            }
        ));
        assert!(worker.state.lock().unwrap().updates.len() <= 4);
    }
    #[test]
    fn rejects_foreign_alignment_and_duration_before_inference() {
        let worker = Worker::new();
        let mut foreign = request(1);
        foreign.alignments =
            r#"{"888":{"source":"manual","duration_seconds":1.0,"phonemes":[]}}"#.into();
        worker.submit(Task::Prepare(Box::new(foreign))).unwrap();
        assert!(matches!(
            finish(&worker),
            Update::Prepared { result: Err(_), .. }
        ));
        let mut huge = request(2);
        huge.script.scenes[0].duration = 601.0;
        worker.submit(Task::Prepare(Box::new(huge))).unwrap();
        let Update::Prepared { result, .. } = finish(&worker) else {
            panic!("wrong update")
        };
        assert!(result.err().unwrap().contains("ten-minute"));
    }
    #[test]
    fn dropping_worker_cancels_its_outstanding_request() {
        let worker = Worker::new();
        let request = request(3);
        let token = request.cancel.clone();
        worker.submit(Task::Prepare(Box::new(request))).unwrap();
        drop(worker);
        assert!(token.is_cancelled());
    }
}
