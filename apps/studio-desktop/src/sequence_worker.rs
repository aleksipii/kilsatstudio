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
    /// Process-budget bytes for the retained scene and mixed PCM; released on drop.
    _reservations: Vec<motion_jobs::Reservation>,
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
pub enum ExportFormat {
    Png,
    Svg,
    Mp4 { ffmpeg: PathBuf },
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
        format: ExportFormat,
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
fn execute(task: Task, progress: &(impl Fn(Update) + Sync)) -> Update {
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
            format,
            cancel,
        } => {
            let report = |p| {
                progress(Update::ExportProgress {
                    generation,
                    progress: p,
                })
            };
            let result = match format {
                ExportFormat::Mp4 { ffmpeg } => motion_export::export_mp4_sequence(
                    &prepared.script,
                    &prepared.rendered,
                    Some(&prepared.art),
                    &destination,
                    &ffmpeg,
                    &cancel,
                    report,
                ),
                ExportFormat::Svg => motion_export::export_svg_sequence(
                    &prepared.script,
                    &prepared.rendered,
                    Some(&prepared.art),
                    &destination,
                    &cancel,
                    report,
                ),
                ExportFormat::Png => motion_export::export_png_sequence(
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
fn prepare(
    request: PrepareRequest,
    progress: &(impl Fn(Update) + Sync),
) -> Result<Prepared, String> {
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
    let Stages {
        rendered,
        alignment_statuses,
        audio,
        peaks,
        reservations,
    } = run_stages(
        &request,
        &timeline,
        &inputs,
        &jobs,
        &provider,
        &cache,
        PREPARE_WORKERS,
        progress,
    )?;
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
        _reservations: reservations,
    })
}

/// Scene speech/alignment preparation is serialized by default: each scene may start
/// the local speech sidecar, and concurrent inference has not been shown safe or
/// faster on this runtime. The dependency graph is correct for more workers (tests
/// compare 1 and 3), but only the dependency ordering is enabled in the product.
const PREPARE_WORKERS: usize = 1;

#[allow(clippy::large_enum_variant)] // short-lived channel messages
enum Stage {
    Scene(motion_voice::scene_audio::PreparedAlignedScene),
    Mix(AudioBuffer),
    Peaks(Vec<WaveformPeak>),
}
struct Stages {
    rendered: Vec<RenderedScene>,
    alignment_statuses: BTreeMap<u64, AlignmentStatus>,
    audio: AudioBuffer,
    peaks: Vec<WaveformPeak>,
    reservations: Vec<motion_jobs::Reservation>,
}

/// Per-scene preparation -> sequence mix -> waveform peaks as a dependency graph:
/// `scene:N` jobs (no dependencies), `mix` after every scene, `peaks` after `mix`.
/// Results are assembled in screenplay order whatever the worker count; the first
/// failing job in registration order supplies the error.
#[allow(clippy::too_many_arguments)]
fn run_stages(
    request: &PrepareRequest,
    timeline: &SequenceTimeline,
    inputs: &BTreeMap<u64, AlignmentInput>,
    jobs: &[motion_voice::DialogueJob],
    provider: &dyn motion_tts::SpeechProvider,
    cache: &AlignmentCache,
    workers: usize,
    progress: &(impl Fn(Update) + Sync),
) -> Result<Stages, String> {
    use motion_jobs::{Graph, Outcome};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let completed = AtomicUsize::new(0);
    let mut graph: Graph<'_, Stage, String> = Graph::new();
    let mut scene_names = Vec::with_capacity(request.script.scenes.len());
    for (index, scene) in request.script.scenes.iter().enumerate() {
        let name = format!("scene:{index}");
        let (completed, scene_id) = (&completed, scene.id.as_str());
        graph
            .add(&name, &[], move |_| {
                request.cancel.check().map_err(|e| e.to_string())?;
                let local = inputs
                    .iter()
                    .filter(|(id, _)| jobs.iter().any(|j| j.scene == scene_id && j.id == **id))
                    .map(|(id, input)| (*id, input.clone()))
                    .collect();
                let mut last = 0;
                let mut report = |done: usize| {
                    let delta = done.saturating_sub(last);
                    last = last.max(done);
                    let total_done = completed.fetch_add(delta, Ordering::AcqRel) + delta;
                    progress(Update::PrepareProgress {
                        generation: request.generation,
                        completed: total_done,
                        total: jobs.len(),
                    });
                };
                let prepared = motion_voice::scene_audio::prepare_scene_with_alignment_inputs(
                    &request.script,
                    index,
                    &request.root,
                    provider,
                    &local,
                    cache,
                    &request.cancel,
                    |p| report(p.completed),
                )
                .map_err(|e| e.to_string())?;
                report(prepared.prepared.clips.len());
                Ok(Stage::Scene(prepared))
            })
            .map_err(|e| e.to_string())?;
        scene_names.push(name);
    }
    let deps: Vec<&str> = scene_names.iter().map(String::as_str).collect();
    graph
        .add("mix", &deps, |scenes| {
            let rendered: Vec<RenderedScene> = scenes
                .iter()
                .map(|stage| match stage.as_ref() {
                    Stage::Scene(s) => s.prepared.rendered.clone(),
                    _ => unreachable!("mix depends only on scene jobs"),
                })
                .collect();
            motion_voice::sequence::mix_sequence(timeline, &rendered, &request.cancel)
                .map(Stage::Mix)
                .map_err(|e| e.to_string())
        })
        .map_err(|e| e.to_string())?;
    graph
        .add("peaks", &["mix"], |mix| {
            let Stage::Mix(audio) = mix[0].as_ref() else {
                unreachable!("peaks depends only on mix")
            };
            motion_audio::waveform_peaks(audio, audio.frames().div_ceil(700).max(1))
                .map(Stage::Peaks)
                .map_err(|e| e.to_string())
        })
        .map_err(|e| e.to_string())?;
    let results = graph
        .run(workers, || request.cancel.is_cancelled())
        .map_err(|e| e.to_string())?;

    let mut stages = Vec::with_capacity(results.len());
    for (_, outcome) in results {
        match outcome {
            Outcome::Done(stage) => stages.push(stage),
            Outcome::Failed(error) => return Err(error),
            Outcome::Panicked => {
                return Err("Project worker panicked; check inputs before retrying".into())
            }
            Outcome::Cancelled => {
                return Err(request
                    .cancel
                    .check()
                    .err()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "Speech generation cancelled".into()))
            }
            // The failed dependency itself precedes its dependants in registration order.
            Outcome::DependencyFailed(name) => {
                return Err(format!("Project preparation stopped after {name} failed"))
            }
        }
    }
    let mut stages = stages.into_iter();
    let mut rendered = Vec::new();
    let mut alignment_statuses = BTreeMap::new();
    for _ in 0..request.script.scenes.len() {
        let Some(stage) = stages.next() else {
            return Err("Project preparation lost a scene result".into());
        };
        let Ok(Stage::Scene(scene)) = Arc::try_unwrap(stage) else {
            return Err("Project preparation scene result is still shared".into());
        };
        alignment_statuses.extend(scene.alignment_statuses);
        rendered.push(scene.prepared.rendered);
    }
    let (Some(mix), Some(peaks)) = (stages.next(), stages.next()) else {
        return Err("Project preparation lost its mix result".into());
    };
    let (Ok(Stage::Mix(audio)), Ok(Stage::Peaks(peaks))) =
        (Arc::try_unwrap(mix), Arc::try_unwrap(peaks))
    else {
        return Err("Project preparation mix result is still shared".into());
    };
    // Account the retained PCM against the process budget (deterministic refusal).
    let budget = motion_jobs::process_budget();
    let mut reservations = Vec::with_capacity(rendered.len() + 1);
    for scene in &rendered {
        reservations.push(
            budget
                .reserve_pcm(
                    &format!("scene {} prepared audio", scene.manifest.scene_id),
                    scene.audio.frames() as u64,
                    scene.audio.channels,
                )
                .map_err(|e| e.to_string())?,
        );
    }
    reservations.push(
        budget
            .reserve_pcm(
                "sequence mixed audio",
                audio.frames() as u64,
                audio.channels,
            )
            .map_err(|e| e.to_string())?,
    );
    Ok(Stages {
        rendered,
        alignment_statuses,
        audio,
        peaks,
        reservations,
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
    fn worker_exports_selected_vector_or_raster_format_without_encoder() {
        let worker = Worker::new();
        // Verify worker dispatch and real formats without a full-HD performance gate.
        let mut input = request(7);
        input.script.project.width = 64;
        input.script.project.height = 64;
        worker.submit(Task::Prepare(Box::new(input))).unwrap();
        let Update::Prepared { result, .. } = finish(&worker) else {
            panic!("wrong update")
        };
        let prepared = result.unwrap();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "native-sequence-export-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        for (format, extension) in [(ExportFormat::Svg, "svg"), (ExportFormat::Png, "png")] {
            let destination = root.join(extension);
            worker
                .submit(Task::Export {
                    generation: 7,
                    prepared: prepared.clone(),
                    destination: destination.clone(),
                    format,
                    cancel: CancellationToken::new(),
                })
                .unwrap();
            let Update::Exported { generation, result } = finish(&worker) else {
                panic!("wrong update")
            };
            assert_eq!(generation, 7);
            assert!(result.unwrap().frame_count > 0);
            assert!(destination.join("audio.wav").is_file());
            assert!(std::fs::read_dir(&destination).unwrap().any(|entry| {
                entry
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|value| value == extension)
            }));
        }
        std::fs::remove_dir_all(root).unwrap();
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

    struct NoSynthesis;
    impl motion_tts::SpeechProvider for NoSynthesis {
        fn synthesize(
            &self,
            _: motion_tts::SpeechRequest,
        ) -> Result<motion_tts::SpeechResult, motion_tts::SpeechError> {
            Err(motion_tts::SpeechError(
                "dialogue uses imported audio".into(),
            ))
        }
    }
    struct Project(PathBuf);
    impl Project {
        /// Three scenes with imported dialogue audio and decoded cues, no synthesis.
        fn new(tag: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("kilsat-graph-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(root.join("audio")).unwrap();
            let wav = |name: &str, value: f32, frames: usize| {
                motion_audio::write_wav(
                    root.join(name),
                    &AudioBuffer {
                        sample_rate: 24000,
                        channels: 1,
                        samples: (0..frames)
                            .map(|i| value * ((i % 7) as f32 / 7.0))
                            .collect(),
                    },
                )
                .unwrap()
            };
            wav("voice.wav", 0.5, 12000);
            wav("audio/hit.wav", 0.25, 500);
            Self(root)
        }
        fn script(cue_asset: &str) -> Script {
            motion_script::parse(&format!(
                "characters: {{EMMA: {{asset: characters/emma}}}}\nscenes:\n\
                 - {{id: a, duration: 1.0, composition: {{audio_cues: [{{id: h, asset: audio/hit.wav, at: 0.25, gain: 1.5}}]}}, events: [{{type: speak, at: 0.0, actor: EMMA, text: Hi, audio: voice.wav}}]}}\n\
                 - {{id: b, duration: 0.5, composition: {{audio_cues: [{{id: h, asset: {cue_asset}, at: 0.1}}]}}, events: []}}\n\
                 - {{id: c, duration: 0.75, events: [{{type: speak, at: 0.1, actor: EMMA, text: Again, audio: voice.wav}}]}}\n"
            ))
            .unwrap()
        }
    }
    impl Drop for Project {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The linear loop that `prepare` used before the dependency graph (reference).
    fn linear_reference(
        request: &PrepareRequest,
        timeline: &SequenceTimeline,
    ) -> Result<Stages, String> {
        let cache = AlignmentCache::new(request.config.cache_directory.join("alignments"));
        let mut rendered = Vec::new();
        let mut statuses = BTreeMap::new();
        for index in 0..request.script.scenes.len() {
            request.cancel.check().map_err(|e| e.to_string())?;
            let prepared = motion_voice::scene_audio::prepare_scene_with_alignment_inputs(
                &request.script,
                index,
                &request.root,
                &NoSynthesis,
                &BTreeMap::new(),
                &cache,
                &request.cancel,
                |_| {},
            )
            .map_err(|e| e.to_string())?;
            statuses.extend(prepared.alignment_statuses);
            rendered.push(prepared.prepared.rendered);
        }
        let audio = motion_voice::sequence::mix_sequence(timeline, &rendered, &request.cancel)
            .map_err(|e| e.to_string())?;
        let peaks = motion_audio::waveform_peaks(&audio, audio.frames().div_ceil(700).max(1))
            .map_err(|e| e.to_string())?;
        Ok(Stages {
            rendered,
            alignment_statuses: statuses,
            audio,
            peaks,
            reservations: Vec::new(),
        })
    }
    fn graph_stages(
        request: &PrepareRequest,
        workers: usize,
        updates: &Mutex<Vec<(usize, usize)>>,
    ) -> Result<Stages, String> {
        let timeline = SequenceTimeline::compile(&request.script).unwrap();
        let jobs = motion_voice::dialogue_jobs(&request.script, &request.root).unwrap();
        let cache = AlignmentCache::new(request.config.cache_directory.join("alignments"));
        run_stages(
            request,
            &timeline,
            &BTreeMap::new(),
            &jobs,
            &NoSynthesis,
            &cache,
            workers,
            &|update| {
                if let Update::PrepareProgress {
                    completed, total, ..
                } = update
                {
                    updates.lock().unwrap().push((completed, total));
                }
            },
        )
    }
    fn with_script(project: &Project, script: Script) -> PrepareRequest {
        let mut r = request(1);
        r.script = script;
        r.root = project.0.clone();
        r
    }

    #[test]
    fn graph_preparation_equals_the_linear_path_for_one_and_three_workers() {
        let project = Project::new("equal");
        let request = with_script(&project, Project::script("audio/hit.wav"));
        let timeline = SequenceTimeline::compile(&request.script).unwrap();
        let reference = linear_reference(&request, &timeline).unwrap();
        assert!(reference.audio.samples.iter().any(|s| *s != 0.0));
        for workers in [1, 3] {
            let updates = Mutex::new(Vec::new());
            let graph = graph_stages(&request, workers, &updates).unwrap();
            assert_eq!(graph.audio.samples, reference.audio.samples, "{workers}");
            assert_eq!(graph.peaks, reference.peaks, "{workers}");
            assert_eq!(graph.alignment_statuses, reference.alignment_statuses);
            assert_eq!(graph.rendered.len(), reference.rendered.len());
            for (g, r) in graph.rendered.iter().zip(&reference.rendered) {
                assert_eq!(g.audio.samples, r.audio.samples, "{workers}");
                assert_eq!(
                    serde_json::to_string(&g.manifest).unwrap(),
                    serde_json::to_string(&r.manifest).unwrap()
                );
            }
            // Two imported-audio lines: progress totals are the job count, the
            // cumulative count never decreases and reaches it.
            let updates = updates.into_inner().unwrap();
            assert!(updates.iter().all(|(_, total)| *total == 2));
            assert!(updates.iter().map(|(c, _)| *c).is_sorted());
            assert_eq!(updates.last().unwrap().0, 2);
        }
    }

    #[test]
    fn graph_preparation_reports_the_same_typed_failure_as_the_linear_path() {
        let project = Project::new("failure");
        let request = with_script(&project, Project::script("audio/missing.wav"));
        let timeline = SequenceTimeline::compile(&request.script).unwrap();
        let reference = linear_reference(&request, &timeline).err().unwrap();
        assert!(reference.contains("scene b audio cue h"), "{reference}");
        for workers in [1, 3] {
            let graph = graph_stages(&request, workers, &Mutex::new(Vec::new()))
                .err()
                .unwrap();
            assert_eq!(graph, reference, "{workers}");
        }
    }

    #[test]
    fn graph_preparation_honours_cancellation_before_any_scene_starts() {
        let project = Project::new("cancel");
        let request = with_script(&project, Project::script("audio/hit.wav"));
        request.cancel.cancel();
        let error = graph_stages(&request, 3, &Mutex::new(Vec::new()))
            .err()
            .unwrap();
        assert!(error.contains("cancelled"), "{error}");
    }
}
