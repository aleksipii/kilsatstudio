//! Blocking provider boundary. Call on a worker, never on the desktop event loop.
mod local;
pub use local::{
    installed_system_voices, LocalSpeechProvider, PiperConfig, PiperProvenance, PiperProvider,
    SystemSpeechProvider, SystemVoice,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fmt, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VoiceSettings {
    pub preset: String,
    pub language: String,
    pub speed: f32,
    /// Retained in cache identity; Kokoro does not offer an emotion control.
    #[serde(default)]
    pub style: Option<String>,
}
impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            preset: "af_heart".into(),
            language: "en-us".into(),
            speed: 1.0,
            style: None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpeechRequest {
    pub text: String,
    pub voice: VoiceSettings,
}
impl SpeechRequest {
    pub fn validate(&self) -> Result<(), SpeechError> {
        if self.text.trim().is_empty() || self.text.len() > 16000 || self.text.contains('\0') {
            return Err(SpeechError("text must contain 1..16000 bytes".into()));
        }
        if !self.voice.speed.is_finite() || !(0.5..=2.0).contains(&self.voice.speed) {
            return Err(SpeechError(
                "speed must be finite and between 0.5 and 2".into(),
            ));
        }
        if !["en-us", "en-gb", "fi-fi"].contains(&self.voice.language.as_str()) {
            return Err(SpeechError(
                "supported local languages are en-us, en-gb and fi-fi".into(),
            ));
        }
        if self.voice.preset.is_empty()
            || !self
                .voice
                .preset
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(SpeechError("invalid voice preset".into()));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpeechResult {
    pub audio_path: PathBuf,
    pub sample_rate: u32,
    pub sample_count: u64,
    pub cache_key: String,
    pub phonemes: String,
    pub cached: bool,
}
impl SpeechResult {
    pub fn duration_seconds(&self) -> f64 {
        self.sample_count as f64 / self.sample_rate as f64
    }
}
#[derive(Debug)]
pub struct SpeechError(pub String);
impl fmt::Display for SpeechError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for SpeechError {}
impl From<std::io::Error> for SpeechError {
    fn from(e: std::io::Error) -> Self {
        Self(e.to_string())
    }
}
impl From<serde_json::Error> for SpeechError {
    fn from(e: serde_json::Error) -> Self {
        Self(e.to_string())
    }
}
/// Cooperative cancellation shared by the UI job and its speech worker.
#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn check(&self) -> Result<(), SpeechError> {
        if self.is_cancelled() {
            Err(SpeechError("Speech generation cancelled".into()))
        } else {
            Ok(())
        }
    }
}
pub trait SpeechProvider: Send + Sync {
    fn synthesize(&self, request: SpeechRequest) -> Result<SpeechResult, SpeechError>;
    /// Providers should override this to interrupt active inference. The default
    /// checks at the boundary, but cannot interrupt a custom blocking provider.
    fn synthesize_cancellable(
        &self,
        request: SpeechRequest,
        cancel: &CancellationToken,
    ) -> Result<SpeechResult, SpeechError> {
        cancel.check()?;
        let result = self.synthesize(request)?;
        cancel.check()?;
        Ok(result)
    }
}

#[derive(Clone, Debug)]
pub struct KokoroConfig {
    pub python: PathBuf,
    pub runtime_script: PathBuf,
    pub model_config: PathBuf,
    pub model_weights: PathBuf,
    pub voices_directory: PathBuf,
    pub cache_directory: PathBuf,
    /// Immutable revision plus runtime/dependency version identity supplied by installer.
    pub model_version: String,
    pub timeout: Duration,
}
pub struct KokoroProvider {
    pub config: KokoroConfig,
}
fn fingerprint(path: &Path) -> Result<String, SpeechError> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
const MAX_METADATA_BYTES: u64 = 1_000_000;

// Read through a bounded reader: a size check alone races concurrent writers.
fn read_bounded(path: &Path) -> Result<Vec<u8>, SpeechError> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        return Err(SpeechError("speech metadata exceeds size limit".into()));
    }
    Ok(bytes)
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    result: SpeechResult,
    audio_sha256: String,
}

fn validate_wav(path: &Path) -> Result<(u32, u64), SpeechError> {
    let mut wav = hound::WavReader::open(path)
        .map_err(|e| SpeechError(format!("invalid speech WAV: {e}")))?;
    let spec = wav.spec();
    if spec.channels != 1
        || spec.sample_rate != 24000
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
        || wav.duration() == 0
    {
        return Err(SpeechError(
            "expected nonempty 24kHz mono PCM16 speech WAV".into(),
        ));
    }
    for sample in wav.samples::<i16>() {
        sample.map_err(|e| SpeechError(e.to_string()))?;
    }
    Ok((spec.sample_rate, wav.duration() as u64))
}
impl SpeechProvider for KokoroProvider {
    fn synthesize(&self, request: SpeechRequest) -> Result<SpeechResult, SpeechError> {
        self.synthesize_cancellable(request, &CancellationToken::new())
    }
    fn synthesize_cancellable(
        &self,
        request: SpeechRequest,
        cancel: &CancellationToken,
    ) -> Result<SpeechResult, SpeechError> {
        cancel.check()?;
        request.validate()?;
        if !["en-us", "en-gb"].contains(&request.voice.language.as_str()) {
            return Err(SpeechError(
                "Kokoro supports only en-us and en-gb; use a Finnish provider or imported WAV"
                    .into(),
            ));
        }
        let c = &self.config;
        if c.timeout.is_zero() || c.model_version.trim().is_empty() {
            return Err(SpeechError(
                "model version and positive timeout are required".into(),
            ));
        }
        let model_config = fs::canonicalize(&c.model_config)?;
        let model_weights = fs::canonicalize(&c.model_weights)?;
        let runtime_script = fs::canonicalize(&c.runtime_script)?;
        let voice = fs::canonicalize(
            c.voices_directory
                .join(format!("{}.pt", request.voice.preset)),
        )?;
        // Hash file contents, so replacing an asset cannot return stale speech.
        let identity = serde_json::to_vec(&(
            2,
            &request,
            &c.model_version,
            fingerprint(&model_config)?,
            fingerprint(&model_weights)?,
            fingerprint(&voice)?,
            fingerprint(&runtime_script)?,
        ))?;
        let key = format!("{:x}", Sha256::digest(identity));
        fs::create_dir_all(&c.cache_directory)?;
        let cache_directory = fs::canonicalize(&c.cache_directory)?;
        let audio_path = cache_directory.join(format!("{key}.wav"));
        let metadata_path = cache_directory.join(format!("{key}.json"));
        cancel.check()?;
        if let Ok(bytes) = read_bounded(&metadata_path) {
            if let Ok(entry) = serde_json::from_slice::<CacheEntry>(&bytes) {
                let mut result = entry.result;
                if let Ok((rate, count)) = validate_wav(&audio_path) {
                    if result.cache_key == key
                        && result.sample_rate == rate
                        && result.sample_count == count
                        && fingerprint(&audio_path).is_ok_and(|digest| digest == entry.audio_sha256)
                    {
                        cancel.check()?;
                        result.audio_path = audio_path;
                        result.cached = true;
                        return Ok(result);
                    }
                }
            }
        }
        // Unique staging directory supports overlapping generations of the same key.
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let staging = cache_directory.join(format!(".{key}-{}-{nonce}", std::process::id()));
        fs::create_dir(&staging)?;
        let outcome = (|| {
            let output = staging.join("speech.wav");
            let request_path = staging.join("request.json");
            let response_path = staging.join("response.json");
            fs::write(
                &request_path,
                serde_json::to_vec(
                    &serde_json::json!({"protocol":1,"request":request,"config_path":model_config,"model_path":model_weights,"voice_path":voice,"output_path":output}),
                )?,
            )?;
            cancel.check()?;
            let mut child = Command::new(&c.python)
                .arg(&runtime_script)
                .arg("--request")
                .arg(&request_path)
                .arg("--response")
                .arg(&response_path)
                .env("HF_HUB_OFFLINE", "1")
                .env("TRANSFORMERS_OFFLINE", "1")
                .env("HF_HUB_DISABLE_TELEMETRY", "1")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| SpeechError(format!("cannot launch Kokoro runtime: {e}")))?;
            let start = Instant::now();
            let status = loop {
                if let Err(error) = cancel.check() {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error);
                }
                match child.try_wait() {
                    Ok(Some(status)) => break status,
                    Ok(None) if start.elapsed() < c.timeout => {
                        thread::sleep(Duration::from_millis(20))
                    }
                    Ok(None) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(SpeechError("Kokoro synthesis timed out".into()));
                    }
                    Err(e) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(e.into());
                    }
                }
            };
            cancel.check()?;
            let bytes = read_bounded(&response_path).map_err(|e| {
                SpeechError(format!("runtime exited {status} without a response: {e}"))
            })?;
            let response: serde_json::Value = serde_json::from_slice(&bytes)?;
            if response["protocol"] != 1 {
                return Err(SpeechError("unsupported Kokoro response protocol".into()));
            }
            if !status.success() || response["ok"] != true {
                return Err(SpeechError(format!(
                    "Kokoro unavailable: {}",
                    response["error"]
                        .as_str()
                        .unwrap_or("unknown runtime error")
                )));
            }
            let (sample_rate, sample_count) = validate_wav(&output)?;
            let result = SpeechResult {
                audio_path: audio_path.clone(),
                sample_rate,
                sample_count,
                cache_key: key.clone(),
                phonemes: response["phonemes"].as_str().unwrap_or("").into(),
                cached: false,
            };
            let audio_sha256 = fingerprint(&output)?;
            let mut meta = fs::File::create(staging.join("metadata.json"))?;
            meta.write_all(&serde_json::to_vec(&CacheEntry {
                result: result.clone(),
                audio_sha256,
            })?)?;
            meta.sync_all()?;
            // Publication is a short commit boundary: once begun, finish it so
            // cancellation cannot invalidate a previously successful cache entry.
            cancel.check()?;
            fs::rename(&output, &audio_path)?;
            fs::rename(staging.join("metadata.json"), &metadata_path)?;
            Ok(result)
        })();
        let _ = fs::remove_dir_all(staging);
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_requests() {
        let mut r = SpeechRequest {
            text: "hello".into(),
            voice: VoiceSettings::default(),
        };
        assert!(r.validate().is_ok());
        r.voice.speed = f32::NAN;
        assert!(r.validate().is_err());
        r.voice.speed = 1.0;
        r.voice.preset = "../escape".into();
        assert!(r.validate().is_err());
    }
    fn mock_provider() -> KokoroProvider {
        static NEXT_FIXTURE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "motion-tts-test-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("voices")).unwrap();
        for name in ["config.json", "weights.pth", "voices/af_heart.pt"] {
            fs::write(root.join(name), b"fixture").unwrap();
        }
        let script = root.join("runtime.py");
        fs::write(&script, r#"import argparse,json,wave
p=argparse.ArgumentParser();p.add_argument('--request');p.add_argument('--response');a=p.parse_args()
r=json.load(open(a.request))
with wave.open(r['output_path'],'wb') as w:
 w.setnchannels(1);w.setsampwidth(2);w.setframerate(24000);w.writeframes(bytes(480))
json.dump({'ok':True,'protocol':1,'phonemes':'hɛloʊ'},open(a.response,'w'))
"#).unwrap();
        KokoroProvider {
            config: KokoroConfig {
                python: "python3".into(),
                runtime_script: script,
                model_config: root.join("config.json"),
                model_weights: root.join("weights.pth"),
                voices_directory: root.join("voices"),
                cache_directory: root.join("cache"),
                model_version: "mock-v1".into(),
                timeout: Duration::from_secs(5),
            },
        }
    }
    #[test]
    fn cache_roundtrip_and_corruption_regenerate() {
        let provider = mock_provider();
        let request = SpeechRequest {
            text: "hello".into(),
            voice: VoiceSettings::default(),
        };
        let first = provider.synthesize(request.clone()).unwrap();
        assert!(!first.cached);
        assert!(provider.synthesize(request.clone()).unwrap().cached);
        fs::write(&first.audio_path, b"corrupt").unwrap();
        assert!(!provider.synthesize(request.clone()).unwrap().cached);
        fs::write(&provider.config.model_weights, b"updated model").unwrap();
        let changed = provider.synthesize(request).unwrap();
        assert_ne!(first.cache_key, changed.cache_key);
        fs::remove_dir_all(provider.config.model_config.parent().unwrap()).unwrap();
    }
    #[test]
    fn relative_cache_directory_produces_absolute_audio_assets() {
        let mut provider = mock_provider();
        let name = provider
            .config
            .model_config
            .parent()
            .unwrap()
            .file_name()
            .unwrap();
        provider.config.cache_directory = PathBuf::from(name).join("cache");
        let result = provider
            .synthesize(SpeechRequest {
                text: "hello".into(),
                voice: VoiceSettings::default(),
            })
            .unwrap();
        assert!(result.audio_path.is_absolute());
        assert!(result.audio_path.is_file());
        fs::remove_dir_all(provider.config.cache_directory.parent().unwrap()).unwrap();
        fs::remove_dir_all(provider.config.model_config.parent().unwrap()).unwrap();
    }

    #[test]
    fn parallel_fixtures_never_share_a_directory() {
        let threads: Vec<_> = (0..16).map(|_| std::thread::spawn(mock_provider)).collect();
        let providers: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        let roots: std::collections::BTreeSet<_> = providers
            .iter()
            .map(|p| p.config.model_config.parent().unwrap().to_path_buf())
            .collect();
        assert_eq!(roots.len(), providers.len());
        for root in roots {
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn metadata_reads_are_bounded() {
        let provider = mock_provider();
        let path = provider
            .config
            .model_config
            .parent()
            .unwrap()
            .join("oversized.json");
        fs::File::create(&path)
            .unwrap()
            .set_len(MAX_METADATA_BYTES + 1)
            .unwrap();
        assert!(read_bounded(&path).unwrap_err().0.contains("size limit"));
        fs::remove_dir_all(provider.config.model_config.parent().unwrap()).unwrap();
    }

    #[test]
    fn valid_wav_tampering_and_oversized_cache_metadata_regenerate() {
        let provider = mock_provider();
        let request = SpeechRequest {
            text: "hello".into(),
            voice: VoiceSettings::default(),
        };
        let first = provider.synthesize(request.clone()).unwrap();
        // Preserve the WAV header and sample count while changing its audio.
        let mut wav = fs::read(&first.audio_path).unwrap();
        let last = wav.len() - 1;
        wav[last] ^= 1;
        fs::write(&first.audio_path, wav).unwrap();
        assert!(!provider.synthesize(request.clone()).unwrap().cached);
        fs::File::create(first.audio_path.with_extension("json"))
            .unwrap()
            .set_len(MAX_METADATA_BYTES + 1)
            .unwrap();
        assert!(!provider.synthesize(request).unwrap().cached);
        fs::remove_dir_all(provider.config.model_config.parent().unwrap()).unwrap();
    }

    #[test]
    fn cancellation_terminates_running_sidecar_and_cleans_staging() {
        let provider = mock_provider();
        let root = provider.config.model_config.parent().unwrap().to_path_buf();
        let started = root.join("started");
        let finished = root.join("finished");
        fs::write(
            &provider.config.runtime_script,
            "import pathlib,time\np=pathlib.Path(__file__).parent\n(p/'started').write_text('ready')\ntime.sleep(30)\n(p/'finished').write_text('unexpected')\n",
        ).unwrap();
        let cancel = CancellationToken::new();
        let worker_cancel = cancel.clone();
        let cache = provider.config.cache_directory.clone();
        let worker = thread::spawn(move || {
            provider.synthesize_cancellable(
                SpeechRequest {
                    text: "hello".into(),
                    voice: VoiceSettings::default(),
                },
                &worker_cancel,
            )
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while !started.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let did_start = started.exists();
        cancel.cancel();
        let error = worker.join().unwrap().unwrap_err();
        assert!(did_start, "sidecar must start before cancellation");
        assert_eq!(error.0, "Speech generation cancelled");
        assert!(!finished.exists());
        assert_eq!(
            fs::read_dir(&cache).unwrap().count(),
            0,
            "cancelled work must leave neither staging nor published cache entries"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancelled_cache_hit_preserves_success_and_does_not_launch_sidecar() {
        let provider = mock_provider();
        let root = provider.config.model_config.parent().unwrap().to_path_buf();
        // The sentinel is part of an unchanged runtime identity for both calls.
        let script = fs::read_to_string(&provider.config.runtime_script).unwrap();
        fs::write(
            &provider.config.runtime_script,
            format!(
            "import pathlib\n(pathlib.Path(__file__).parent/'launched').write_text('yes')\n{script}"
        ),
        )
        .unwrap();
        let request = SpeechRequest {
            text: "hello".into(),
            voice: VoiceSettings::default(),
        };
        let first = provider.synthesize(request.clone()).unwrap();
        let original = fs::read(&first.audio_path).unwrap();
        fs::remove_file(root.join("launched")).unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let error = provider
            .synthesize_cancellable(request.clone(), &cancel)
            .unwrap_err();
        assert_eq!(error.0, "Speech generation cancelled");
        assert!(!root.join("launched").exists());
        assert_eq!(fs::read(&first.audio_path).unwrap(), original);
        assert!(provider.synthesize(request).unwrap().cached);
        assert!(!root.join("launched").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn default_provider_checks_cancellation_after_blocking_synthesis() {
        struct BoundaryProvider(CancellationToken);
        impl SpeechProvider for BoundaryProvider {
            fn synthesize(&self, _: SpeechRequest) -> Result<SpeechResult, SpeechError> {
                self.0.cancel();
                Ok(SpeechResult {
                    audio_path: PathBuf::new(),
                    sample_rate: 24000,
                    sample_count: 1,
                    cache_key: "fixture".into(),
                    phonemes: String::new(),
                    cached: false,
                })
            }
        }
        let cancel = CancellationToken::new();
        let provider = BoundaryProvider(cancel.clone());
        let error = provider
            .synthesize_cancellable(
                SpeechRequest {
                    text: "hello".into(),
                    voice: VoiceSettings::default(),
                },
                &cancel,
            )
            .unwrap_err();
        assert_eq!(error.0, "Speech generation cancelled");
    }

    #[test]
    fn worker_timeout_is_an_error() {
        let mut provider = mock_provider();
        fs::write(
            &provider.config.runtime_script,
            "import time; time.sleep(30)",
        )
        .unwrap();
        provider.config.timeout = Duration::from_millis(40);
        let error = provider
            .synthesize(SpeechRequest {
                text: "hello".into(),
                voice: VoiceSettings::default(),
            })
            .unwrap_err();
        assert!(error.0.contains("timed out"));
        fs::remove_dir_all(provider.config.model_config.parent().unwrap()).unwrap();
    }
}
