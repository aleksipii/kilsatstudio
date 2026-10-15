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
            fs::rename(&temporary, self.root.join(filename)).map_err(err)?;
            fs::File::open(&self.root)
                .and_then(|f| f.sync_all())
                .map_err(err)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
    }
}
