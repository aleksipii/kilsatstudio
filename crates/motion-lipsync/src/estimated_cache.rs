//! Immutable saved estimated results, selected by their full content identity.
//! Loading never locates or executes an aligner and never implies quality approval.
use crate::{rhubarb::EstimatedVisemeAlignment, AlignmentError};
use motion_audio::AudioBuffer;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_BYTES: u64 = 4 * 1024 * 1024;
const DOMAIN: &[u8] = b"kilsat-saved-estimated-cues-v1\0";
static SERIAL: AtomicU64 = AtomicU64::new(0);

fn error(e: impl std::fmt::Display) -> AlignmentError {
    AlignmentError(format!("saved estimated cues: {e}"))
}
fn identity(alignment: &EstimatedVisemeAlignment) -> Result<String, AlignmentError> {
    let mut digest = Sha256::new();
    digest.update(DOMAIN);
    digest.update(serde_json::to_vec(alignment).map_err(error)?);
    Ok(format!("{:x}", digest.finalize()))
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    schema_version: u32,
    alignment: EstimatedVisemeAlignment,
}
struct OwnedTemporary(PathBuf);
impl Drop for OwnedTemporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Explicit saved-result storage, rather than memoization of new engine execution.
/// Keys cover the complete canonical alignment, including cues and tool provenance.
pub struct EstimatedCache {
    root: PathBuf,
}
impl EstimatedCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    fn path(&self, key: &str) -> Result<PathBuf, AlignmentError> {
        if key.len() != 64
            || !key
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(error("expected a 64-character lowercase SHA-256 key"));
        }
        Ok(self.root.join(format!("{key}.json")))
    }
    pub fn load(
        &self,
        key: &str,
        audio: &AudioBuffer,
        transcript: &str,
    ) -> Result<EstimatedVisemeAlignment, AlignmentError> {
        let path = self.path(key)?;
        let metadata = fs::symlink_metadata(&path).map_err(error)?;
        if !metadata.is_file() || metadata.len() > MAX_BYTES {
            return Err(error("expected a regular cache entry within 4 MiB"));
        }
        let file = File::open(path).map_err(error)?;
        let metadata = file.metadata().map_err(error)?;
        if !metadata.is_file() || metadata.len() > MAX_BYTES {
            return Err(error("expected a regular cache entry within 4 MiB"));
        }
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(error)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(error("cache entry exceeds 4 MiB"));
        }
        let entry: Entry = serde_json::from_slice(&bytes).map_err(error)?;
        if entry.schema_version != 1 || identity(&entry.alignment)? != key {
            return Err(error("cache schema or content identity mismatch"));
        }
        entry.alignment.validate_for(audio, transcript)?;
        Ok(entry.alignment)
    }
    pub fn store(
        &self,
        alignment: &EstimatedVisemeAlignment,
        audio: &AudioBuffer,
        transcript: &str,
    ) -> Result<String, AlignmentError> {
        alignment.validate_for(audio, transcript)?;
        let key = identity(alignment)?;
        let destination = self.path(&key)?;
        let bytes = serde_json::to_vec(&Entry {
            schema_version: 1,
            alignment: alignment.clone(),
        })
        .map_err(error)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(error("cache entry exceeds 4 MiB"));
        }
        fs::create_dir_all(&self.root).map_err(error)?;
        // Only a successfully exclusively created staging path belongs to us.
        // A pre-existing name must never be removed, including on failed creation.
        let (temporary, mut file) = (0..100)
            .find_map(|_| {
                let path = self.root.join(format!(
                    ".{key}.{}.{}.tmp",
                    std::process::id(),
                    SERIAL.fetch_add(1, Ordering::Relaxed)
                ));
                match OpenOptions::new().write(true).create_new(true).open(&path) {
                    Ok(file) => Some(Ok((OwnedTemporary(path), file))),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => None,
                    Err(e) => Some(Err(error(e))),
                }
            })
            .ok_or_else(|| error("could not reserve a temporary file"))??;
        file.write_all(&bytes).map_err(error)?;
        file.sync_all().map_err(error)?;
        drop(file);
        match fs::hard_link(&temporary.0, destination) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // An identical publisher is idempotent. Corrupt or unknown existing
                // entries remain untouched and are reported rather than repaired.
                let winner = self.load(&key, audio, transcript)?;
                if serde_json::to_vec(&winner).map_err(error)?
                    != serde_json::to_vec(alignment).map_err(error)?
                {
                    return Err(error("existing saved result differs"));
                }
            }
            Err(e) => return Err(error(e)),
        }
        drop(temporary);
        File::open(&self.root)
            .and_then(|directory| directory.sync_all())
            .map_err(error)?;
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhubarb::{pcm_identity, MouthCue, Recognizer};
    struct Root(PathBuf);
    impl Root {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "estimated-cache-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            )))
        }
        fn cache(&self) -> EstimatedCache {
            EstimatedCache::new(self.0.clone())
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> (AudioBuffer, EstimatedVisemeAlignment) {
        let audio = AudioBuffer {
            sample_rate: 24000,
            channels: 1,
            samples: vec![0.0; 240],
        };
        let alignment = EstimatedVisemeAlignment {
            schema_version: 1,
            source: "estimated_rhubarb".into(),
            tool_version: "synthetic-test-only".into(),
            executable_sha256: "1".repeat(64),
            pcm_sha256: pcm_identity(&audio).unwrap(),
            transcript_sha256: format!("{:x}", Sha256::digest(b"hello")),
            recognizer: Recognizer::Phonetic,
            sample_rate: 24000,
            frame_count: 240,
            cues: vec![MouthCue { start_sample: 0, end_sample: 200, shape: 'A' }],
        };
        (audio, alignment)
    }
    #[test]
    fn roundtrip_is_selected_and_requires_no_executable() {
        let root = Root::new();
        let (audio, alignment) = fixture();
        let key = root.cache().store(&alignment, &audio, "hello").unwrap();
        assert_eq!(key.len(), 64);
        let loaded = root.cache().load(&key, &audio, "hello").unwrap();
        assert_eq!(serde_json::to_vec(&loaded).unwrap(), serde_json::to_vec(&alignment).unwrap());
        assert_eq!(root.cache().store(&alignment, &audio, "hello").unwrap(), key);
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
        let mut wrong = audio.clone();
        wrong.samples[0] = 0.25;
        assert!(root.cache().load(&key, &wrong, "hello").is_err());
        assert!(root.cache().load(&key, &audio, "edited").is_err());
        assert!(root.cache().store(&alignment, &wrong, "hello").is_err());
    }
    #[test]
    fn complete_provenance_and_cues_determine_distinct_keys() {
        let root = Root::new();
        let (audio, alignment) = fixture();
        let original = root.cache().store(&alignment, &audio, "hello").unwrap();
        let mut variants = vec![alignment.clone(); 4];
        variants[0].tool_version.push('2');
        variants[1].executable_sha256 = "2".repeat(64);
        variants[2].recognizer = Recognizer::PocketSphinx;
        variants[3].cues[0].end_sample = 199;
        let mut keys = std::collections::HashSet::from([original]);
        for variant in variants {
            assert!(keys.insert(root.cache().store(&variant, &audio, "hello").unwrap()));
        }
        let mut invalid = alignment;
        invalid.source = "forced".into();
        assert!(root.cache().store(&invalid, &audio, "hello").is_err());
    }
    #[test]
    fn rejects_corruption_truncation_oversize_schema_and_legacy_shape() {
        let root = Root::new();
        let (audio, alignment) = fixture();
        let cache = root.cache();
        let key = cache.store(&alignment, &audio, "hello").unwrap();
        let path = cache.path(&key).unwrap();
        let original = fs::read(&path).unwrap();
        let mut tampered: serde_json::Value = serde_json::from_slice(&original).unwrap();
        tampered["alignment"]["cues"][0]["shape"] = "B".into();
        for bytes in [serde_json::to_vec(&tampered).unwrap(), b"{".to_vec(), serde_json::to_vec(&alignment).unwrap(), vec![b' '; MAX_BYTES as usize + 1]] {
            fs::write(&path, &bytes).unwrap();
            assert!(cache.load(&key, &audio, "hello").is_err());
            assert!(cache.store(&alignment, &audio, "hello").is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes, "never replace corrupt entries");
            assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
        }
        tampered = serde_json::from_slice(&original).unwrap();
        tampered["schema_version"] = 2.into();
        fs::write(&path, serde_json::to_vec(&tampered).unwrap()).unwrap();
        assert!(cache.load(&key, &audio, "hello").is_err());
    }
    #[test]
    fn invalid_keys_and_non_regular_files_are_rejected() {
        let root = Root::new();
        let (audio, _) = fixture();
        for key in ["../escape", "/tmp/absolute", "", &"A".repeat(64), &"g".repeat(64), &"0".repeat(63)] {
            assert!(root.cache().load(key, &audio, "hello").is_err());
        }
        assert!(!root.0.exists());
        fs::create_dir_all(root.0.join(format!("{}.json", "0".repeat(64)))).unwrap();
        assert!(root.cache().load(&"0".repeat(64), &audio, "hello").is_err());
    }
    #[test]
    fn concurrent_identical_writers_publish_one_complete_result_and_clean_staging() {
        let root = Root::new();
        let (audio, alignment) = fixture();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
        let handles: Vec<_> = (0..4).map(|_| {
            let cache = root.cache();
            let audio = audio.clone();
            let alignment = alignment.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                cache.store(&alignment, &audio, "hello").unwrap()
            })
        }).collect();
        let keys: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert!(keys.iter().all(|key| key == &keys[0]));
        root.cache().load(&keys[0], &audio, "hello").unwrap();
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
    }
    #[cfg(unix)]
    #[test]
    fn symlink_target_and_unowned_files_are_preserved() {
        use std::os::unix::fs::symlink;
        let root = Root::new();
        fs::create_dir_all(&root.0).unwrap();
        let (audio, alignment) = fixture();
        let key = identity(&alignment).unwrap();
        let unknown = root.0.join("unowned");
        fs::write(&unknown, b"do not delete").unwrap();
        symlink(&unknown, root.cache().path(&key).unwrap()).unwrap();
        assert!(root.cache().load(&key, &audio, "hello").is_err());
        assert!(root.cache().store(&alignment, &audio, "hello").is_err());
        assert_eq!(fs::read(&unknown).unwrap(), b"do not delete");
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 2);
    }
}
