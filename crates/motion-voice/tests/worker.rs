use motion_tts::{SpeechError, SpeechProvider, SpeechRequest, SpeechResult, VoiceSettings};
use motion_voice::{DialogueJob, VoiceUpdate, VoiceWorker};
use std::{
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

struct Panics;
impl SpeechProvider for Panics {
    fn synthesize(&self, _: SpeechRequest) -> Result<SpeechResult, SpeechError> {
        panic!("test provider failure")
    }
}
fn job(id: u64) -> DialogueJob {
    DialogueJob {
        id,
        scene: "intro".into(),
        actor: "EMMA".into(),
        at: 1.0,
        request: SpeechRequest {
            text: "Hello".into(),
            voice: VoiceSettings::default(),
        },
        imported_audio: None,
    }
}
#[test]
fn provider_panic_reports_failure_and_worker_accepts_next_job() {
    let worker = VoiceWorker::new(Arc::new(Panics), 2);
    worker.submit(job(1)).unwrap();
    worker.submit(job(2)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut finished = Vec::new();
    while finished.len() < 2 && Instant::now() < deadline {
        if let Some(VoiceUpdate::Finished { id, result }) = worker.poll() {
            assert!(result.unwrap_err().0.contains("panicked"));
            finished.push(id);
        }
        std::thread::yield_now();
    }
    assert_eq!(finished, vec![1, 2]);
}
struct Blocked(mpsc::Sender<()>, std::sync::Mutex<mpsc::Receiver<()>>);
impl SpeechProvider for Blocked {
    fn synthesize(&self, _: SpeechRequest) -> Result<SpeechResult, SpeechError> {
        self.0.send(()).unwrap();
        self.1.lock().unwrap().recv().unwrap();
        Err(SpeechError("fixture".into()))
    }
}
#[test]
fn running_provider_does_not_block_submit_and_full_queue_is_explicit() {
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker = VoiceWorker::new(
        Arc::new(Blocked(started_tx, std::sync::Mutex::new(release_rx))),
        1,
    );
    worker.submit(job(1)).unwrap();
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    worker.submit(job(2)).unwrap();
    assert!(matches!(
        worker.submit(job(3)),
        Err(mpsc::TrySendError::Full(_))
    ));
    release_tx.send(()).unwrap();
    release_tx.send(()).unwrap();
}
#[test]
fn missing_import_is_a_job_error() {
    let worker = VoiceWorker::new(Arc::new(Panics), 1);
    let mut imported = job(9);
    imported.imported_audio = Some("/nonexistent/motion-voice-fixture.wav".into());
    worker.submit(imported).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(VoiceUpdate::Finished { id, result }) = worker.poll() {
            assert_eq!(id, 9);
            assert!(result.unwrap_err().0.contains("import audio"));
            break;
        }
        assert!(
            Instant::now() < deadline,
            "worker did not report import error"
        );
        std::thread::yield_now();
    }
}

#[test]
fn cancelled_worker_job_skips_provider_and_next_job_runs() {
    let worker = VoiceWorker::new(Arc::new(Panics), 2);
    let cancel = motion_tts::CancellationToken::new();
    cancel.cancel();
    // A missing import proves cancellation is checked before even opening the WAV.
    let mut cancelled = job(1);
    cancelled.imported_audio = Some("/nonexistent/cancelled.wav".into());
    assert!(worker.submit_cancellable(cancelled, cancel).is_ok());
    worker.submit(job(2)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut finished = Vec::new();
    while finished.len() < 2 && Instant::now() < deadline {
        if let Some(VoiceUpdate::Finished { id, result }) = worker.poll() {
            let error = result.unwrap_err();
            assert!(error
                .0
                .contains(if id == 1 { "cancel" } else { "panicked" }));
            finished.push(id);
        }
        std::thread::yield_now();
    }
    assert_eq!(finished, vec![1, 2]);
}

struct ReturnsAfterRelease(mpsc::Sender<()>, std::sync::Mutex<mpsc::Receiver<()>>);
impl SpeechProvider for ReturnsAfterRelease {
    fn synthesize(&self, _: SpeechRequest) -> Result<SpeechResult, SpeechError> {
        self.0.send(()).unwrap();
        self.1.lock().unwrap().recv().unwrap();
        Ok(SpeechResult {
            audio_path: "unused.wav".into(),
            sample_rate: 24000,
            sample_count: 1,
            cache_key: String::new(),
            phonemes: String::new(),
            cached: false,
        })
    }
}
#[test]
fn cancelling_running_job_discards_late_success() {
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker = VoiceWorker::new(
        Arc::new(ReturnsAfterRelease(
            started_tx,
            std::sync::Mutex::new(release_rx),
        )),
        1,
    );
    let cancel = motion_tts::CancellationToken::new();
    assert!(worker.submit_cancellable(job(4), cancel.clone()).is_ok());
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    cancel.cancel();
    release_tx.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(VoiceUpdate::Finished { id, result }) = worker.poll() {
            assert_eq!(id, 4);
            assert!(result.unwrap_err().0.contains("cancel"));
            break;
        }
        assert!(
            Instant::now() < deadline,
            "worker did not report cancellation"
        );
        std::thread::yield_now();
    }
}
