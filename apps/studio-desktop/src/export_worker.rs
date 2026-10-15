//! One persistent export thread. Progress is coalesced, never queued per frame.
use motion_export::{Progress, Report};
use motion_script::Script;
use motion_tts::CancellationToken;
use motion_voice::export::RenderedScene;
use std::{
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    #[default]
    Svg,
    Png,
    Mp4,
}

pub struct Request {
    pub format: Format,
    pub ffmpeg: PathBuf,
    pub script: Script,
    pub scene_index: usize,
    pub audio: Arc<motion_audio::AudioBuffer>,
    pub manifest: Arc<motion_voice::export::SceneManifest>,
    pub destination: PathBuf,
    pub cancel: CancellationToken,
}

pub enum Update {
    Progress(Progress),
    Finished(Result<Report, String>),
}

#[derive(Default)]
struct State {
    // Kept until completion is consumed, bounding both requests and results.
    active: Option<CancellationToken>,
    progress: Option<Progress>,
}

pub struct Worker {
    tx: Option<mpsc::SyncSender<Request>>,
    rx: mpsc::Receiver<Result<Report, String>>,
    state: Arc<Mutex<State>>,
}

impl Worker {
    pub fn new() -> Self {
        Self::with_executor(|request, progress| {
            request.cancel.check().map_err(|error| error.to_string())?;
            // The mixed samples are copied on this worker, never on the UI thread.
            let rendered = RenderedScene {
                audio: (*request.audio).clone(),
                manifest: (*request.manifest).clone(),
            };
            request.cancel.check().map_err(|error| error.to_string())?;
            match request.format {
                Format::Svg => motion_export::export_scene(
                    &request.script,
                    request.scene_index,
                    &rendered,
                    &request.destination,
                    &request.cancel,
                    progress,
                ),
                Format::Png => motion_export::export_png_scene(
                    &request.script,
                    request.scene_index,
                    &rendered,
                    &request.destination,
                    &request.cancel,
                    progress,
                ),
                Format::Mp4 => motion_export::export_mp4_scene(
                    &request.script,
                    request.scene_index,
                    &rendered,
                    &request.destination,
                    &request.ffmpeg,
                    &request.cancel,
                    progress,
                ),
            }
            .map_err(|error| error.to_string())
        })
    }

    fn with_executor(
        execute: impl Fn(Request, &mut dyn FnMut(Progress)) -> Result<Report, String> + Send + 'static,
    ) -> Self {
        let (tx, requests) = mpsc::sync_channel::<Request>(1);
        let (finished, rx) = mpsc::sync_channel(1);
        let state = Arc::new(Mutex::new(State::default()));
        let worker_state = Arc::clone(&state);
        std::thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    request.cancel.check().map_err(|error| error.to_string())?;
                    execute(request, &mut |progress| {
                        if let Ok(mut state) = worker_state.lock() {
                            state.progress = Some(progress);
                        }
                    })
                }))
                .unwrap_or_else(|_| Err("Export worker panicked; check inputs and retry".into()));
                // Only one accepted job can be outstanding. Never block shutdown
                // on a UI that has stopped polling, even if that invariant changes.
                if finished.try_send(result).is_err() {
                    break;
                }
            }
        });
        Self {
            tx: Some(tx),
            rx,
            state,
        }
    }

    pub fn submit(&self, request: Request) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "Export state unavailable")?;
        if state.active.is_some() {
            return Err("An export is already pending; wait for its result".into());
        }
        state.active = Some(request.cancel.clone());
        state.progress = None;
        let result = self
            .tx
            .as_ref()
            .ok_or_else(|| "Export worker stopped".to_string())?
            .try_send(request)
            .map_err(|error| format!("Export queue: {error}"));
        if result.is_err() {
            state.active = None;
        }
        result
    }

    pub fn poll(&self) -> Option<Update> {
        match self.rx.try_recv() {
            Ok(result) => {
                if let Ok(mut state) = self.state.lock() {
                    state.active = None;
                    state.progress = None;
                }
                Some(Update::Finished(result))
            }
            Err(mpsc::TryRecvError::Empty) => self
                .state
                .lock()
                .ok()?
                .progress
                .take()
                .map(Update::Progress),
            Err(mpsc::TryRecvError::Disconnected) => {
                let mut state = self.state.lock().ok()?;
                state.progress = None;
                state.active.take().map(|_| {
                    Update::Finished(Err(
                        "Export worker stopped unexpectedly; reopen the app".into()
                    ))
                })
            }
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Ok(state) = self.state.lock() {
            if let Some(cancel) = &state.active {
                cancel.cancel();
            }
        }
        // Closing the queue wakes an idle worker. Active exports check cancellation
        // cooperatively; do not join filesystem work on the UI thread.
        self.tx.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn request() -> Request {
        let rendered = motion_voice::export::render_scene("test", 0.1, &[]).unwrap();
        Request {
            format: Format::Svg,
            ffmpeg: PathBuf::new(),
            script: Script {
                project: Default::default(),
                characters: Default::default(),
                scenes: vec![],
            },
            scene_index: 0,
            audio: Arc::new(rendered.audio),
            manifest: Arc::new(rendered.manifest),
            destination: PathBuf::from("unused-worker-test-output"),
            cancel: CancellationToken::new(),
        }
    }

    fn finished(worker: &Worker) -> Result<Report, String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(Update::Finished(result)) = worker.poll() {
                return result;
            }
            assert!(Instant::now() < deadline, "worker did not finish");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn png_dispatch_writes_real_image_on_worker() {
        let worker = Worker::new();
        let mut request = request();
        request.format = Format::Png;
        request.script = motion_script::parse("project: {fps: 10, width: 64, height: 64}\ncharacters: {}\nscenes: [{id: test, duration: 0.1, events: []}]\n").unwrap();
        request.destination = std::env::temp_dir().join(format!(
            "kilsat-worker-png-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let destination = request.destination.clone();
        worker.submit(request).unwrap();
        let report = finished(&worker).unwrap();
        assert_eq!(report.frame_count, 1);
        let frame = destination.join("frame_000000.png");
        let bytes = std::fs::read(frame).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        std::fs::remove_dir_all(destination).unwrap();
    }

    #[test]
    fn progress_does_not_require_polling_and_completion_bounds_requests() {
        let (ready, observed) = mpsc::channel();
        let worker = Worker::with_executor(move |request, progress| {
            for completed_frames in 1..=10_000 {
                progress(Progress {
                    completed_frames,
                    total_frames: 10_000,
                });
            }
            ready.send(()).unwrap();
            Ok(Report {
                frame_count: 10_000,
                output_directory: request.destination,
            })
        });
        worker.submit(request()).unwrap();
        observed.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(worker.submit(request()).is_err());
        assert_eq!(finished(&worker).unwrap().frame_count, 10_000);
        worker.submit(request()).unwrap();
        assert_eq!(finished(&worker).unwrap().frame_count, 10_000);
    }

    #[test]
    fn cancellation_before_execution_is_recoverable() {
        let worker = Worker::with_executor(|request, _| {
            Ok(Report {
                frame_count: 1,
                output_directory: request.destination,
            })
        });
        let cancelled = request();
        cancelled.cancel.cancel();
        worker.submit(cancelled).unwrap();
        assert!(finished(&worker).err().unwrap().contains("cancelled"));
        worker.submit(request()).unwrap();
        assert_eq!(finished(&worker).unwrap().frame_count, 1);
    }

    #[test]
    fn drop_cancels_active_job_without_waiting_for_completion_poll() {
        let (started, running) = mpsc::channel();
        let (stopped, cancelled) = mpsc::channel();
        let worker = Worker::with_executor(move |request, _| {
            started.send(()).unwrap();
            while !request.cancel.is_cancelled() {
                std::thread::sleep(Duration::from_millis(1));
            }
            stopped.send(()).unwrap();
            Err("cancelled".into())
        });
        worker.submit(request()).unwrap();
        running.recv_timeout(Duration::from_secs(5)).unwrap();
        drop(worker);
        cancelled.recv_timeout(Duration::from_secs(5)).unwrap();
    }

    #[test]
    fn executor_panic_reports_failure_and_allows_retry() {
        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let worker = Worker::with_executor(move |request, _| {
            if attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                panic!("simulated renderer failure");
            }
            Ok(Report {
                frame_count: 2,
                output_directory: request.destination,
            })
        });
        worker.submit(request()).unwrap();
        assert!(finished(&worker).err().unwrap().contains("panicked"));
        worker.submit(request()).unwrap();
        assert_eq!(finished(&worker).unwrap().frame_count, 2);
    }
}
