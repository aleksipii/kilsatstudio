//! Explicit local Finnish speech. Construct providers on the UI thread, execute on a worker.
//! System voices are private preview; publication needs independently cleared assets.
use super::{
    fingerprint, read_bounded, validate_wav, CancellationToken, KokoroConfig, KokoroProvider,
    SpeechError, SpeechProvider, SpeechRequest, SpeechResult,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MAX_AUDIO_BYTES: u64 = 32 * 1024 * 1024;
const MAX_OUTPUT_SAMPLES: u64 = 24_000 * 300;
const MAX_LOG_BYTES: u64 = 1024 * 1024;
const MAX_MODEL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_EXECUTABLE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemVoice {
    pub preset: String,
    pub language: String,
}

fn parse_system_voices(text: &str) -> Vec<SystemVoice> {
    let mut voices = Vec::new();
    for line in text.lines() {
        let fields: Vec<_> = line
            .split('#')
            .next()
            .unwrap_or("")
            .split_whitespace()
            .collect();
        let Some((locale, name)) = fields.split_last() else {
            continue;
        };
        if !matches!(*locale, "fi_FI" | "fi-FI") || name.is_empty() {
            continue;
        }
        let preset = name.join(" ");
        if !preset
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            continue;
        }
        let voice = SystemVoice {
            preset,
            language: "fi-fi".into(),
        };
        if !voices.contains(&voice) {
            voices.push(voice);
        }
    }
    voices.sort_by(|a, b| a.preset.cmp(&b.preset));
    voices
}

fn regular(path: &Path, limit: u64, label: &str) -> Result<(), SpeechError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > limit {
        return Err(SpeechError(format!(
            "{label} must be a bounded regular non-symlink file"
        )));
    }
    Ok(())
}

fn checked_fingerprint(path: &Path, cancel: &CancellationToken) -> Result<String, SpeechError> {
    let mut input = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut block = [0u8; 65536];
    loop {
        cancel.check()?;
        let size = input.read(&mut block)?;
        if size == 0 {
            break;
        }
        hash.update(&block[..size]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

struct Staging(PathBuf);
impl Staging {
    fn new(root: &Path) -> Result<Self, SpeechError> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        fs::create_dir_all(root)?;
        let root = fs::canonicalize(root)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let nonce = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = root.join(format!(
            ".local-speech-{}-{now}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(
    mut command: Command,
    dir: &Path,
    output: Option<&Path>,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<(), SpeechError> {
    if timeout.is_zero() || timeout > Duration::from_secs(600) {
        return Err(SpeechError(
            "local speech timeout must be within 1..600 seconds".into(),
        ));
    }
    cancel.check()?;
    let stdout = dir.join("stdout.log");
    let stderr = dir.join("stderr.log");
    command
        .stdout(Stdio::from(fs::File::create(&stdout)?))
        .stderr(Stdio::from(fs::File::create(&stderr)?));
    let mut child = command
        .spawn()
        .map_err(|e| SpeechError(format!("local speech process: {e}")))?;
    let started = Instant::now();
    let status = loop {
        let too_large = [&stdout, &stderr]
            .iter()
            .any(|p| fs::metadata(p).is_ok_and(|m| m.len() > MAX_LOG_BYTES))
            || output.is_some_and(|p| fs::metadata(p).is_ok_and(|m| m.len() > MAX_AUDIO_BYTES));
        let interrupted = if cancel.is_cancelled() {
            Some("Speech generation cancelled")
        } else if started.elapsed() > timeout {
            Some("Local speech synthesis timed out")
        } else if too_large {
            Some("Local speech output exceeds work budget")
        } else {
            None
        };
        if let Some(message) = interrupted {
            let _ = child.kill();
            let _ = child.wait();
            return Err(SpeechError(message.into()));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
    };
    cancel.check()?;
    regular(&stdout, MAX_LOG_BYTES, "speech log")?;
    regular(&stderr, MAX_LOG_BYTES, "speech log")?;
    if !status.success() {
        let mut detail = String::new();
        fs::File::open(&stderr)?
            .take(4096)
            .read_to_string(&mut detail)?;
        return Err(SpeechError(format!(
            "Local speech exited {status}: {}",
            detail.trim()
        )));
    }
    if let Some(output) = output {
        regular(output, MAX_AUDIO_BYTES, "speech output")?;
    }
    Ok(())
}

fn publish(
    audio: &Path,
    root: &Path,
    request_key: &str,
    cancel: &CancellationToken,
) -> Result<SpeechResult, SpeechError> {
    regular(audio, MAX_AUDIO_BYTES, "speech WAV")?;
    let (sample_rate, sample_count) = validate_wav(audio)?;
    if sample_count > MAX_OUTPUT_SAMPLES {
        return Err(SpeechError("speech exceeds five minutes".into()));
    }
    cancel.check()?;
    let audio_hash = checked_fingerprint(audio, cancel)?;
    let key = format!(
        "{:x}",
        Sha256::digest(format!("{request_key}:{audio_hash}"))
    );
    let destination = fs::canonicalize(root)?.join(format!("{key}.wav"));
    for attempt in 0..3 {
        cancel.check()?;
        match fs::hard_link(audio, &destination) {
            Ok(()) => break,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                regular(&destination, MAX_AUDIO_BYTES, "cached WAV")?;
                if checked_fingerprint(&destination, cancel)? == audio_hash {
                    break;
                }
                // Only a corrupt entry in our own content cache is discarded.
                fs::remove_file(&destination)?;
                if attempt == 2 {
                    return Err(SpeechError(
                        "concurrent corrupt speech cache publication".into(),
                    ));
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
    if checked_fingerprint(&destination, cancel)? != audio_hash {
        return Err(SpeechError("speech cache identity mismatch".into()));
    }
    cancel.check()?;
    Ok(SpeechResult {
        audio_path: destination,
        sample_rate,
        sample_count,
        cache_key: key,
        phonemes: String::new(),
        cached: false,
    })
}

/// No downloads. macOS installed-voice discovery belongs on a background worker.
pub fn installed_system_voices(
    cancel: &CancellationToken,
) -> Result<Vec<SystemVoice>, SpeechError> {
    if !cfg!(target_os = "macos") {
        return Err(SpeechError(
            "Finnish system preview requires macOS; configure local Piper or import WAV".into(),
        ));
    }
    let temp = Staging::new(&std::env::temp_dir())?;
    let mut command = Command::new("/usr/bin/say");
    command
        .args(["-v", "?"])
        .env("LC_ALL", "C")
        .stdin(Stdio::null());
    run(command, &temp.0, None, Duration::from_secs(10), cancel)?;
    let bytes = read_bounded(&temp.0.join("stdout.log"))?;
    let text = std::str::from_utf8(&bytes).map_err(|e| SpeechError(e.to_string()))?;
    Ok(parse_system_voices(text))
}

/// Private preview only. Apple's system-voice terms are not publication clearance.
pub struct SystemSpeechProvider {
    cache_directory: PathBuf,
    timeout: Duration,
}
impl SystemSpeechProvider {
    pub fn new(cache_directory: PathBuf, timeout: Duration) -> Self {
        Self {
            cache_directory,
            timeout,
        }
    }
}
impl SpeechProvider for SystemSpeechProvider {
    fn synthesize(&self, request: SpeechRequest) -> Result<SpeechResult, SpeechError> {
        self.synthesize_cancellable(request, &CancellationToken::new())
    }
    fn synthesize_cancellable(
        &self,
        request: SpeechRequest,
        cancel: &CancellationToken,
    ) -> Result<SpeechResult, SpeechError> {
        request.validate()?;
        cancel.check()?;
        if request.voice.language != "fi-fi" || request.voice.preset.starts_with("piper_") {
            return Err(SpeechError(
                "system private preview requires fi-fi and a system voice name".into(),
            ));
        }
        if request.text.contains("[[") {
            return Err(SpeechError(
                "embedded macOS speech commands are not dialogue; remove [[ markup".into(),
            ));
        }
        if !installed_system_voices(cancel)?
            .iter()
            .any(|voice| voice.preset == request.voice.preset)
        {
            return Err(SpeechError(format!("Finnish voice {} is not installed; choose an installed private-preview voice, configure Piper, or import WAV", request.voice.preset)));
        }
        let temp = Staging::new(&self.cache_directory)?;
        let text = temp.0.join("text.txt");
        let output = temp.0.join("speech.wav");
        fs::write(&text, &request.text)?;
        let mut command = Command::new("/usr/bin/say");
        command
            .arg("-v")
            .arg(&request.voice.preset)
            .arg("-r")
            .arg((175.0_f32 * request.voice.speed).round().to_string())
            .arg("-f")
            .arg(text)
            .arg("-o")
            .arg(&output)
            .args([
                "--file-format=WAVE",
                "--data-format=LEI16@24000",
                "--channels=1",
            ])
            .env("LC_ALL", "C")
            .stdin(Stdio::null());
        run(command, &temp.0, Some(&output), self.timeout, cancel)?;
        let key = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                "system-private-preview-v1",
                &request,
                fingerprint(Path::new("/usr/bin/say"))?,
                std::env::consts::OS,
                std::env::consts::ARCH
            ))?)
        );
        // System voice asset versions are opaque. Never reuse an old request hit.
        publish(&output, &self.cache_directory, &key, cancel)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PiperConfig {
    pub executable: PathBuf,
    pub model_path: PathBuf,
    pub config_path: PathBuf,
    pub cache_directory: PathBuf,
    pub preset: String,
    pub model_version: String,
    pub environment_identity: String,
    pub timeout_seconds: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PiperProvenance {
    pub executable_sha256: String,
    pub model_sha256: String,
    pub config_sha256: String,
    pub native_sample_rate: u32,
    pub model_version_claim: String,
    pub environment_identity_claim: String,
    pub request_cache_reuse: bool,
    pub dependency_closure_verified: bool,
}
#[derive(Clone)]
pub struct PiperProvider {
    pub config: PiperConfig,
}
impl PiperProvider {
    pub fn new(config: PiperConfig) -> Self {
        Self { config }
    }
    pub fn from_config_file(path: &Path) -> Result<Self, SpeechError> {
        regular(path, MAX_LOG_BYTES, "Piper profile")?;
        Ok(Self::new(serde_json::from_slice(&read_bounded(path)?)?))
    }
    pub fn provenance(&self) -> Result<PiperProvenance, SpeechError> {
        self.provenance_cancellable(&CancellationToken::new())
    }
    fn provenance_cancellable(
        &self,
        cancel: &CancellationToken,
    ) -> Result<PiperProvenance, SpeechError> {
        cancel.check()?;
        let c = &self.config;
        for path in [
            &c.executable,
            &c.model_path,
            &c.config_path,
            &c.cache_directory,
        ] {
            if !path.is_absolute() {
                return Err(SpeechError("Piper paths must be absolute".into()));
            }
        }
        if c.preset.len() > 128
            || !c.preset.starts_with("piper_")
            || !c
                .preset
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || c.model_version.trim().is_empty()
            || c.model_version.len() > 4096
            || c.environment_identity.trim().is_empty()
            || c.environment_identity.len() > 4096
            || !(1..=600).contains(&c.timeout_seconds)
        {
            return Err(SpeechError(
                "Piper needs a piper_ preset, version/environment labels and 1..600 second timeout"
                    .into(),
            ));
        }
        regular(&c.executable, MAX_EXECUTABLE_BYTES, "Piper executable")?;
        regular(&c.model_path, MAX_MODEL_BYTES, "Piper model")?;
        regular(&c.config_path, MAX_LOG_BYTES, "Piper model configuration")?;
        let config: serde_json::Value = serde_json::from_slice(&read_bounded(&c.config_path)?)?;
        let native_sample_rate = config
            .pointer("/audio/sample_rate")
            .and_then(|n| n.as_u64())
            .filter(|rate| (8000..=96_000).contains(rate))
            .ok_or_else(|| SpeechError("Piper model needs 8..96kHz audio.sample_rate".into()))?
            as u32;
        let language = config
            .pointer("/espeak/voice")
            .and_then(|s| s.as_str())
            .unwrap_or("");
        if !matches!(language, "fi" | "fi-fi" | "fi_FI") {
            return Err(SpeechError(
                "Piper profile must reference an explicitly Finnish model (espeak.voice=fi)".into(),
            ));
        }
        Ok(PiperProvenance {
            executable_sha256: checked_fingerprint(&c.executable, cancel)?,
            model_sha256: checked_fingerprint(&c.model_path, cancel)?,
            config_sha256: checked_fingerprint(&c.config_path, cancel)?,
            native_sample_rate,
            model_version_claim: c.model_version.clone(),
            environment_identity_claim: c.environment_identity.clone(),
            request_cache_reuse: false,
            dependency_closure_verified: false,
        })
    }
}

fn resample_wav(
    input: &Path,
    output: &Path,
    native_rate: u32,
    cancel: &CancellationToken,
) -> Result<(), SpeechError> {
    regular(input, MAX_AUDIO_BYTES, "Piper output")?;
    let mut reader = hound::WavReader::open(input)
        .map_err(|e| SpeechError(format!("invalid native speech WAV: {e}")))?;
    let spec = reader.spec();
    let count = reader.duration() as u64;
    if spec.channels != 1
        || spec.sample_rate != native_rate
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
        || count == 0
        || count > u64::from(native_rate) * 300
    {
        return Err(SpeechError(
            "Piper must produce bounded nonempty native-rate mono PCM16 WAV".into(),
        ));
    }
    let mut samples = Vec::with_capacity(count as usize);
    for (i, sample) in reader.samples::<i16>().enumerate() {
        if i % 16_384 == 0 {
            cancel.check()?;
        }
        samples.push(sample.map_err(|e| SpeechError(e.to_string()))?);
    }
    if samples.len() as u64 != count {
        return Err(SpeechError("native speech WAV is truncated".into()));
    }
    let total =
        (u128::from(count) * 24_000 + u128::from(native_rate) / 2) / u128::from(native_rate);
    if total == 0 || total > u128::from(MAX_OUTPUT_SAMPLES) {
        return Err(SpeechError(
            "resampled speech exceeds five-minute work budget".into(),
        ));
    }
    let mut writer = hound::WavWriter::create(
        output,
        hound::WavSpec {
            channels: 1,
            sample_rate: 24_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .map_err(|e| SpeechError(e.to_string()))?;
    for i in 0..total as u64 {
        if i % 16_384 == 0 {
            cancel.check()?;
        }
        let numerator = i * u64::from(native_rate);
        let at = (numerator / 24_000) as usize;
        let fraction = (numerator % 24_000) as f64 / 24_000.0;
        let left = f64::from(samples[at.min(samples.len() - 1)]);
        let right = f64::from(samples[(at + 1).min(samples.len() - 1)]);
        let sample = (left + (right - left) * fraction)
            .round()
            .clamp(i16::MIN as f64, i16::MAX as f64) as i16;
        writer
            .write_sample(sample)
            .map_err(|e| SpeechError(e.to_string()))?;
    }
    writer.finalize().map_err(|e| SpeechError(e.to_string()))?;
    cancel.check()
}

impl SpeechProvider for PiperProvider {
    fn synthesize(&self, request: SpeechRequest) -> Result<SpeechResult, SpeechError> {
        self.synthesize_cancellable(request, &CancellationToken::new())
    }
    fn synthesize_cancellable(
        &self,
        request: SpeechRequest,
        cancel: &CancellationToken,
    ) -> Result<SpeechResult, SpeechError> {
        request.validate()?;
        cancel.check()?;
        if request.voice.language != "fi-fi" || request.voice.preset != self.config.preset {
            return Err(SpeechError(
                "Piper request must match configured Finnish preset".into(),
            ));
        }
        let before = self.provenance_cancellable(cancel)?;
        let temp = Staging::new(&self.config.cache_directory)?;
        let text = temp.0.join("input.txt");
        let native = temp.0.join("native.wav");
        let output = temp.0.join("speech.wav");
        fs::write(
            &text,
            format!("{}\n", request.text.replace(['\r', '\n'], " ")),
        )?;
        let mut command = Command::new(&self.config.executable);
        command
            .arg("--model")
            .arg(&self.config.model_path)
            .arg("--config")
            .arg(&self.config.config_path)
            .arg("--output_file")
            .arg(&native)
            .arg("--length_scale")
            .arg((1.0 / request.voice.speed).to_string())
            .stdin(Stdio::from(fs::File::open(text)?))
            .env_remove("PYTHONPATH")
            .env_remove("PYTHONHOME")
            .env_remove("PYTHONUSERBASE")
            .env("PYTHONNOUSERSITE", "1");
        run(
            command,
            &temp.0,
            Some(&native),
            Duration::from_secs(self.config.timeout_seconds),
            cancel,
        )?;
        if before != self.provenance_cancellable(cancel)? {
            return Err(SpeechError(
                "Piper assets changed during synthesis; output discarded".into(),
            ));
        }
        resample_wav(&native, &output, before.native_sample_rate, cancel)?;
        let request_key = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                "piper-local-v1",
                &request,
                &before,
                std::env::consts::OS,
                std::env::consts::ARCH
            ))?)
        );
        // Dependency labels and launcher hashes do not verify Python/ONNX closure.
        // Always perform inference; only identical actual PCM output is deduplicated.
        publish(&output, &self.config.cache_directory, &request_key, cancel)
    }
}

pub struct LocalSpeechProvider {
    kokoro: KokoroProvider,
    system: SystemSpeechProvider,
    piper_config_path: Option<PathBuf>,
}
impl LocalSpeechProvider {
    pub fn new(config: KokoroConfig) -> Self {
        let system = SystemSpeechProvider::new(
            config.cache_directory.join("system-preview"),
            config.timeout,
        );
        Self {
            kokoro: KokoroProvider { config },
            system,
            piper_config_path: None,
        }
    }
    pub fn with_piper_config_path(mut self, path: PathBuf) -> Self {
        self.piper_config_path = Some(path);
        self
    }
}
impl SpeechProvider for LocalSpeechProvider {
    fn synthesize(&self, request: SpeechRequest) -> Result<SpeechResult, SpeechError> {
        self.synthesize_cancellable(request, &CancellationToken::new())
    }
    fn synthesize_cancellable(
        &self,
        request: SpeechRequest,
        cancel: &CancellationToken,
    ) -> Result<SpeechResult, SpeechError> {
        request.validate()?;
        cancel.check()?;
        match request.voice.language.as_str() {
            "en-us" | "en-gb" => self.kokoro.synthesize_cancellable(request, cancel),
            "fi-fi" if request.voice.preset.starts_with("piper_") => {
                let path = self.piper_config_path.as_deref().ok_or_else(|| SpeechError("Configure a local Piper JSON profile before selecting a piper_ Finnish voice".into()))?;
                PiperProvider::from_config_file(path)?.synthesize_cancellable(request, cancel)
            }
            "fi-fi" => self.system.synthesize_cancellable(request, cancel),
            _ => Err(SpeechError("unsupported local speech language".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VoiceSettings;
    fn request() -> SpeechRequest {
        SpeechRequest {
            text: "Hei, maailma!".into(),
            voice: VoiceSettings {
                preset: "piper_harri".into(),
                language: "fi-fi".into(),
                speed: 1.0,
                style: None,
            },
        }
    }
    #[test]
    fn provider_boundaries_never_send_finnish_to_kokoro() {
        let config = KokoroConfig {
            python: "/never/python".into(),
            runtime_script: "/never/runtime".into(),
            model_config: "/never/config".into(),
            model_weights: "/never/model".into(),
            voices_directory: "/never/voices".into(),
            cache_directory: std::env::temp_dir().join("unused-fi-boundary"),
            model_version: "fixture".into(),
            timeout: Duration::from_secs(1),
        };
        assert!(request().validate().is_ok());
        assert!(KokoroProvider {
            config: config.clone()
        }
        .synthesize(request())
        .unwrap_err()
        .to_string()
        .contains("Kokoro supports only"));
        assert!(LocalSpeechProvider::new(config)
            .synthesize(request())
            .unwrap_err()
            .to_string()
            .contains("Configure a local Piper"));
    }
    #[test]
    fn inventory_requires_explicit_finnish_locale_and_safe_exact_name() {
        let voices = parse_system_voices("Satu fi_FI # Hei\nAlex en_US # Hi\nOnni fi-FI # Moi\nSatu fi_FI # duplicate\nUnsafe Voice fi_FI # excluded\n");
        assert_eq!(
            voices.iter().map(|v| v.preset.as_str()).collect::<Vec<_>>(),
            vec!["Onni", "Satu"]
        );
    }
    #[cfg(unix)]
    mod unix {
        use super::*;
        use std::os::unix::fs::PermissionsExt;
        struct Fixture {
            root: PathBuf,
            provider: PiperProvider,
        }
        impl Fixture {
            fn new() -> Self {
                let staging = Staging::new(&std::env::temp_dir()).unwrap();
                let root = staging.0.clone();
                std::mem::forget(staging);
                let executable = root.join("piper");
                let script = r#"#!/usr/bin/env python3
import argparse,pathlib,sys,wave,struct,time
p=argparse.ArgumentParser();p.add_argument('--model');p.add_argument('--config');p.add_argument('--output_file');p.add_argument('--length_scale');a=p.parse_args()
root=pathlib.Path(a.model).parent;calls=root/'calls';calls.write_text(str(int(calls.read_text())+1) if calls.exists() else '1')
text=sys.stdin.read()
if 'timeout' in text: time.sleep(5)
if 'invalid' in text: pathlib.Path(a.output_file).write_bytes(b'invalid');sys.exit(0)
with wave.open(a.output_file,'wb') as out:
 out.setnchannels(1);out.setsampwidth(2);out.setframerate(22050);out.writeframes(struct.pack('<441h',*([1000]*441)))
"#;
                fs::write(&executable, script).unwrap();
                fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
                let model = root.join("model.onnx");
                fs::write(&model, b"synthetic-model-identity").unwrap();
                let config = root.join("model.json");
                fs::write(
                    &config,
                    br#"{"audio":{"sample_rate":22050},"espeak":{"voice":"fi"}}"#,
                )
                .unwrap();
                let provider = PiperProvider::new(PiperConfig {
                    executable,
                    model_path: model,
                    config_path: config,
                    cache_directory: root.join("cache"),
                    preset: "piper_harri".into(),
                    model_version: "synthetic fixture".into(),
                    environment_identity: "synthetic python process, not real speech".into(),
                    timeout_seconds: 1,
                });
                Self { root, provider }
            }
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.root);
            }
        }
        #[test]
        fn actual_output_is_resampled_and_requests_never_reuse_unverified_runtime() {
            let fixture = Fixture::new();
            let a = fixture.provider.synthesize(request()).unwrap();
            assert_eq!((a.sample_rate, a.sample_count), (24000, 480));
            assert!(!a.cached);
            let b = fixture.provider.synthesize(request()).unwrap();
            assert_eq!(a.audio_path, b.audio_path);
            assert!(!b.cached);
            assert_eq!(fs::read_to_string(fixture.root.join("calls")).unwrap(), "2");
            fs::write(&b.audio_path, b"tampered").unwrap();
            let c = fixture.provider.synthesize(request()).unwrap();
            assert_eq!(validate_wav(&c.audio_path).unwrap(), (24000, 480));
            fs::write(&fixture.provider.config.model_path, b"changed model").unwrap();
            let d = fixture.provider.synthesize(request()).unwrap();
            assert_ne!(a.cache_key, d.cache_key);
            assert!(
                !fixture
                    .provider
                    .provenance()
                    .unwrap()
                    .dependency_closure_verified
            );
        }
        #[test]
        fn bad_output_timeout_and_precancel_are_recoverable() {
            let fixture = Fixture::new();
            let mut input = request();
            input.text = "invalid".into();
            assert!(fixture
                .provider
                .synthesize(input)
                .unwrap_err()
                .to_string()
                .contains("invalid native"));
            let mut input = request();
            input.text = "timeout".into();
            assert!(fixture
                .provider
                .synthesize(input)
                .unwrap_err()
                .to_string()
                .contains("timed out"));
            let cancel = CancellationToken::new();
            cancel.cancel();
            assert!(fixture
                .provider
                .synthesize_cancellable(request(), &cancel)
                .is_err());
            assert_eq!(fs::read_to_string(fixture.root.join("calls")).unwrap(), "2");
        }
        #[test]
        fn symlink_language_and_asset_changes_are_not_accepted() {
            let mut fixture = Fixture::new();
            let link = fixture.root.join("link");
            std::os::unix::fs::symlink(&fixture.provider.config.model_path, &link).unwrap();
            fixture.provider.config.model_path = link;
            assert!(fixture.provider.synthesize(request()).is_err());
            let fixture = Fixture::new();
            fs::write(
                &fixture.provider.config.config_path,
                br#"{"audio":{"sample_rate":22050},"espeak":{"voice":"en"}}"#,
            )
            .unwrap();
            assert!(fixture
                .provider
                .synthesize(request())
                .unwrap_err()
                .to_string()
                .contains("Finnish model"));
        }
        #[test]
        fn declared_large_or_truncated_native_audio_is_rejected_before_resampling() {
            let fixture = Fixture::new();
            let input = fixture.root.join("truncated.wav");
            let output = fixture.root.join("out.wav");
            let mut data = vec![0u8; 44];
            data[..4].copy_from_slice(b"RIFF");
            data[8..12].copy_from_slice(b"WAVE");
            data[12..16].copy_from_slice(b"fmt ");
            data[16..20].copy_from_slice(&16u32.to_le_bytes());
            data[20..22].copy_from_slice(&1u16.to_le_bytes());
            data[22..24].copy_from_slice(&1u16.to_le_bytes());
            data[24..28].copy_from_slice(&22050u32.to_le_bytes());
            data[28..32].copy_from_slice(&44100u32.to_le_bytes());
            data[32..34].copy_from_slice(&2u16.to_le_bytes());
            data[34..36].copy_from_slice(&16u16.to_le_bytes());
            data[36..40].copy_from_slice(b"data");
            data[40..44].copy_from_slice(&4000u32.to_le_bytes());
            data[4..8].copy_from_slice(&4036u32.to_le_bytes());
            fs::write(&input, data).unwrap();
            assert!(resample_wav(&input, &output, 22050, &CancellationToken::new()).is_err());
        }
    }
}
