//! Opt-in local execution; Rhubarb mouth cues are estimated, never phonemes.
use crate::{AlignmentError, Viseme};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};
const MAX_WAV: u64 = 32 * 1024 * 1024;
const MAX_BINARY: u64 = 128 * 1024 * 1024;
const MAX_JSON: u64 = 4 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);
fn error(m: impl ToString) -> AlignmentError {
    AlignmentError(m.to_string())
}
fn hash(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn bounded(p: &Path, max: u64) -> Result<Vec<u8>, AlignmentError> {
    let f = File::open(p).map_err(error)?;
    if !f.metadata().map_err(error)?.is_file() {
        return Err(error("expected regular local file"));
    }
    let mut b = Vec::new();
    f.take(max + 1).read_to_end(&mut b).map_err(error)?;
    if b.len() as u64 > max {
        return Err(error("local file exceeds budget"));
    }
    Ok(b)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recognizer {
    PocketSphinx,
    Phonetic,
}
impl Recognizer {
    fn argument(self) -> &'static str {
        match self {
            Self::PocketSphinx => "pocketSphinx",
            Self::Phonetic => "phonetic",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MouthCue {
    pub start_sample: u64,
    pub end_sample: u64,
    pub shape: char,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EstimatedVisemeAlignment {
    pub schema_version: u32,
    pub source: String,
    pub tool_version: String,
    pub executable_sha256: String,
    pub pcm_sha256: String,
    pub transcript_sha256: String,
    pub recognizer: Recognizer,
    pub sample_rate: u32,
    pub frame_count: u64,
    pub cues: Vec<MouthCue>,
}
pub fn shape_viseme(s: char) -> Option<Viseme> {
    Some(match s {
        'A' => Viseme::Closed,
        'B' => Viseme::I,
        'C' => Viseme::E,
        'D' => Viseme::A,
        'E' => Viseme::O,
        'F' => Viseme::U,
        'G' => Viseme::TeethLip,
        'H' => Viseme::TongueUp,
        'X' => Viseme::Rest,
        _ => return None,
    })
}
impl EstimatedVisemeAlignment {
    pub fn validate(&self) -> Result<(), AlignmentError> {
        if self.schema_version != 1
            || self.source != "estimated_rhubarb"
            || self.tool_version.trim().is_empty()
            || self.tool_version.len() > 1024
            || self.sample_rate != 24000
            || self.frame_count == 0
            || self.frame_count > 24000 * 300
            || self.cues.len() > 100_000
        {
            return Err(error("invalid estimated alignment identity or bounds"));
        }
        for d in [
            &self.executable_sha256,
            &self.pcm_sha256,
            &self.transcript_sha256,
        ] {
            if d.len() != 64 || !d.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err(error("invalid provenance SHA-256"));
            }
        }
        let mut previous = 0;
        for c in &self.cues {
            if c.start_sample < previous
                || c.end_sample <= c.start_sample
                || c.end_sample > self.frame_count
                || shape_viseme(c.shape).is_none()
            {
                return Err(error("invalid mouth cue interval or shape"));
            }
            previous = c.end_sample;
        }
        Ok(())
    }
    /// Reject equal-duration but different PCM or changed dialogue text.
    pub fn validate_for(
        &self,
        audio: &motion_audio::AudioBuffer,
        transcript: &str,
    ) -> Result<(), AlignmentError> {
        self.validate_audio(audio)?;
        if transcript.len() > 65536 || hash(transcript.as_bytes()) != self.transcript_sha256 {
            return Err(error("estimated cues do not match current transcript"));
        }
        Ok(())
    }
    pub fn validate_audio(&self, audio: &motion_audio::AudioBuffer) -> Result<(), AlignmentError> {
        self.validate()?;
        if self.sample_rate != audio.sample_rate
            || self.frame_count != audio.frames() as u64
            || pcm_identity(audio)? != self.pcm_sha256
        {
            return Err(error("estimated cues do not match exact decoded PCM"));
        }
        Ok(())
    }
    /// Half-open intervals; uncovered samples and positions beyond audio are rest.
    pub fn viseme_at_sample(&self, sample: u64) -> Result<Viseme, AlignmentError> {
        self.validate()?;
        let i = self.cues.partition_point(|c| c.end_sample <= sample);
        Ok(self
            .cues
            .get(i)
            .filter(|c| c.start_sample <= sample)
            .and_then(|c| shape_viseme(c.shape))
            .unwrap_or(Viseme::Rest))
    }
}
/// Canonical schema-1 hash of exact decoded mono 24 kHz PCM, including format/frame count.
pub fn pcm_identity(audio: &motion_audio::AudioBuffer) -> Result<String, AlignmentError> {
    audio.validate().map_err(error)?;
    if audio.sample_rate != 24000
        || audio.channels != 1
        || audio.frames() == 0
        || audio.frames() > 24000 * 300
    {
        return Err(error(
            "estimated cues require nonempty mono 24 kHz PCM within five minutes",
        ));
    }
    let mut digest = Sha256::new();
    digest.update(audio.sample_rate.to_le_bytes());
    digest.update(audio.channels.to_le_bytes());
    digest.update((audio.frames() as u64).to_le_bytes());
    for sample in &audio.samples {
        digest.update(sample.to_bits().to_le_bytes());
    }
    Ok(format!("{:x}", digest.finalize()))
}
struct Stage(PathBuf);
impl Stage {
    fn new() -> Result<Self, AlignmentError> {
        for _ in 0..100 {
            let p = std::env::temp_dir().join(format!(
                "kilsat-rhubarb-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&p) {
                Ok(()) => return Ok(Self(p)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(error(e)),
            }
        }
        Err(error("cannot allocate Rhubarb staging directory"))
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn create(p: &Path, b: &[u8]) -> Result<(), AlignmentError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(p)
        .map_err(error)?
        .write_all(b)
        .map_err(error)
}
fn execute(
    command: &mut Command,
    out: &Path,
    max: u64,
    timeout: Duration,
    cancelled: &impl Fn() -> bool,
) -> Result<(), AlignmentError> {
    if cancelled() {
        return Err(error("Rhubarb cancelled"));
    }
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out)
        .map_err(error)?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(error)?;
    let pipe = child.stdout.take().expect("piped stdout");
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    // The pipe reader retains at most max+1 bytes, then closes the pipe. An
    // overproducing executable cannot grow a staging file between polling ticks.
    let reader = thread::Builder::new()
        .name("rhubarb-output".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            let result = pipe.take(max + 1).read_to_end(&mut bytes).map(|_| bytes);
            let _ = tx.send(result);
        });
    let reader = match reader {
        Ok(reader) => reader,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error(e));
        }
    };
    let started = Instant::now();
    let mut capture = None;
    let mut status = None;
    let result = loop {
        if cancelled() {
            break Err(error("Rhubarb cancelled"));
        }
        if started.elapsed() > timeout {
            break Err(error("Rhubarb timed out"));
        }
        if capture.is_none() {
            match rx.try_recv() {
                Ok(Ok(bytes)) => {
                    if bytes.len() as u64 > max {
                        break Err(error("Rhubarb output exceeded budget"));
                    }
                    capture = Some(bytes);
                }
                Ok(Err(e)) => break Err(error(e)),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    break Err(error("Rhubarb output reader failed"))
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(value) => status = value,
                Err(e) => break Err(error(e)),
            }
        }
        if let (Some(status), Some(bytes)) = (&status, &capture) {
            if !status.success() {
                break Err(error(format!("Rhubarb failed: {status}")));
            }
            break destination.write_all(bytes).map_err(error);
        }
        thread::sleep(Duration::from_millis(10));
    };
    if result.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    // On normal Rhubarb execution the direct child owns stdout and killing it
    // closes the pipe. Never block cancellation waiting for an unrelated program
    // that inherited a descriptor: dropping the handle detaches only that reader.
    if capture.is_some() || reader.is_finished() {
        let _ = reader.join();
    }
    result
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    metadata: Metadata,
    mouth_cues: Vec<RawCue>,
}
#[derive(Deserialize)]
struct Metadata {
    duration: f64,
}
#[derive(Deserialize)]
struct RawCue {
    start: f64,
    end: f64,
    value: String,
}
fn convert(bytes: &[u8], frames: u64) -> Result<Vec<MouthCue>, AlignmentError> {
    if bytes.len() as u64 > MAX_JSON {
        return Err(error("Rhubarb JSON exceeds budget"));
    }
    let raw: Raw = serde_json::from_slice(bytes).map_err(error)?;
    let truncated = (frames / 240) as f64 / 100.0;
    if !raw.metadata.duration.is_finite()
        || (raw.metadata.duration - truncated).abs() > 1e-7
        || raw.mouth_cues.len() > 100_000
    {
        return Err(error("Rhubarb duration differs from exact WAV"));
    }
    let mut result = Vec::new();
    let mut previous = 0.0;
    for c in raw.mouth_cues {
        if !c.start.is_finite()
            || !c.end.is_finite()
            || c.start < previous
            || c.start < 0.0
            || c.end <= c.start
            || c.end > truncated + 1e-7
        {
            return Err(error("invalid Rhubarb timing"));
        }
        let mut chars = c.value.chars();
        let shape = chars.next().ok_or_else(|| error("empty mouth shape"))?;
        if chars.next().is_some() || shape_viseme(shape).is_none() {
            return Err(error("unknown Rhubarb mouth shape"));
        }
        let start_sample = (c.start * 24000.0).round() as u64;
        let end_sample = (c.end * 24000.0).round() as u64;
        if start_sample >= end_sample || end_sample > frames {
            return Err(error("mouth interval collapses on sample clock"));
        }
        result.push(MouthCue {
            start_sample,
            end_sample,
            shape,
        });
        previous = c.end;
    }
    Ok(result)
}
// Check chunk lengths before hound can reserve from an untrusted data-size header.
fn preflight_wav(b: &[u8]) -> Result<(), AlignmentError> {
    let invalid =
        || error("requires bounded RIFF WAV: mono 24 kHz PCM16 or float32, maximum five minutes");
    if b.len() < 12
        || &b[..4] != b"RIFF"
        || &b[8..12] != b"WAVE"
        || u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize + 8 != b.len()
    {
        return Err(invalid());
    }
    let mut position = 12;
    let mut block = None;
    let mut data = None;
    while position < b.len() {
        if b.len() - position < 8 {
            return Err(invalid());
        }
        let size = u32::from_le_bytes(b[position + 4..position + 8].try_into().unwrap()) as usize;
        let start = position + 8;
        let end = start
            .checked_add(size)
            .filter(|&end| end <= b.len())
            .ok_or_else(invalid)?;
        match &b[position..position + 4] {
            b"fmt " => {
                if block.is_some() || size < 16 {
                    return Err(invalid());
                }
                let f = &b[start..end];
                let encoding = u16::from_le_bytes(f[..2].try_into().unwrap());
                let channels = u16::from_le_bytes(f[2..4].try_into().unwrap());
                let rate = u32::from_le_bytes(f[4..8].try_into().unwrap());
                let align = u16::from_le_bytes(f[12..14].try_into().unwrap());
                let bits = u16::from_le_bytes(f[14..16].try_into().unwrap());
                if channels != 1
                    || rate != 24000
                    || !matches!((encoding, bits, align), (1, 16, 2) | (3, 32, 4))
                {
                    return Err(invalid());
                }
                block = Some(align as usize);
            }
            b"data" if data.replace(size).is_some() => {
                return Err(invalid());
            }
            _ => {}
        }
        position = end.checked_add(size % 2).ok_or_else(invalid)?;
        if position > b.len() {
            return Err(invalid());
        }
    }
    let block = block.ok_or_else(invalid)?;
    let data = data.ok_or_else(invalid)?;
    if data == 0 || data % block != 0 || data / block > 24000 * 300 {
        return Err(invalid());
    }
    Ok(())
}
/// Blocking, explicit opt-in operation. Call from a worker, never the UI event loop.
/// No installation, asset download or implicit PATH lookup is performed.
pub fn align(
    executable: &Path,
    wav: &Path,
    transcript: &str,
    recognizer: Recognizer,
    cancelled: impl Fn() -> bool,
) -> Result<EstimatedVisemeAlignment, AlignmentError> {
    align_with_policy(executable, wav, transcript, recognizer, None, &cancelled)
}
pub(crate) fn align_controlled(
    executable: &Path,
    wav: &Path,
    transcript: &str,
    recognizer: Recognizer,
    working_directory: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<EstimatedVisemeAlignment, AlignmentError> {
    align_with_policy(
        executable,
        wav,
        transcript,
        recognizer,
        Some(working_directory),
        &cancelled,
    )
}
fn align_with_policy(
    executable: &Path,
    wav: &Path,
    transcript: &str,
    recognizer: Recognizer,
    working_directory: Option<&Path>,
    cancelled: &impl Fn() -> bool,
) -> Result<EstimatedVisemeAlignment, AlignmentError> {
    if !executable.is_absolute() {
        return Err(error(
            "Rhubarb executable must be an explicit absolute path",
        ));
    }
    if transcript.len() > 64 * 1024 || transcript.contains('\0') {
        return Err(error("transcript exceeds budget or contains NUL"));
    }
    if cancelled() {
        return Err(error("Rhubarb cancelled"));
    }
    let executable = fs::canonicalize(executable).map_err(error)?;
    let executable_sha256 = hash(&bounded(&executable, MAX_BINARY)?);
    let stage = Stage::new()?;
    let input = stage.0.join("input.wav");
    let wav_bytes = bounded(wav, MAX_WAV)?;
    preflight_wav(&wav_bytes)?;
    create(&input, &wav_bytes)?;
    let audio = motion_audio::read_wav(&input).map_err(error)?;
    if audio.sample_rate != 24000
        || audio.channels != 1
        || audio.frames() == 0
        || audio.frames() > 24000 * 300
    {
        return Err(error(
            "requires nonempty mono 24 kHz WAV, maximum five minutes",
        ));
    }
    let pcm_sha256 = pcm_identity(&audio)?;
    let text = stage.0.join("dialog.txt");
    create(&text, transcript.as_bytes())?;
    let version_file = stage.0.join("version.txt");
    let command = || {
        let mut command = Command::new(&executable);
        if let Some(directory) = working_directory {
            command
                .env_clear()
                .env("LC_ALL", "C")
                .env("LANG", "C")
                .env("PATH", "/usr/bin:/bin")
                .env("HOME", &stage.0)
                .current_dir(directory);
        }
        command
    };
    execute(
        command().arg("--version"),
        &version_file,
        4096,
        Duration::from_secs(5),
        &cancelled,
    )?;
    let tool_version = String::from_utf8(bounded(&version_file, 4096)?)
        .map_err(error)?
        .trim()
        .to_owned();
    if tool_version.is_empty() {
        return Err(error("empty Rhubarb version"));
    }
    let output = stage.0.join("result.json");
    execute(
        command()
            .args([
                "--exportFormat",
                "json",
                "--recognizer",
                recognizer.argument(),
                "--extendedShapes",
                "GHX",
                "--threads",
                "2",
                "--quiet",
                "--dialogFile",
            ])
            .arg(&text)
            .arg(&input),
        &output,
        MAX_JSON,
        Duration::from_secs(120),
        &cancelled,
    )?;
    if bounded(&input, MAX_WAV)? != wav_bytes {
        return Err(error("input WAV changed during execution"));
    }
    if hash(&bounded(&executable, MAX_BINARY)?) != executable_sha256 {
        return Err(error("Rhubarb executable changed during execution"));
    }
    let result = EstimatedVisemeAlignment {
        schema_version: 1,
        source: "estimated_rhubarb".into(),
        tool_version,
        executable_sha256,
        pcm_sha256,
        transcript_sha256: hash(transcript.as_bytes()),
        recognizer,
        sample_rate: audio.sample_rate,
        frame_count: audio.frames() as u64,
        cues: convert(&bounded(&output, MAX_JSON)?, audio.frames() as u64)?,
    };
    result.validate()?;
    if cancelled() {
        return Err(error("Rhubarb cancelled"));
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pcm_identity_matches_offline_quality_evidence_golden_vector() {
        // Shared with the Python evidence reader, including signed PCM16 extrema.
        let audio = motion_audio::AudioBuffer {
            sample_rate: 24000,
            channels: 1,
            samples: [-32768_i32, -1, 0, 1, 32767]
                .into_iter()
                .map(|sample| sample as f32 / 32768.0)
                .collect(),
        };
        assert_eq!(
            pcm_identity(&audio).unwrap(),
            "161dcb72c4cbb1b6ba7ce5a0520fdb05d1446c8f84a4246fc24f03504984a855"
        );
    }
    fn raw(duration: f64, cues: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"metadata":{"duration":duration},"mouthCues":cues}))
            .unwrap()
    }
    #[test]
    fn shape_mapping() {
        for (s, v) in "ABCDEFGHX".chars().zip([
            Viseme::Closed,
            Viseme::I,
            Viseme::E,
            Viseme::A,
            Viseme::O,
            Viseme::U,
            Viseme::TeethLip,
            Viseme::TongueUp,
            Viseme::Rest,
        ]) {
            assert_eq!(shape_viseme(s), Some(v));
        }
        assert_eq!(shape_viseme('Q'), None);
    }
    #[test]
    fn exact_duration_truncation_and_half_open() {
        let cues = convert(
            &raw(
                0.47,
                serde_json::json!([{"start":0.0,"end":0.47,"value":"D"}]),
            ),
            11399,
        )
        .unwrap();
        assert_eq!(cues[0].end_sample, 11280);
        let a = EstimatedVisemeAlignment {
            schema_version: 1,
            source: "estimated_rhubarb".into(),
            tool_version: "test stub".into(),
            executable_sha256: "a".repeat(64),
            pcm_sha256: "b".repeat(64),
            transcript_sha256: "c".repeat(64),
            recognizer: Recognizer::PocketSphinx,
            sample_rate: 24000,
            frame_count: 11399,
            cues,
        };
        assert_eq!(a.viseme_at_sample(11279).unwrap(), Viseme::A);
        assert_eq!(a.viseme_at_sample(11280).unwrap(), Viseme::Rest);
        let b: EstimatedVisemeAlignment =
            serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        b.validate().unwrap();
    }
    #[test]
    fn rejects_bad_output() {
        assert!(convert(&raw(1.0, serde_json::json!([])), 11399).is_err());
        for c in [
            serde_json::json!([{"start":0.0,"end":0.4,"value":"D"},{"start":0.3,"end":0.47,"value":"X"}]),
            serde_json::json!([{"start":0.0,"end":0.47,"value":"Q"}]),
            serde_json::json!([{"start":0.0,"end":0.000001,"value":"D"}]),
        ] {
            assert!(convert(&raw(0.47, c), 11399).is_err());
        }
    }
    #[test]
    fn refuses_implicit_execution_and_cancel() {
        assert!(align(
            Path::new("rhubarb"),
            Path::new("missing"),
            "",
            Recognizer::Phonetic,
            || false
        )
        .is_err());
        assert!(align(
            Path::new("/missing"),
            Path::new("missing"),
            "",
            Recognizer::Phonetic,
            || true
        )
        .unwrap_err()
        .0
        .contains("cancelled"));
    }
    #[test]
    fn riff_preflight_rejects_oversized_chunks_before_decode() {
        let stage = Stage::new().unwrap();
        let path = stage.0.join("audio.wav");
        let audio = motion_audio::AudioBuffer {
            sample_rate: 24000,
            channels: 1,
            samples: vec![0.0; 12000],
        };
        motion_audio::write_wav(&path, &audio).unwrap();
        let mut bytes = bounded(&path, MAX_WAV).unwrap();
        preflight_wav(&bytes).unwrap();
        let data = bytes.windows(4).position(|x| x == b"data").unwrap();
        bytes[data + 4..data + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(preflight_wav(&bytes).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn stub_execution_identity_and_serialization_not_real_engine_quality() {
        use std::os::unix::fs::PermissionsExt;
        let stage = Stage::new().unwrap();
        let executable = stage.0.join("rhubarb-stub");
        create(&executable,b"#!/bin/bash\nif [ \"$1\" = \"--version\" ]; then printf 'synthetic-test-stub-1'; else printf '%s' '{\"metadata\":{\"duration\":0.5},\"mouthCues\":[{\"start\":0.0,\"end\":0.5,\"value\":\"F\"}]}'; fi\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let wav = stage.0.join("input.wav");
        let mut audio = motion_audio::AudioBuffer {
            sample_rate: 24000,
            channels: 1,
            samples: vec![0.1; 12000],
        };
        motion_audio::write_wav(&wav, &audio).unwrap();
        let a = align(&executable, &wav, "hello", Recognizer::PocketSphinx, || {
            false
        })
        .unwrap();
        assert_eq!(a.viseme_at_sample(11999).unwrap(), Viseme::U);
        assert_eq!(a.viseme_at_sample(12000).unwrap(), Viseme::Rest);
        assert_eq!(a.tool_version, "synthetic-test-stub-1");
        assert_eq!(a.source, "estimated_rhubarb");
        let b = align(
            &executable,
            &wav,
            "changed",
            Recognizer::PocketSphinx,
            || false,
        )
        .unwrap();
        assert_ne!(a.transcript_sha256, b.transcript_sha256);
        assert_eq!(a.pcm_sha256, b.pcm_sha256);
        audio.samples[0] = 0.2;
        motion_audio::write_wav(&wav, &audio).unwrap();
        let c = align(&executable, &wav, "hello", Recognizer::PocketSphinx, || {
            false
        })
        .unwrap();
        assert_ne!(a.pcm_sha256, c.pcm_sha256);
    }
    #[cfg(unix)]
    #[test]
    fn stub_process_timeout_cancel_failure_and_output_budget() {
        let stage = Stage::new().unwrap();
        let mut spin = Command::new("/bin/bash");
        spin.args(["-c", "while :; do :; done"]);
        assert!(execute(
            &mut spin,
            &stage.0.join("timeout"),
            4096,
            Duration::from_millis(30),
            &|| false
        )
        .unwrap_err()
        .0
        .contains("timed out"));
        let polls = std::cell::Cell::new(0);
        let cancel = || {
            polls.set(polls.get() + 1);
            polls.get() > 2
        };
        let mut spin = Command::new("/bin/bash");
        spin.args(["-c", "while :; do :; done"]);
        assert!(execute(
            &mut spin,
            &stage.0.join("cancel"),
            4096,
            Duration::from_secs(2),
            &cancel
        )
        .unwrap_err()
        .0
        .contains("cancelled"));
        let mut fail = Command::new("/bin/bash");
        fail.args(["-c", "exit 7"]);
        assert!(execute(
            &mut fail,
            &stage.0.join("fail"),
            4096,
            Duration::from_secs(2),
            &|| false
        )
        .unwrap_err()
        .0
        .contains("failed"));
        let mut large = Command::new("/bin/bash");
        large.args(["-c", "printf '%5000s' x"]);
        assert!(execute(
            &mut large,
            &stage.0.join("large"),
            4096,
            Duration::from_secs(2),
            &|| false
        )
        .unwrap_err()
        .0
        .contains("budget"));
    }
}
