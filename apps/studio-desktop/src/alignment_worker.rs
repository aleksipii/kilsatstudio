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
#[derive(Clone, Debug)]
pub enum Action {
    Estimate,
    EstimateFresh,
    EnrollAndEstimate {
        expected_version: String,
    },
    Load {
        key: String,
    },
    Manual {
        original: Box<motion_lipsync::rhubarb::EstimatedVisemeAlignment>,
        cues: Vec<motion_lipsync::rhubarb::MouthCue>,
    },
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResultOrigin {
    #[default]
    Fresh,
    ExplicitArchiveLoad,
    AutomaticReuse,
    Manual,
}
#[derive(Debug, Default)]
pub struct CacheInfo {
    pub key: Option<String>,
    pub origin: ResultOrigin,
    pub message: String,
    pub provenance: String,
}
pub struct Request {
    pub cache_root: PathBuf,
    pub action: Action,
    pub identity: Identity,
    pub audio: Arc<dyn PreparedAudio>,
    pub scene_alignments: String,
    pub executable: PathBuf,
    pub recognizer: Recognizer,
    pub cancel: CancellationToken,
}
#[derive(Debug)]
pub struct Ready {
    pub cache: CacheInfo,
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
    if let Action::Manual { original, cues } = &request.action {
        let bound = motion_lipsync::manual::bind_edits(
            original,
            cues,
            request.audio.audio(),
            &request.identity.transcript,
        )
        .map_err(|e| e.to_string())?;
        request.cancel.check().map_err(|e| e.to_string())?;
        let input = motion_lipsync::cache::AlignmentInput::Bound(bound);
        let json = serde_json::to_string_pretty(&input).map_err(|e| e.to_string())?;
        scene_alignments.insert(request.identity.line_id, input);
        let scene_json =
            serde_json::to_string_pretty(&scene_alignments).map_err(|e| e.to_string())?;
        if json.len() > MAX_JSON_BYTES || scene_json.len() > MAX_JSON_BYTES {
            return Err("Manual timing exceeds the editor's 100 KB input limit".into());
        }
        request.cancel.check().map_err(|e| e.to_string())?;
        return Ok(Ready {
            cache: CacheInfo {
                origin: ResultOrigin::Manual,
                message: "Manual mouth-shape edits; no recognizer ran and no automatic estimate was overwritten.".into(),
                provenance: format!("Edited from {} / {:?}; representative shape labels are not measured phonemes.", original.tool_version, original.recognizer),
                ..Default::default()
            },
            identity: request.identity.clone(),
            json,
            scene_json,
        });
    }
    let archive = motion_lipsync::estimated_cache::EstimatedCache::new(request.cache_root.clone());
    let cancel = || request.cancel.is_cancelled();
    if let Action::EnrollAndEstimate { expected_version } = &request.action {
        if request.recognizer != Recognizer::Phonetic {
            return Err("Local package enrollment requires Phonetic.".into());
        }
        motion_lipsync::local_package::enroll(&request.executable, expected_version, &cancel)
            .map_err(|e| e.to_string())?;
    }
    let mut package = None;
    let mut fingerprint = None;
    let index = motion_lipsync::engine_reuse::EngineIndex::new(request.cache_root.clone());
    let (alignment, mut cache) = match &request.action {
        Action::Estimate | Action::EstimateFresh | Action::EnrollAndEstimate { .. } => {
            let inspection = motion_lipsync::local_package::inspect(
                &request.executable,
                request.recognizer,
                &cancel,
            )
            .map_err(|e| e.to_string())?;
            match inspection {
                motion_lipsync::local_package::Inspection::Unavailable(reason) => (
                    rhubarb::align(
                        &request.executable,
                        &request.identity.audio_path,
                        &request.identity.transcript,
                        request.recognizer,
                        cancel,
                    )
                    .map_err(|e| e.to_string())?,
                    CacheInfo {
                        message: reason,
                        ..Default::default()
                    },
                ),
                motion_lipsync::local_package::Inspection::Eligible(local) => {
                    let identity = motion_lipsync::engine_reuse::RequestIdentity::new(
                        request.audio.audio(),
                        &request.identity.transcript,
                        request.recognizer,
                        &cancel,
                    )
                    .map_err(|e| e.to_string())?;
                    let cached = if matches!(request.action, Action::EstimateFresh) {
                        None
                    } else {
                        index.lookup(&identity, local.environment(),
                            request.audio.audio(), &request.identity.transcript, &cancel)
                            .map_err(|e| format!("Automatic saved timing: {e}. The existing entry was preserved; Run fresh bypasses the index."))?
                    };
                    local.recheck(&cancel).map_err(|e| e.to_string())?;
                    let (alignment, origin) = match cached {
                        Some(value) => (value, ResultOrigin::AutomaticReuse),
                        None => (
                            local
                                .execute(
                                    &request.identity.audio_path,
                                    &request.identity.transcript,
                                    &cancel,
                                )
                                .map_err(|e| e.to_string())?,
                            ResultOrigin::Fresh,
                        ),
                    };
                    package = Some(local);
                    fingerprint = Some(identity);
                    (alignment, CacheInfo { origin,
                        message: "Explicitly enrolled local package identity. Estimated timing; package trust does not establish quality, licensing or OS isolation.".into(),
                        ..Default::default()
                    })
                }
            }
        }
        Action::Load { key } => (
            archive
                .load(key, request.audio.audio(), &request.identity.transcript)
                .map_err(|e| format!("Saved estimated timing: {e}"))?,
            CacheInfo {
                key: Some(key.clone()),
                origin: ResultOrigin::ExplicitArchiveLoad,
                message: String::new(),
                provenance: String::new(),
            },
        ),
        Action::Manual { .. } => return Err("Manual timing was not handled".into()),
    };
    cache.provenance = format!(
        "Original tool: {}; recognizer: {:?}",
        alignment.tool_version, alignment.recognizer
    );
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
        motion_lipsync::cache::AlignmentInput::Estimated(alignment.clone()),
    );
    let scene_json = serde_json::to_string_pretty(&scene_alignments).map_err(|e| e.to_string())?;
    if scene_json.len() > MAX_JSON_BYTES {
        return Err("Combined scene alignment exceeds 100 KB; shorten dialogue or remove unused alignment entries and retry".into());
    }
    request.cancel.check().map_err(|e| e.to_string())?;
    if !matches!(request.action, Action::Load { .. }) {
        match archive.store(
            &alignment,
            request.audio.audio(),
            &request.identity.transcript,
        ) {
            Ok(key) => cache.key = Some(key),
            Err(error) => cache
                .message
                .push_str(&format!(" Timing is usable, but saving it failed: {error}")),
        }
    }
    if let (Some(package), Some(identity)) = (&package, &fingerprint) {
        package.recheck(&cancel).map_err(|e| e.to_string())?;
        if cache.origin == ResultOrigin::Fresh && !matches!(request.action, Action::EstimateFresh) {
            index
                .publish(
                    identity,
                    package.environment(),
                    &alignment,
                    request.audio.audio(),
                    &request.identity.transcript,
                    &cancel,
                )
                .map_err(|e| format!("Automatic timing publication: {e}"))?;
        }
        package.recheck(&cancel).map_err(|e| e.to_string())?;
    }
    request.cancel.check().map_err(|e| e.to_string())?;
    Ok(Ready {
        cache,
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
        if !matches!(request.action, Action::Load { .. } | Action::Manual { .. })
            && !request.executable.is_absolute()
        {
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
            cache_root: std::env::temp_dir().join("not-written-estimated-cache"),
            action: Action::Estimate,
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
    fn manual_edits_need_no_paths_or_recognizer_and_preserve_other_lines() {
        use sha2::{Digest, Sha256};
        let mut r = request();
        r.executable = PathBuf::new();
        r.cache_root = PathBuf::from("/not-created-manual-test");
        let original = motion_lipsync::rhubarb::EstimatedVisemeAlignment {
            schema_version: 1,
            source: "estimated_rhubarb".into(),
            tool_version: "test-only".into(),
            executable_sha256: "a".repeat(64),
            pcm_sha256: motion_lipsync::rhubarb::pcm_identity(r.audio.audio()).unwrap(),
            transcript_sha256: format!("{:x}", Sha256::digest(r.identity.transcript.as_bytes())),
            recognizer: Recognizer::Phonetic,
            sample_rate: 24000,
            frame_count: 24,
            cues: vec![],
        };
        let other = motion_lipsync::cache::AlignmentInput::Estimated(original.clone());
        r.scene_alignments =
            serde_json::to_string(&std::collections::BTreeMap::from([(99, other)])).unwrap();
        r.action = Action::Manual {
            original: Box::new(original),
            cues: vec![motion_lipsync::rhubarb::MouthCue {
                start_sample: 3,
                end_sample: 12,
                shape: 'G',
            }],
        };
        let ready = execute(&r).unwrap();
        assert_eq!(ready.cache.origin, ResultOrigin::Manual);
        assert!(ready.cache.key.is_none());
        let map: std::collections::BTreeMap<u64, motion_lipsync::cache::AlignmentInput> =
            serde_json::from_str(&ready.scene_json).unwrap();
        assert!(map.contains_key(&99));
        assert!(matches!(
            map.get(&r.identity.line_id),
            Some(motion_lipsync::cache::AlignmentInput::Bound(_))
        ));
        let input: motion_lipsync::cache::AlignmentInput =
            serde_json::from_str(&ready.json).unwrap();
        let motion_lipsync::cache::AlignmentInput::Bound(bound) = input else {
            panic!("manual result was not bound");
        };
        assert!(matches!(
            bound.alignment.unwrap().source,
            motion_lipsync::AlignmentSource::Manual
        ));
        r.audio = Arc::new(AudioBuffer {
            sample_rate: 24000,
            channels: 1,
            samples: vec![0.5; 24],
        });
        assert!(execute(&r).unwrap_err().contains("PCM"));
        r.cancel.cancel();
        assert!(execute(&r).is_err());
    }
    #[test]
    fn bounded_until_completion_is_consumed() {
        let mut worker = Worker::with_runner(|r| {
            Ok(Ready {
                cache: CacheInfo::default(),
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
    fn cached_request_does_not_require_an_executable() {
        let mut worker = Worker::with_runner(|r| {
            assert!(matches!(&r.action, Action::Load { key } if key == &"a".repeat(64)));
            Ok(Ready {
                cache: CacheInfo::default(),
                identity: r.identity.clone(),
                json: "saved".into(),
                scene_json: "{}".into(),
            })
        });
        let mut r = request();
        r.executable = PathBuf::new();
        r.action = Action::Load {
            key: "a".repeat(64),
        };
        worker.submit(r).unwrap();
        for _ in 0..1000 {
            if let Some(update) = worker.poll() {
                assert_eq!(update.result.unwrap().json, "saved");
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("cached request did not finish");
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
        r.cache_root = path.join("saved");
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
        let original_root = r.cache_root.clone();
        r.cache_root = path.join("blocked-cache");
        std::fs::write(&r.cache_root, b"file blocks directory").unwrap();
        let usable = execute(&r).unwrap();
        assert!(usable.cache.message.contains("saving it failed"));
        assert!(usable.cache.key.is_none());
        r.cache_root = original_root;
        let key = ready.cache.key.clone().unwrap();
        std::fs::remove_file(&r.executable).unwrap();
        std::fs::remove_file(&r.identity.audio_path).unwrap();
        r.executable = PathBuf::new();
        r.action = Action::Load { key };
        let saved = execute(&r).unwrap();
        assert_eq!(saved.cache.origin, ResultOrigin::ExplicitArchiveLoad);
        assert_eq!(saved.json, ready.json);
        r.identity.transcript = "changed".into();
        assert!(execute(&r).is_err());
        r.identity.transcript = "hello".into();
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
    #[cfg(unix)]
    #[test]
    fn enrolled_exact_hit_needs_neither_an_executable_process_nor_the_wav_path() {
        use sha2::{Digest, Sha256};
        let path = std::env::temp_dir().join(format!(
            "automatic-estimate-hit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let root = Cleanup(std::fs::canonicalize(path).unwrap());
        let package_root = root.0.join("package");
        std::fs::create_dir(&package_root).unwrap();
        let mut r = request();
        r.cache_root = root.0.join("archive");
        r.executable = package_root.join("rhubarb");
        // Structurally native identity fixture, deliberately not executable.
        // Any attempted --version/recognition launch makes this test fail.
        let mut binary = vec![0u8; 64];
        if std::env::consts::OS == "macos" {
            binary[..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
            binary[4..8].copy_from_slice(
                &(if std::env::consts::ARCH == "aarch64" {
                    0x0100000cu32
                } else {
                    0x01000007u32
                })
                .to_le_bytes(),
            );
            binary[12..16].copy_from_slice(&2u32.to_le_bytes());
        } else {
            binary[..6].copy_from_slice(b"\x7fELF\x02\x01");
            binary[18..20].copy_from_slice(
                &(if std::env::consts::ARCH == "aarch64" {
                    183u16
                } else {
                    62u16
                })
                .to_le_bytes(),
            );
            binary[54..56].copy_from_slice(&56u16.to_le_bytes());
        }
        std::fs::write(&r.executable, &binary).unwrap();
        motion_lipsync::local_package::enroll(&r.executable, "synthetic-no-process", &|| false)
            .unwrap();
        let motion_lipsync::local_package::Inspection::Eligible(package) =
            motion_lipsync::local_package::inspect(&r.executable, r.recognizer, &|| false).unwrap()
        else {
            panic!("fixture enrollment")
        };
        let fingerprint = motion_lipsync::engine_reuse::RequestIdentity::new(
            r.audio.audio(),
            &r.identity.transcript,
            r.recognizer,
            &|| false,
        )
        .unwrap();
        let alignment = rhubarb::EstimatedVisemeAlignment {
            schema_version: 1,
            source: "estimated_rhubarb".into(),
            tool_version: "synthetic-no-process".into(),
            executable_sha256: format!("{:x}", Sha256::digest(&binary)),
            pcm_sha256: rhubarb::pcm_identity(r.audio.audio()).unwrap(),
            transcript_sha256: format!("{:x}", Sha256::digest(r.identity.transcript.as_bytes())),
            recognizer: r.recognizer,
            sample_rate: 24000,
            frame_count: 24,
            cues: vec![rhubarb::MouthCue {
                start_sample: 0,
                end_sample: 24,
                shape: 'A',
            }],
        };
        motion_lipsync::engine_reuse::EngineIndex::new(&r.cache_root)
            .publish(
                &fingerprint,
                package.environment(),
                &alignment,
                r.audio.audio(),
                &r.identity.transcript,
                &|| false,
            )
            .unwrap();
        let ready = execute(&r).unwrap();
        assert_eq!(ready.cache.origin, ResultOrigin::AutomaticReuse);
        assert!(ready.cache.provenance.contains("synthetic-no-process"));
        assert_eq!(
            serde_json::from_str::<rhubarb::EstimatedVisemeAlignment>(&ready.json)
                .unwrap()
                .cues,
            alignment.cues
        );
        let index_path = std::fs::read_dir(r.cache_root.join("engine-index-v1"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        std::fs::write(&index_path, b"corrupt").unwrap();
        assert!(execute(&r).unwrap_err().contains("Automatic saved timing"));
        assert_eq!(std::fs::read(index_path).unwrap(), b"corrupt");
    }
}
