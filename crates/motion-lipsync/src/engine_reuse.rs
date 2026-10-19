//! Input-to-result index foundation. No production package profile is verified yet.
//! A request fingerprint is not evidence of a complete execution environment.
use crate::{
    estimated_cache::EstimatedCache,
    rhubarb::{EstimatedVisemeAlignment, Recognizer},
    AlignmentError,
};
use motion_audio::AudioBuffer;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
const MAX_INDEX: u64 = 16 * 1024;
static SERIAL: AtomicU64 = AtomicU64::new(0);
fn error(message: impl std::fmt::Display) -> AlignmentError {
    AlignmentError(format!("estimated engine index: {message}"))
}
fn check(cancel: &dyn Fn() -> bool) -> Result<(), AlignmentError> {
    if cancel() {
        Err(error("cancelled"))
    } else {
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestIdentity {
    pcm_sha256: String,
    transcript_sha256: String,
    recognizer: Recognizer,
    adapter: String,
    platform: String,
}
impl RequestIdentity {
    pub fn new(
        audio: &AudioBuffer,
        text: &str,
        recognizer: Recognizer,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Self, AlignmentError> {
        check(cancel)?;
        if text.len() > 65536 || text.contains('\0') {
            return Err(error("invalid transcript"));
        }
        audio.validate().map_err(error)?;
        if audio.sample_rate != 24000
            || audio.channels != 1
            || audio.frames() == 0
            || audio.frames() > 24000 * 300
        {
            return Err(error(
                "requires nonempty mono 24 kHz audio within five minutes",
            ));
        }
        let mut hash = Sha256::new();
        hash.update(audio.sample_rate.to_le_bytes());
        hash.update(audio.channels.to_le_bytes());
        hash.update((audio.frames() as u64).to_le_bytes());
        for chunk in audio.samples.chunks(1024) {
            check(cancel)?;
            for sample in chunk {
                hash.update(sample.to_bits().to_le_bytes());
            }
        }
        check(cancel)?;
        Ok(Self {
            pcm_sha256: format!("{:x}", hash.finalize()),
            transcript_sha256: format!("{:x}", Sha256::digest(text.as_bytes())),
            recognizer,
            adapter: "rhubarb-json-ghx-threads2-converter1".into(),
            platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        })
    }
    fn validate_result(&self, value: &EstimatedVisemeAlignment) -> Result<(), AlignmentError> {
        if self.pcm_sha256 != value.pcm_sha256
            || self.transcript_sha256 != value.transcript_sha256
            || self.recognizer != value.recognizer
        {
            return Err(error("result does not match exact request"));
        }
        Ok(())
    }
}
/// Private fields deliberately prevent callers from asserting environment verification.
/// Only a reviewed package inspector may eventually construct production tokens.
pub struct VerifiedEnvironment {
    manifest: String,
    executable: String,
    version: String,
}
impl VerifiedEnvironment {
    pub(crate) fn from_inspected_package(manifest: String, executable: String, version: String) -> Self {
        Self { manifest, executable, version }
    }
}
/// Unknown packages remain eligible for explicit execution, never automatic reuse.
pub fn automatic_reuse_unavailable() -> &'static str {
    "Automatic reuse unavailable: no inspected Rhubarb resource and dependency profile is installed. Explicit Estimate executes the recognizer; saved timing can be loaded by key."
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    schema_version: u32,
    request: RequestIdentity,
    environment: String,
    result_key: String,
}
pub struct EngineIndex {
    root: PathBuf,
    archive: EstimatedCache,
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
impl EngineIndex {
    pub fn new(archive_root: impl Into<PathBuf>) -> Self {
        let root = archive_root.into();
        Self {
            root: root.join("engine-index-v1"),
            archive: EstimatedCache::new(root),
        }
    }
    fn path(
        &self,
        request: &RequestIdentity,
        environment: &VerifiedEnvironment,
    ) -> Result<PathBuf, AlignmentError> {
        let mut hash = Sha256::new();
        hash.update(b"kilsat-estimated-engine-index-v1\0");
        hash.update(serde_json::to_vec(&(request, &environment.manifest)).map_err(error)?);
        Ok(self.root.join(format!("{:x}.json", hash.finalize())))
    }
    pub fn lookup(
        &self,
        request: &RequestIdentity,
        environment: &VerifiedEnvironment,
        audio: &AudioBuffer,
        text: &str,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Option<EstimatedVisemeAlignment>, AlignmentError> {
        check(cancel)?;
        if RequestIdentity::new(audio, text, request.recognizer, cancel)? != *request {
            return Err(error("request changed"));
        }
        let path = self.path(request, environment)?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(value) => value,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(error(e)),
        };
        if !metadata.is_file() || metadata.len() > MAX_INDEX {
            return Err(error("expected regular index within 16 KiB"));
        }
        check(cancel)?;
        let file = fs::File::open(&path).map_err(error)?;
        if !file.metadata().map_err(error)?.is_file() {
            return Err(error("index is not regular"));
        }
        let mut bytes = Vec::new();
        file.take(MAX_INDEX + 1)
            .read_to_end(&mut bytes)
            .map_err(error)?;
        check(cancel)?;
        if bytes.len() as u64 > MAX_INDEX {
            return Err(error("index exceeds 16 KiB"));
        }
        let entry: Entry = serde_json::from_slice(&bytes).map_err(error)?;
        if entry.schema_version != 1
            || entry.request != *request
            || entry.environment != environment.manifest
        {
            return Err(error("index identity/schema mismatch"));
        }
        let value = self.archive.load(&entry.result_key, audio, text)?;
        request.validate_result(&value)?;
        if value.executable_sha256 != environment.executable
            || value.tool_version != environment.version
        {
            return Err(error("execution provenance mismatch"));
        }
        check(cancel)?;
        Ok(Some(value))
    }
    pub fn publish(
        &self,
        request: &RequestIdentity,
        environment: &VerifiedEnvironment,
        value: &EstimatedVisemeAlignment,
        audio: &AudioBuffer,
        text: &str,
        cancel: &dyn Fn() -> bool,
    ) -> Result<(), AlignmentError> {
        check(cancel)?;
        request.validate_result(value)?;
        if RequestIdentity::new(audio, text, request.recognizer, cancel)? != *request
            || value.executable_sha256 != environment.executable
            || value.tool_version != environment.version
        {
            return Err(error("request or execution provenance changed"));
        }
        let result_key = self.archive.store(value, audio, text)?;
        check(cancel)?;
        let bytes = serde_json::to_vec(&Entry {
            schema_version: 1,
            request: request.clone(),
            environment: environment.manifest.clone(),
            result_key,
        })
        .map_err(error)?;
        if bytes.len() as u64 > MAX_INDEX {
            return Err(error("index exceeds 16 KiB"));
        }
        fs::create_dir_all(&self.root).map_err(error)?;
        let path = self.path(request, environment)?;
        let temporary_path = self.root.join(format!(
            ".{}.{}.tmp",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(error)?;
        let temporary = Temporary(temporary_path);
        file.write_all(&bytes).map_err(error)?;
        file.sync_all().map_err(error)?;
        drop(file);
        check(cancel)?;
        match fs::hard_link(&temporary.0, &path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let winner = self
                    .lookup(request, environment, audio, text, cancel)?
                    .ok_or_else(|| error("index disappeared"))?;
                if serde_json::to_vec(&winner).map_err(error)?
                    != serde_json::to_vec(value).map_err(error)?
                {
                    return Err(error(
                        "conflicting result preserved; automatic selection refused",
                    ));
                }
            }
            Err(e) => return Err(error(e)),
        }
        fs::File::open(&self.root)
            .and_then(|file| file.sync_all())
            .map_err(error)?;
        check(cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhubarb::MouthCue;
    struct Root(PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> (
        Root,
        AudioBuffer,
        RequestIdentity,
        VerifiedEnvironment,
        EstimatedVisemeAlignment,
    ) {
        let root = Root(std::env::temp_dir().join(format!(
            "engine-index-test-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        )));
        let audio = AudioBuffer {
            sample_rate: 24000,
            channels: 1,
            samples: vec![0.0; 240],
        };
        let request =
            RequestIdentity::new(&audio, "hello", Recognizer::Phonetic, &|| false).unwrap();
        let environment = VerifiedEnvironment {
            manifest: "synthetic-test-only-no-real-package".into(),
            executable: "1".repeat(64),
            version: "synthetic-test".into(),
        };
        let value = EstimatedVisemeAlignment {
            schema_version: 1,
            source: "estimated_rhubarb".into(),
            tool_version: environment.version.clone(),
            executable_sha256: environment.executable.clone(),
            pcm_sha256: request.pcm_sha256.clone(),
            transcript_sha256: request.transcript_sha256.clone(),
            recognizer: Recognizer::Phonetic,
            sample_rate: 24000,
            frame_count: 240,
            cues: vec![MouthCue {
                start_sample: 0,
                end_sample: 200,
                shape: 'A',
            }],
        };
        (root, audio, request, environment, value)
    }
    #[test]
    fn immutable_index_hits_and_preserves_conflicting_results() {
        let (root, audio, request, environment, value) = fixture();
        let index = EngineIndex::new(&root.0);
        assert!(index
            .lookup(&request, &environment, &audio, "hello", &|| false)
            .unwrap()
            .is_none());
        index
            .publish(&request, &environment, &value, &audio, "hello", &|| false)
            .unwrap();
        index
            .publish(&request, &environment, &value, &audio, "hello", &|| false)
            .unwrap();
        let before = fs::read(index.path(&request, &environment).unwrap()).unwrap();
        let mut conflict = value.clone();
        conflict.cues[0].shape = 'B';
        assert!(index
            .publish(&request, &environment, &conflict, &audio, "hello", &|| {
                false
            })
            .unwrap_err()
            .to_string()
            .contains("conflicting"));
        assert_eq!(
            fs::read(index.path(&request, &environment).unwrap()).unwrap(),
            before
        );
        assert_eq!(
            index
                .lookup(&request, &environment, &audio, "hello", &|| false)
                .unwrap()
                .unwrap()
                .cues[0]
                .shape,
            'A'
        );
    }
    #[test]
    fn corruption_and_dangling_archive_are_errors_not_misses() {
        let (root, audio, request, environment, value) = fixture();
        let index = EngineIndex::new(&root.0);
        index
            .publish(&request, &environment, &value, &audio, "hello", &|| false)
            .unwrap();
        let path = index.path(&request, &environment).unwrap();
        let original = fs::read(&path).unwrap();
        for bytes in [b"{".to_vec(), vec![b' '; MAX_INDEX as usize + 1]] {
            fs::write(&path, &bytes).unwrap();
            assert!(index
                .lookup(&request, &environment, &audio, "hello", &|| false)
                .is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        fs::write(&path, original).unwrap();
        let entry: Entry = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        fs::remove_file(root.0.join(format!("{}.json", entry.result_key))).unwrap();
        assert!(index
            .lookup(&request, &environment, &audio, "hello", &|| false)
            .is_err());
    }
    #[test]
    fn exact_inputs_and_environment_changes_invalidate() {
        let (root, audio, request, mut environment, value) = fixture();
        let index = EngineIndex::new(&root.0);
        index
            .publish(&request, &environment, &value, &audio, "hello", &|| false)
            .unwrap();
        let mut changed = audio.clone();
        changed.samples[0] = 0.1;
        assert_ne!(
            RequestIdentity::new(&changed, "hello", Recognizer::Phonetic, &|| false).unwrap(),
            request
        );
        assert_ne!(
            RequestIdentity::new(&audio, "Hello", Recognizer::Phonetic, &|| false).unwrap(),
            request
        );
        assert_ne!(
            RequestIdentity::new(&audio, "hello", Recognizer::PocketSphinx, &|| false).unwrap(),
            request
        );
        assert!(index
            .lookup(&request, &environment, &changed, "hello", &|| false)
            .is_err());
        environment.manifest.push('2');
        assert!(index
            .lookup(&request, &environment, &audio, "hello", &|| false)
            .unwrap()
            .is_none());
        assert!(automatic_reuse_unavailable().contains("no inspected"));
    }
    #[test]
    fn cancellation_during_fingerprinting_and_before_io_is_visible() {
        let (root, mut audio, request, environment, value) = fixture();
        let index = EngineIndex::new(&root.0);
        audio.samples.resize(8192, 0.0);
        let calls = std::cell::Cell::new(0);
        assert!(
            RequestIdentity::new(&audio, "hello", Recognizer::Phonetic, &|| {
                calls.set(calls.get() + 1);
                calls.get() > 3
            })
            .unwrap_err()
            .to_string()
            .contains("cancelled")
        );
        assert!(index
            .lookup(&request, &environment, &audio, "hello", &|| true)
            .is_err());
        assert!(index
            .publish(&request, &environment, &value, &audio, "hello", &|| true)
            .is_err());
        assert!(!root.0.exists());
    }
    #[test]
    fn cancellation_after_archive_and_staging_never_publishes_an_index() {
        let (root, audio, request, environment, value) = fixture();
        let index = EngineIndex::new(&root.0);
        let cancelled = || {
            fs::read_dir(&index.root).ok().is_some_and(|entries| {
                entries
                    .filter_map(Result::ok)
                    .any(|entry| entry.path().extension().is_some_and(|ext| ext == "tmp"))
            })
        };
        assert!(index
            .publish(&request, &environment, &value, &audio, "hello", &cancelled)
            .unwrap_err()
            .to_string()
            .contains("cancelled"));
        assert!(!index.path(&request, &environment).unwrap().exists());
        assert_eq!(fs::read_dir(&index.root).unwrap().count(), 0);
        // Completed immutable archive entries may remain; no candidate was delivered.
        assert!(fs::read_dir(&root.0)
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry.path().extension().is_some_and(|ext| ext == "json")));
    }
    #[test]
    fn concurrent_publishers_share_one_complete_index() {
        let (root, audio, request, environment, value) = fixture();
        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for _ in 0..4 {
                let root = &root.0;
                let audio = &audio;
                let request = &request;
                let environment = &environment;
                let value = &value;
                handles.push(scope.spawn(move || {
                    EngineIndex::new(root)
                        .publish(request, environment, value, audio, "hello", &|| false)
                        .unwrap()
                }));
            }
            for handle in handles {
                handle.join().unwrap();
            }
        });
        let index = EngineIndex::new(&root.0);
        assert!(index
            .lookup(&request, &environment, &audio, "hello", &|| false)
            .unwrap()
            .is_some());
        assert_eq!(fs::read_dir(&index.root).unwrap().count(), 1);
    }
}
