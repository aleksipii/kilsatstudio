//! Bounded local preflight; never loads a model or downloads assets.
use std::{
    fs::{self, File},
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};
const LIMIT: u64 = 64 * 1024;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub struct Request {
    pub python: String,
    pub doctor: PathBuf,
    pub config: PathBuf,
    pub model: PathBuf,
    pub voices: Vec<PathBuf>,
}
pub struct Worker {
    tx: mpsc::SyncSender<Request>,
    rx: mpsc::Receiver<Result<String, String>>,
    stop: Arc<AtomicBool>,
}
impl Worker {
    pub fn new() -> Self {
        let (tx, requests) = mpsc::sync_channel::<Request>(1);
        let (updates, rx) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let shutdown = stop.clone();
        std::thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                let result = run(request, Duration::from_secs(30), &shutdown);
                if updates.send(result).is_err() {
                    break;
                }
            }
        });
        Self { tx, rx, stop }
    }
    pub fn submit(&self, request: Request) -> Result<(), String> {
        self.tx
            .try_send(request)
            .map_err(|e| format!("Preflight queue: {e}"))
    }
    pub fn poll(&self) -> Option<Result<String, String>> {
        self.rx.try_recv().ok()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
struct OutputDir(PathBuf);
impl Drop for OutputDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn output_dir() -> Result<OutputDir, String> {
    for _ in 0..100 {
        let path = std::env::temp_dir().join(format!(
            "kilsat-preflight-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(OutputDir(path)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Cannot create preflight output: {e}")),
        }
    }
    Err("Cannot allocate preflight output directory".into())
}
fn read_output(path: &PathBuf) -> Result<String, String> {
    let mut text = String::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(LIMIT + 1)
        .read_to_string(&mut text)
        .map_err(|e| format!("Invalid preflight output: {e}"))?;
    if text.len() as u64 > LIMIT {
        return Err("Preflight output exceeded 64 KiB".into());
    }
    Ok(text)
}
fn run(request: Request, timeout: Duration, stop: &AtomicBool) -> Result<String, String> {
    if !request.doctor.is_file() {
        return Err(format!(
            "Preflight script missing: {}. Install doctor.py beside runtime.py.",
            request.doctor.display()
        ));
    }
    let dir = output_dir()?;
    let stdout = dir.0.join("stdout");
    let stderr = dir.0.join("stderr");
    let mut command = Command::new(&request.python);
    command
        .arg(&request.doctor)
        .arg("--config")
        .arg(request.config)
        .arg("--model")
        .arg(request.model);
    for voice in request.voices {
        command.arg("--voice").arg(voice);
    }
    command
        .env("HF_HUB_OFFLINE", "1")
        .env("TRANSFORMERS_OFFLINE", "1")
        .stdin(Stdio::null());
    command
        .stdout(File::create(&stdout).map_err(|e| e.to_string())?)
        .stderr(File::create(&stderr).map_err(|e| e.to_string())?);
    let mut child = command.spawn().map_err(|e| {
        format!("Cannot start configured Python: {e}. Set an installed Python executable.")
    })?;
    let start = Instant::now();
    let status = loop {
        let excess = [&stdout, &stderr]
            .iter()
            .any(|p| fs::metadata(p).is_ok_and(|m| m.len() > LIMIT));
        if stop.load(Ordering::Relaxed) || start.elapsed() >= timeout || excess {
            let _ = child.kill();
            let _ = child.wait();
            return Err(if excess {
                "Preflight output exceeded 64 KiB"
            } else {
                "Preflight stopped or timed out. Check the Python executable and retry."
            }
            .into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Cannot monitor preflight: {e}"));
            }
        }
    };
    let out = read_output(&stdout)?;
    let err = read_output(&stderr)?;
    summarize(&out, status.code(), &err)
}
fn summarize(output: &str, code: Option<i32>, stderr: &str) -> Result<String, String> {
    let report: serde_json::Value = serde_json::from_str(output).map_err(|_| {
        format!(
            "Python preflight returned invalid JSON (exit {code:?}). {}",
            stderr.chars().take(500).collect::<String>()
        )
    })?;
    if report["schema_version"] != 1 || !report["ok"].is_boolean() || !report["errors"].is_array() {
        return Err("Unsupported preflight report; update doctor.py beside runtime.py.".into());
    }
    if code == Some(0)
        && report["ok"] == true
        && report["errors"].as_array().is_some_and(Vec::is_empty)
    {
        Ok("Local preflight passed. Packages and assets were checked; actual CPU synthesis and audio playback remain to be tested.".into())
    } else {
        let errors = report["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| v.to_string())
            })
            .collect::<Vec<_>>()
            .join("\n");
        Err(format!(
            "Local setup incomplete (exit {code:?}):\n{errors}\nNo assets were downloaded."
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protocol_rejects_false_success() {
        assert!(summarize(r#"{"schema_version":1,"ok":true,"errors":[]}"#, Some(1), "").is_err());
        assert!(summarize("{}", Some(0), "").is_err());
        assert!(summarize(
            r#"{"schema_version":1,"ok":true,"errors":[],"future":1}"#,
            Some(0),
            ""
        )
        .is_ok());
    }
    #[cfg(unix)]
    #[test]
    fn subprocess_times_out_and_can_retry() {
        let dir = output_dir().unwrap();
        let script = dir.0.join("doctor.py");
        fs::write(&script, "while :; do :; done").unwrap();
        let request = || Request {
            python: "/bin/sh".into(),
            doctor: script.clone(),
            config: "c".into(),
            model: "m".into(),
            voices: vec![],
        };
        assert!(run(
            request(),
            Duration::from_millis(60),
            &AtomicBool::new(false)
        )
        .unwrap_err()
        .contains("timed out"));
        fs::write(
            &script,
            "printf '%s' '{\"schema_version\":1,\"ok\":true,\"errors\":[]}'",
        )
        .unwrap();
        assert!(run(request(), Duration::from_secs(2), &AtomicBool::new(false)).is_ok());
    }
    #[cfg(unix)]
    #[test]
    fn subprocess_output_is_bounded() {
        let dir = output_dir().unwrap();
        let script = dir.0.join("doctor.py");
        fs::write(&script, "while :; do printf 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'; done").unwrap();
        let request = Request {
            python: "/bin/sh".into(),
            doctor: script,
            config: "c".into(),
            model: "m".into(),
            voices: vec![],
        };
        assert!(
            run(request, Duration::from_secs(2), &AtomicBool::new(false))
                .unwrap_err()
                .contains("64 KiB")
        );
    }
}
