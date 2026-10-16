//! One bounded, cancellable local alignment worker; no speech synthesis or downloads.
use motion_audio::AudioBuffer;
use motion_lipsync::rhubarb::{self, Recognizer};
use motion_tts::CancellationToken;
use std::{
    path::PathBuf,
    sync::{mpsc, Arc},
};

pub const MAX_JSON_BYTES: usize = 100_000;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub generation: u64,
    pub line_id: u64,
    pub audio_path: PathBuf,
    pub transcript: String,
}
pub trait PreparedAudio: Send + Sync {
    fn audio(&self) -> &AudioBuffer;
}
impl PreparedAudio for AudioBuffer {
    fn audio(&self) -> &AudioBuffer {
        self
    }
}
pub struct Request {
    pub identity: Identity,
    pub audio: Arc<dyn PreparedAudio>,
    pub scene_alignments: String,
    pub executable: PathBuf,
    pub recognizer: Recognizer,
    pub cancel: CancellationToken,
}
#[derive(Debug)]
pub struct Ready {
    pub identity: Identity,
    pub json: String,
    pub scene_json: String,
}
pub struct Update {
    pub identity: Identity,
    pub result: Result<Ready, String>,
}
pub struct Worker {
    tx: Option<mpsc::SyncSender<Request>>,
    rx: mpsc::Receiver<Update>,
    active: Option<CancellationToken>,
}
fn execute(request: &Request) -> Result<Ready, String> {
    request.cancel.check().map_err(|e| e.to_string())?;
    if request.scene_alignments.len() > MAX_JSON_BYTES {
        return Err("Scene alignment JSON exceeds 100 KB; shorten the input and retry".into());
    }
    let mut scene_alignments: std::collections::BTreeMap<
        u64,
        motion_lipsync::cache::AlignmentInput,
    > = if request.scene_alignments.trim().is_empty() {
        Default::default()
    } else {
        serde_json::from_str(&request.scene_alignments)
            .map_err(|e| format!("Scene alignment JSON: {e}"))?
    };
    let alignment = rhubarb::align(
        &request.executable,
        &request.identity.audio_path,
        &request.identity.transcript,
        request.recognizer,
        || request.cancel.is_cancelled(),
    )
    .map_err(|e| e.to_string())?;
    // A cache path can have been replaced since preparation. Exact PCM and text,
    // rather than the path, determine whether this result can be applied.
    alignment
        .validate_for(request.audio.audio(), &request.identity.transcript)
        .map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&alignment).map_err(|e| e.to_string())?;
    if json.len() > MAX_JSON_BYTES {
        return Err("Estimated alignment exceeds the editor's 100 KB input limit; use a shorter dialogue line and retry".into());
    }
    request.cancel.check().map_err(|e| e.to_string())?;
    scene_alignments.insert(
        request.identity.line_id,
        motion_lipsync::cache::AlignmentInput::Estimated(alignment),
    );
    let scene_json = serde_json::to_string_pretty(&scene_alignments).map_err(|e| e.to_string())?;
    if scene_json.len() > MAX_JSON_BYTES {
        return Err("Combined scene alignment exceeds 100 KB; shorten dialogue or remove unused alignment entries and retry".into());
    }
    request.cancel.check().map_err(|e| e.to_string())?;
    Ok(Ready {
        identity: request.identity.clone(),
        json,
        scene_json,
    })
}
impl Worker {
    pub fn new() -> Self {
        Self::with_runner(execute)
    }
    fn with_runner(run: impl Fn(&Request) -> Result<Ready, String> + Send + 'static) -> Self {
        let (tx, requests) = mpsc::sync_channel::<Request>(1);
        let (updates, rx) = mpsc::sync_channel(1);
        let _thread = std::thread::Builder::new()
            .name("native-alignment".into())
            .spawn(move || {
                while let Ok(request) = requests.recv() {
                    let result =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&request)))
                            .unwrap_or_else(|_| {
                                Err(
                            "Local alignment worker failed; retry after checking the executable"
                                .into(),
                        )
                            });
                    if updates
                        .send(Update {
                            identity: request.identity,
                            result,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            });
        Self {
            tx: Some(tx),
            rx,
            active: None,
        }
    }
    pub fn submit(&mut self, request: Request) -> Result<(), String> {
        if self.active.is_some() {
            return Err("Alignment worker is busy; wait for cancellation to finish".into());
        }
        if !request.executable.is_absolute() {
            return Err("Choose an explicit absolute Rhubarb executable path".into());
        }
        let cancel = request.cancel.clone();
        self.tx
            .as_ref()
            .ok_or("Alignment worker has stopped")?
            .try_send(request)
            .map_err(|_| "Alignment worker is unavailable".to_owned())?;
        self.active = Some(cancel);
        Ok(())
    }
    pub fn poll(&mut self) -> Option<Update> {
        let update = self.rx.try_recv().ok()?;
        self.active = None;
        Some(update)
    }
    pub fn busy(&self) -> bool {
        self.active.is_some()
    }
    pub fn cancel(&self) {
        if let Some(c) = &self.active {
            c.cancel();
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel();
        self.tx.take();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Request {
        Request {
            identity: Identity {
                generation: 3,
                line_id: 4,
                audio_path: "/not-read.wav".into(),
                transcript: "hello".into(),
            },
            audio: Arc::new(AudioBuffer {
                sample_rate: 24000,
                channels: 1,
                samples: vec![0.0; 24],
            }),
            scene_alignments: String::new(),
            executable: "/not-executed".into(),
            recognizer: Recognizer::Phonetic,
            cancel: CancellationToken::new(),
        }
    }
    #[test]
    fn bounded_until_completion_is_consumed() {
        let mut worker = Worker::with_runner(|r| {
            Ok(Ready {
                identity: r.identity.clone(),
                json: "stub".into(),
                scene_json: "{}".into(),
            })
        });
        worker.submit(request()).unwrap();
        assert!(worker.submit(request()).is_err());
        for _ in 0..1000 {
            if let Some(u) = worker.poll() {
                assert_eq!(u.result.unwrap().json, "stub");
                assert!(!worker.busy());
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("worker did not finish");
    }
    #[test]
    fn cancellation_reaches_active_worker() {
        let mut worker = Worker::with_runner(|r| {
            while !r.cancel.is_cancelled() {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err("cancelled".into())
        });
        worker.submit(request()).unwrap();
        worker.cancel();
        for _ in 0..1000 {
            if let Some(u) = worker.poll() {
                assert!(u.result.unwrap_err().contains("cancelled"));
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("worker did not cancel");
    }
    #[cfg(unix)]
    #[test]
    fn synthetic_executable_merges_map_and_rejects_replaced_pcm() {
        use std::os::unix::fs::PermissionsExt;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "desktop-rhubarb-stub-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(path.clone());
        let mut r = request();
        r.executable = path.join("stub");
        std::fs::write(&r.executable, b"#!/bin/bash\nif [ \"$1\" = \"--version\" ]; then printf 'synthetic-worker-test'; else printf '%s' '{\"metadata\":{\"duration\":0.01},\"mouthCues\":[{\"start\":0.0,\"end\":0.01,\"value\":\"D\"}]}'; fi\n").unwrap();
        std::fs::set_permissions(&r.executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        r.identity.audio_path = path.join("audio.wav");
        let audio = AudioBuffer {
            sample_rate: 24000,
            channels: 1,
            samples: vec![0.1; 240],
        };
        motion_audio::write_wav(&r.identity.audio_path, &audio).unwrap();
        r.audio = Arc::new(motion_audio::read_wav(&r.identity.audio_path).unwrap());
        let first = execute(&r).unwrap();
        r.scene_alignments = format!("{{\"99\":{}}}", first.json);
        let ready = execute(&r).unwrap();
        let map: std::collections::BTreeMap<u64, motion_lipsync::cache::AlignmentInput> =
            serde_json::from_str(&ready.scene_json).unwrap();
        assert_eq!(map.len(), 2);
        assert!(map.contains_key(&r.identity.line_id));
        assert!(map.contains_key(&99));
        r.audio = Arc::new(AudioBuffer {
            sample_rate: 24000,
            channels: 1,
            samples: vec![0.2; 240],
        });
        assert!(execute(&r).unwrap_err().contains("exact decoded PCM"));
    }
    #[test]
    fn malformed_scene_map_is_rejected_before_file_access() {
        let mut r = request();
        r.scene_alignments = "invalid json".into();
        assert!(execute(&r).unwrap_err().contains("Scene alignment JSON"));
        r.scene_alignments = " ".repeat(MAX_JSON_BYTES + 1);
        assert!(execute(&r).unwrap_err().contains("100 KB"));
    }
    #[test]
    fn cancelled_request_never_reads_files() {
        let r = request();
        r.cancel.cancel();
        assert!(execute(&r).unwrap_err().contains("cancelled"));
    }
}
