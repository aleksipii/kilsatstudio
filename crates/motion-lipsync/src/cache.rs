//! Content-addressed storage of explicit alignments; never estimates phone timing.
use crate::{Alignment, AlignmentError, AlignmentSource};
use motion_audio::AudioBuffer;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
const MAX_BYTES: u64 = 4 * 1024 * 1024;
static SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CacheKey {
    audio_sha256: String,
    sample_rate: u32,
    channels: u16,
    frames: u64,
    transcript: String,
    provider: String,
    version: String,
    configuration: String,
    manual_edit_identity: String,
}
impl CacheKey {
    /// Hashes exact decoded PCM bits, channel/rate metadata and all timing inputs.
    /// Configuration must include model/dictionary revisions and other aligner settings.
    pub fn new(
        audio: &AudioBuffer,
        transcript: &str,
        provider: &str,
        version: &str,
        configuration: &str,
        manual_edit_identity: &str,
    ) -> Result<Self, AlignmentError> {
        audio
            .validate()
            .map_err(|e| AlignmentError(e.to_string()))?;
        if audio.frames() == 0 {
            return Err(AlignmentError("alignment audio is empty".into()));
        }
        let mut digest = Sha256::new();
        digest.update(b"kilsat-pcm-f32-v1");
        digest.update(audio.sample_rate.to_le_bytes());
        digest.update(audio.channels.to_le_bytes());
        for sample in &audio.samples {
            digest.update(sample.to_bits().to_le_bytes());
        }
        let key = Self {
            audio_sha256: format!("{:x}", digest.finalize()),
            sample_rate: audio.sample_rate,
            channels: audio.channels,
            frames: audio.frames() as u64,
            transcript: transcript.into(),
            provider: provider.into(),
            version: version.into(),
            configuration: configuration.into(),
            manual_edit_identity: manual_edit_identity.into(),
        };
        key.validate()?;
        Ok(key)
    }
    fn validate(&self) -> Result<(), AlignmentError> {
        if self.audio_sha256.len() != 64
            || !self.audio_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || self.frames == 0
            || self.sample_rate == 0
            || self.channels == 0
            || [
                &self.transcript,
                &self.provider,
                &self.version,
                &self.configuration,
                &self.manual_edit_identity,
            ]
            .iter()
            .any(|s| s.len() > 65536)
            || self.transcript.trim().is_empty()
            || self.provider.trim().is_empty()
            || self.version.trim().is_empty()
        {
            return Err(AlignmentError(
                "invalid or oversized alignment cache identity".into(),
            ));
        }
        Ok(())
    }
    pub fn audio_sha256(&self) -> &str {
        &self.audio_sha256
    }
    fn filename(&self) -> Result<String, AlignmentError> {
        self.validate()?;
        Ok(format!(
            "{:x}.json",
            Sha256::digest(serde_json::to_vec(self).map_err(err)?)
        ))
    }
    fn check_alignment(&self, alignment: &Alignment) -> Result<(), AlignmentError> {
        alignment.validate()?;
        let duration = self.frames as f64 / self.sample_rate as f64;
        if (duration - alignment.duration_seconds).abs() > 1e-9 {
            return Err(AlignmentError(
                "cache alignment duration differs from exact audio".into(),
            ));
        }
        match &alignment.source {
            AlignmentSource::Forced { tool, version }
                if tool != &self.provider || version != &self.version =>
            {
                return Err(AlignmentError(
                    "cache provider differs from forced alignment provenance".into(),
                ))
            }
            AlignmentSource::Manual if self.manual_edit_identity.trim().is_empty() => {
                return Err(AlignmentError(
                    "manual alignment requires a manual edit identity".into(),
                ))
            }
            _ => {}
        }
        Ok(())
    }
}
/// Portable timing envelope bound to exact decoded PCM and current dialogue text.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundAlignment {
    pub schema_version: u32,
    pub key: CacheKey,
    pub alignment: Option<Alignment>,
}
impl BoundAlignment {
    pub fn new(key: CacheKey, alignment: Option<Alignment>) -> Result<Self, AlignmentError> {
        key.validate()?;
        if let Some(alignment) = &alignment {
            key.check_alignment(alignment)?;
        }
        Ok(Self {
            schema_version: 1,
            key,
            alignment,
        })
    }
}
/// Untagged compatibility input. Bare timings remain explicitly unbound.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AlignmentInput {
    Bound(BoundAlignment),
    Legacy(Alignment),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignmentStatus {
    None,
    LegacyUnbound,
    BoundImported,
    BoundCacheHit,
    BoundCacheMiss,
}
#[derive(Clone, Debug)]
pub struct ResolvedAlignment {
    pub alignment: Option<Alignment>,
    pub status: AlignmentStatus,
}
impl AlignmentInput {
    /// Resolve only exact identities; missing cache-only data keeps the mouth at rest.
    pub fn resolve(
        &self,
        audio: &AudioBuffer,
        transcript: &str,
        cache: &AlignmentCache,
    ) -> Result<ResolvedAlignment, AlignmentError> {
        let bound = match self {
            Self::Legacy(alignment) => {
                alignment.validate()?;
                audio.validate().map_err(err)?;
                if (alignment.duration_seconds - audio.duration_seconds()).abs()
                    > 1.0 / f64::from(audio.sample_rate)
                {
                    return Err(AlignmentError(
                        "legacy alignment duration differs from audio".into(),
                    ));
                }
                return Ok(ResolvedAlignment {
                    alignment: Some(alignment.clone()),
                    status: AlignmentStatus::LegacyUnbound,
                });
            }
            Self::Bound(bound) => bound,
        };
        if bound.schema_version != 1 {
            return Err(AlignmentError(
                "unsupported bound alignment schema; expected 1".into(),
            ));
        }
        bound.key.validate()?;
        let expected = CacheKey::new(
            audio,
            transcript,
            &bound.key.provider,
            &bound.key.version,
            &bound.key.configuration,
            &bound.key.manual_edit_identity,
        )?;
        if expected != bound.key {
            return Err(AlignmentError(
                "bound alignment does not match exact audio and current transcript".into(),
            ));
        }
        if let Some(alignment) = &bound.alignment {
            bound.key.check_alignment(alignment)?;
        }
        if let Some(cached) = cache.load(&bound.key)? {
            if let Some(imported) = &bound.alignment {
                if serde_json::to_vec(imported).map_err(err)?
                    != serde_json::to_vec(&cached).map_err(err)?
                {
                    return Err(AlignmentError(
                        "alignment differs from existing cache identity; use a new edit identity"
                            .into(),
                    ));
                }
            }
            return Ok(ResolvedAlignment {
                alignment: Some(cached),
                status: AlignmentStatus::BoundCacheHit,
            });
        }
        if let Some(alignment) = &bound.alignment {
            cache.store(&bound.key, alignment)?;
            Ok(ResolvedAlignment {
                alignment: Some(alignment.clone()),
                status: AlignmentStatus::BoundImported,
            })
        } else {
            Ok(ResolvedAlignment {
                alignment: None,
                status: AlignmentStatus::BoundCacheMiss,
            })
        }
    }
}
fn err(e: impl std::fmt::Display) -> AlignmentError {
    AlignmentError(format!("alignment cache: {e}"))
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    schema_version: u32,
    key: CacheKey,
    alignment_json: String,
    alignment_sha256: String,
}
/// Missing files return None; corruption, identity mismatch and I/O failure are errors.
/// Entries are bounded and atomically installed after syncing a sibling temporary file.
pub struct AlignmentCache {
    root: PathBuf,
}
impl AlignmentCache {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_owned(),
        }
    }
    pub fn load(&self, key: &CacheKey) -> Result<Option<Alignment>, AlignmentError> {
        let path = self.root.join(key.filename()?);
        let file = match fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(err(e)),
        };
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(err)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(AlignmentError("alignment cache entry exceeds 4 MiB".into()));
        }
        let entry: Entry = serde_json::from_slice(&bytes).map_err(err)?;
        if entry.schema_version != 1 || entry.key != *key {
            return Err(AlignmentError(
                "alignment cache identity/schema mismatch".into(),
            ));
        }
        let hash = format!("{:x}", Sha256::digest(entry.alignment_json.as_bytes()));
        if hash != entry.alignment_sha256 {
            return Err(AlignmentError("alignment cache integrity mismatch".into()));
        }
        let alignment: Alignment = serde_json::from_str(&entry.alignment_json).map_err(err)?;
        key.check_alignment(&alignment)?;
        Ok(Some(alignment))
    }
    pub fn store(&self, key: &CacheKey, alignment: &Alignment) -> Result<(), AlignmentError> {
        key.validate()?;
        key.check_alignment(alignment)?;
        if alignment.phonemes.len() > 100_000 {
            return Err(AlignmentError("too many cached phonemes".into()));
        }
        let hash = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(alignment).map_err(err)?)
        );
        let entry = Entry {
            schema_version: 1,
            key: key.clone(),
            alignment_json: serde_json::to_string(alignment).map_err(err)?,
            alignment_sha256: hash,
        };
        let bytes = serde_json::to_vec(&entry).map_err(err)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(AlignmentError("alignment cache entry exceeds 4 MiB".into()));
        }
        fs::create_dir_all(&self.root).map_err(err)?;
        let filename = key.filename()?;
        let temporary = self.root.join(format!(
            ".{filename}.{}.{}.tmp",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(err)?;
            file.write_all(&bytes).map_err(err)?;
            file.sync_all().map_err(err)?;
            // Linking a fully synced sibling atomically publishes it without replacing
            // another process's winner. A rename would silently overwrite that entry.
            match fs::hard_link(&temporary, self.root.join(filename)) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let cached = self.load(key)?.ok_or_else(|| {
                        AlignmentError(
                            "alignment cache winner disappeared during publication".into(),
                        )
                    })?;
                    if serde_json::to_vec(&cached).map_err(err)?
                        != serde_json::to_vec(alignment).map_err(err)?
                    {
                        return Err(AlignmentError("alignment differs from existing cache identity; use a new edit identity".into()));
                    }
                    Ok(())
                }
                Err(e) => Err(err(e)),
            }
        })();
        // The published name owns its own hard link, so staging can always be removed.
        let cleanup = fs::remove_file(&temporary);
        result?;
        cleanup.map_err(err)?;
        fs::File::open(&self.root)
            .and_then(|f| f.sync_all())
            .map_err(err)?;
        Ok(())
    }
}

#[cfg(test)]
mod bound_tests {
    use super::*;
    use crate::TimedPhoneme;
    struct Root(PathBuf);
    impl Root {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "bound-alignment-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            )))
        }
        fn cache(&self) -> AlignmentCache {
            AlignmentCache::new(&self.0)
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> (AudioBuffer, BoundAlignment) {
        let audio = AudioBuffer {
            sample_rate: 10,
            channels: 1,
            samples: vec![0.0; 10],
        };
        let key = CacheKey::new(
            &audio,
            "hello",
            "measured",
            "1",
            "dictionary-rev-1",
            "edit-1",
        )
        .unwrap();
        let alignment = Alignment {
            source: AlignmentSource::Forced {
                tool: "measured".into(),
                version: "1".into(),
            },
            duration_seconds: 1.0,
            phonemes: vec![TimedPhoneme {
                phoneme: "m".into(),
                start_seconds: 0.1,
                end_seconds: 0.5,
            }],
        };
        (audio, BoundAlignment::new(key, Some(alignment)).unwrap())
    }
    #[test]
    fn concurrent_conflicting_writers_preserve_single_immutable_winner() {
        let root = Root::new();
        let (_, bound) = fixture();
        let key = bound.key;
        let first = bound.alignment.unwrap();
        let mut second = first.clone();
        second.phonemes[0].end_seconds = 0.6;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let writers = [first, second].map(|alignment| {
            let barrier = barrier.clone();
            let cache = root.cache();
            let key = key.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let result = cache.store(&key, &alignment);
                (alignment, result)
            })
        });
        let [a, b] = writers.map(|writer| writer.join().unwrap());
        assert_ne!(
            a.1.is_ok(),
            b.1.is_ok(),
            "exactly one conflicting writer wins"
        );
        let winner = if a.1.is_ok() { &a.0 } else { &b.0 };
        let stored = root.cache().load(&key).unwrap().unwrap();
        assert_eq!(
            serde_json::to_vec(&stored).unwrap(),
            serde_json::to_vec(winner).unwrap()
        );
        // Identical subsequent publishers are idempotent; different timings still fail.
        root.cache().store(&key, winner).unwrap();
        assert_eq!(
            fs::read_dir(&root.0).unwrap().count(),
            1,
            "no staging links remain"
        );
    }
    #[test]
    fn bound_import_repeat_and_cache_only_use_exact_hit() {
        let root = Root::new();
        let cache = root.cache();
        let (audio, bound) = fixture();
        let input = AlignmentInput::Bound(bound.clone());
        assert_eq!(
            input.resolve(&audio, "hello", &cache).unwrap().status,
            AlignmentStatus::BoundImported
        );
        assert_eq!(
            input.resolve(&audio, "hello", &cache).unwrap().status,
            AlignmentStatus::BoundCacheHit
        );
        let only = AlignmentInput::Bound(BoundAlignment::new(bound.key, None).unwrap());
        let hit = only.resolve(&audio, "hello", &cache).unwrap();
        assert_eq!(hit.status, AlignmentStatus::BoundCacheHit);
        assert_eq!(hit.alignment.unwrap().phonemes.len(), 1);
        let missing_root = Root::new();
        let miss = only
            .resolve(&audio, "hello", &missing_root.cache())
            .unwrap();
        assert_eq!(miss.status, AlignmentStatus::BoundCacheMiss);
        assert!(miss.alignment.is_none());
        assert!(!missing_root.0.exists());
    }
    #[test]
    fn bound_rejects_same_size_pcm_and_current_text_changes() {
        let root = Root::new();
        let (mut audio, bound) = fixture();
        let input = AlignmentInput::Bound(bound);
        audio.samples[3] = 0.25;
        assert!(input.resolve(&audio, "hello", &root.cache()).is_err());
        audio.samples[3] = 0.0;
        assert!(input.resolve(&audio, "edited text", &root.cache()).is_err());
        assert!(!root.0.exists());
    }
    #[test]
    fn bound_rejects_corruption_and_conflicting_edit_identity() {
        let root = Root::new();
        let cache = root.cache();
        let (audio, mut bound) = fixture();
        AlignmentInput::Bound(bound.clone())
            .resolve(&audio, "hello", &cache)
            .unwrap();
        bound.alignment.as_mut().unwrap().phonemes[0].end_seconds = 0.6;
        assert!(AlignmentInput::Bound(bound.clone())
            .resolve(&audio, "hello", &cache)
            .is_err());
        fs::write(root.0.join(bound.key.filename().unwrap()), b"broken").unwrap();
        bound.alignment = None;
        assert!(AlignmentInput::Bound(bound)
            .resolve(&audio, "hello", &cache)
            .is_err());
    }
    #[test]
    fn bound_rejects_schema_provenance_duration_and_unknown_fields() {
        let root = Root::new();
        let cache = root.cache();
        let (audio, mut bound) = fixture();
        bound.schema_version = 2;
        assert!(AlignmentInput::Bound(bound.clone())
            .resolve(&audio, "hello", &cache)
            .is_err());
        bound.schema_version = 1;
        bound.alignment.as_mut().unwrap().source = AlignmentSource::Forced {
            tool: "different".into(),
            version: "1".into(),
        };
        assert!(AlignmentInput::Bound(bound.clone())
            .resolve(&audio, "hello", &cache)
            .is_err());
        let (_, mut bound) = fixture();
        bound.alignment.as_mut().unwrap().duration_seconds = 2.0;
        assert!(BoundAlignment::new(bound.key.clone(), bound.alignment.clone()).is_err());
        let (_, bound) = fixture();
        let mut value = serde_json::to_value(bound).unwrap();
        value["unexpected"] = true.into();
        assert!(serde_json::from_value::<AlignmentInput>(value).is_err());
        assert!(!root.0.exists());
    }
    #[test]
    fn legacy_stays_unbound_and_never_writes_cache() {
        let root = Root::new();
        let (audio, bound) = fixture();
        let legacy = AlignmentInput::Legacy(bound.alignment.unwrap());
        let json = serde_json::to_string(&legacy).unwrap();
        let decoded: AlignmentInput = serde_json::from_str(&json).unwrap();
        assert_eq!(
            decoded
                .resolve(&audio, "text is not bound", &root.cache())
                .unwrap()
                .status,
            AlignmentStatus::LegacyUnbound
        );
        assert!(!root.0.exists());
        // Preserve the prior legacy import tolerance of one audio sample.
        let (_, bound) = fixture();
        let mut rounded = bound.alignment.unwrap();
        rounded.duration_seconds += 0.5 / f64::from(audio.sample_rate);
        assert!(AlignmentInput::Legacy(rounded.clone())
            .resolve(&audio, "unbound", &root.cache())
            .is_ok());
        rounded.duration_seconds += 1.0 / f64::from(audio.sample_rate);
        assert!(AlignmentInput::Legacy(rounded)
            .resolve(&audio, "unbound", &root.cache())
            .is_err());
        let (_, mut bound) = fixture();
        bound.key.manual_edit_identity.clear();
        bound.alignment.as_mut().unwrap().source = AlignmentSource::Manual;
        assert!(AlignmentInput::Bound(bound)
            .resolve(&audio, "hello", &root.cache())
            .is_err());
    }
}
