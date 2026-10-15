//! Blocking provider boundary. Call on a worker, never on the desktop event loop.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fmt, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
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
        if self.text.trim().is_empty() || self.text.len() > 16000 {
            return Err(SpeechError("text must contain 1..16000 bytes".into()));
        }
        if !self.voice.speed.is_finite() || !(0.5..=2.0).contains(&self.voice.speed) {
            return Err(SpeechError(
                "speed must be finite and between 0.5 and 2".into(),
            ));
        }
        if !["en-us", "en-gb"].contains(&self.voice.language.as_str()) {
            return Err(SpeechError("only en-us and en-gb are supported".into()));
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
pub trait SpeechProvider: Send + Sync {
    fn synthesize(&self, request: SpeechRequest) -> Result<SpeechResult, SpeechError>;
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
        request.validate()?;
        let c = &self.config;
        if c.timeout.is_zero() || c.model_version.trim().is_empty() {
            return Err(SpeechError(
                "model version and positive timeout are required".into(),
            ));
        }
        let voice = c
            .voices_directory
            .join(format!("{}.pt", request.voice.preset));
        // Hash file contents, so replacing an asset cannot return stale speech.
        let identity = serde_json::to_vec(&(
            1,
            &request,
            &c.model_version,
            fingerprint(&c.model_config)?,
            fingerprint(&c.model_weights)?,
            fingerprint(&voice)?,
            fingerprint(&c.runtime_script)?,
        ))?;
        let key = format!("{:x}", Sha256::digest(identity));
        fs::create_dir_all(&c.cache_directory)?;
        let audio_path = c.cache_directory.join(format!("{key}.wav"));
        let metadata_path = c.cache_directory.join(format!("{key}.json"));
        if let Ok(bytes) = fs::read(&metadata_path) {
            if let Ok(mut result) = serde_json::from_slice::<SpeechResult>(&bytes) {
                if let Ok((rate, count)) = validate_wav(&audio_path) {
                    if result.cache_key == key
                        && result.sample_rate == rate
                        && result.sample_count == count
                    {
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
        let staging = c
            .cache_directory
            .join(format!(".{key}-{}-{nonce}", std::process::id()));
        fs::create_dir(&staging)?;
        let outcome = (|| {
            let output = staging.join("speech.wav");
            let request_path = staging.join("request.json");
            let response_path = staging.join("response.json");
            fs::write(
                &request_path,
                serde_json::to_vec(
                    &serde_json::json!({"protocol":1,"request":request,"config_path":c.model_config,"model_path":c.model_weights,"voice_path":voice,"output_path":output}),
                )?,
            )?;
            let mut child = Command::new(&c.python)
                .arg(&c.runtime_script)
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
            let bytes = fs::read(&response_path).map_err(|e| {
                SpeechError(format!("runtime exited {status} without a response: {e}"))
            })?;
            if bytes.len() > 1_000_000 {
                return Err(SpeechError("runtime response too large".into()));
            }
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
            fs::rename(&output, &audio_path)?;
            let mut meta = fs::File::create(staging.join("metadata.json"))?;
            meta.write_all(&serde_json::to_vec(&result)?)?;
            meta.sync_all()?;
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
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("motion-tts-test-{}-{nonce}", std::process::id()));
        fs::create_dir_all(root.join("voices")).unwrap();
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
