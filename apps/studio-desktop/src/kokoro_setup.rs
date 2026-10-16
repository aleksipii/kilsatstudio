//! One bounded background worker for opt-in Kokoro installation and local tests.
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug)]
pub struct Profile {
    pub python: PathBuf,
    pub config: PathBuf,
    pub model: PathBuf,
    pub voices: PathBuf,
    pub cache: PathBuf,
    pub revision: String,
}
pub enum Task {
    Setup {
        python: Option<PathBuf>,
        root: PathBuf,
        installer: PathBuf,
        manifest: PathBuf,
        allow_downloads: bool,
    },
    ConnectExisting {
        root: PathBuf,
    },
    Test {
        profile: Profile,
        smoke_script: PathBuf,
        voice_preset: String,
        output_root: PathBuf,
    },
}
#[derive(Debug)]
pub enum Outcome {
    Connected(Profile),
    Tested {
        wav: PathBuf,
        report: PathBuf,
        metrics: serde_json::Value,
        audio: Arc<motion_audio::AudioBuffer>,
    },
}
#[derive(Debug)]
pub enum Update {
    Progress(String),
    Finished(Result<Outcome, String>),
}
pub struct Worker {
    requests: SyncSender<Task>,
    updates: Receiver<Update>,
    cancel: Arc<AtomicBool>,
    busy: Arc<AtomicBool>,
}
impl Worker {
    pub fn spawn() -> Self {
        let (requests, incoming) = mpsc::sync_channel(1);
        let (outgoing, updates) = mpsc::sync_channel(32);
        let cancel = Arc::new(AtomicBool::new(false));
        let busy = Arc::new(AtomicBool::new(false));
        let (flag, active) = (cancel.clone(), busy.clone());
        thread::spawn(move || {
            while let Ok(task) = incoming.recv() {
                let result = execute(task, &flag, &outgoing);
                active.store(false, Ordering::Release);
                if outgoing.send(Update::Finished(result)).is_err() {
                    break;
                }
            }
        });
        Self {
            requests,
            updates,
            cancel,
            busy,
        }
    }
    pub fn submit(&self, task: Task) -> Result<(), String> {
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err("Kokoro setup is already running".into());
        }
        self.cancel.store(false, Ordering::Release);
        self.requests.try_send(task).map_err(|error| {
            self.busy.store(false, Ordering::Release);
            match error {
                TrySendError::Full(_) => "Kokoro queue is full",
                TrySendError::Disconnected(_) => "Kokoro worker stopped",
            }
            .into()
        })
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
    pub fn poll(&self) -> Option<Update> {
        self.updates.try_recv().ok()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel();
    }
}
pub fn default_root() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Library/Application Support/Kilsat Studio/Kokoro")
}
pub fn helper_paths(runtime: &Path) -> (PathBuf, PathBuf) {
    let parent = runtime.parent().unwrap_or_else(|| Path::new("."));
    (
        parent.join("setup_kokoro.py"),
        parent.join("setup-manifest.json"),
    )
}
fn progress(updates: &SyncSender<Update>, message: impl Into<String>) {
    let _ = updates.try_send(Update::Progress(message.into()));
}
fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    if file.metadata().map_err(|e| e.to_string())?.len() > 1024 * 1024 {
        return Err("Kokoro JSON exceeds 1 MiB".into());
    }
    serde_json::from_reader(file).map_err(|e| format!("Invalid {}: {e}", path.display()))
}
fn profile(root: &Path) -> Result<Profile, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("Kokoro setup folder: {e}"))?;
    let profile_path = root
        .join("profile.json")
        .canonicalize()
        .map_err(|e| format!("Missing profile: {e}"))?;
    if !profile_path.starts_with(&root) {
        return Err("Profile file escapes the setup folder".into());
    }
    let value = read_json(&profile_path)?;
    if value["schema_version"].as_u64() != Some(1) {
        return Err("Unsupported Kokoro profile version".into());
    }
    let field = |name: &str, directory: bool| -> Result<PathBuf, String> {
        let path = PathBuf::from(
            value[name]
                .as_str()
                .ok_or_else(|| format!("Missing profile {name}"))?,
        );
        if !path.is_absolute() || !path.starts_with(&root) {
            return Err(format!(
                "Profile {name} must be absolute and inside the setup folder"
            ));
        }
        if name == "python"
            && !path
                .parent()
                .ok_or("Missing Python parent")?
                .canonicalize()
                .map_err(|e| e.to_string())?
                .starts_with(&root)
        {
            return Err("Python parent escapes setup folder".into());
        }
        let resolved = path
            .canonicalize()
            .map_err(|e| format!("Profile {name}: {e}"))?;
        // venv Python may legitimately be a symlink to its host interpreter.
        if name != "python" && !resolved.starts_with(&root) {
            return Err(format!("Profile {name} escapes the setup folder"));
        }
        if if directory {
            !resolved.is_dir()
        } else {
            !resolved.is_file()
        } {
            return Err(format!("Invalid profile {name} path"));
        }
        Ok(path)
    };
    let python = field("python", false)?;
    let voices = field("voices", true)?;
    for name in ["af_heart.pt", "am_michael.pt"] {
        let voice = voices
            .join(name)
            .canonicalize()
            .map_err(|e| format!("Missing voice {name}: {e}"))?;
        if !voice.starts_with(&root) || !voice.is_file() {
            return Err(format!("Invalid voice {name}"));
        }
    }
    let revision = value["revision"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 512)
        .ok_or("Missing model revision")?
        .to_owned();
    crate::managed_license_evidence::validate(&root, &value)?;
    Ok(Profile {
        python,
        config: field("config", false)?,
        model: field("model", false)?,
        voices,
        cache: field("cache", true)?,
        revision,
    })
}
fn host_python(
    override_path: Option<PathBuf>,
    cancel: &AtomicBool,
    updates: &SyncSender<Update>,
    work: &Path,
) -> Result<PathBuf, String> {
    let candidates = override_path.map(|p| vec![p]).unwrap_or_else(|| {
        [
            "/opt/homebrew/bin/python3.12",
            "/opt/homebrew/bin/python3.11",
            "/opt/homebrew/bin/python3.10",
            "/Library/Frameworks/Python.framework/Versions/3.12/bin/python3",
            "/Library/Frameworks/Python.framework/Versions/3.11/bin/python3",
            "/Library/Frameworks/Python.framework/Versions/3.10/bin/python3",
            "/usr/local/bin/python3.12",
            "/usr/local/bin/python3.11",
            "/usr/local/bin/python3.10",
            "/usr/bin/python3",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect()
    });
    for path in candidates {
        if !path.is_absolute() || !path.is_file() {
            continue;
        }
        let mut command = Command::new(&path);
        command.args([
            "-c",
            "import sys; sys.exit(0 if (3,10) <= sys.version_info[:2] <= (3,12) else 1)",
        ]);
        if run(
            command,
            work,
            Duration::from_secs(10),
            Duration::from_secs(1),
            cancel,
            updates,
        )
        .is_ok()
        {
            return Ok(path);
        }
        if cancel.load(Ordering::Acquire) {
            return Err("Cancelled".into());
        }
    }
    Err(
        "Python 3.10–3.12 was not found. Install a supported Python or select its absolute path."
            .into(),
    )
}
fn run(
    mut command: Command,
    work: &Path,
    timeout: Duration,
    grace: Duration,
    cancel: &AtomicBool,
    updates: &SyncSender<Update>,
) -> Result<(), String> {
    let log_path = work.join("process.log");
    let log = File::create(&log_path).map_err(|e| e.to_string())?;
    let err = log.try_clone().map_err(|e| e.to_string())?;
    command.stdin(Stdio::null()).stdout(log).stderr(err);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    if cancel.load(Ordering::Acquire) {
        return Err("Kokoro task cancelled".into());
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Cannot start Kokoro helper: {e}"))?;
    let mut reader = match File::open(log_path) {
        Ok(reader) => reader,
        Err(error) => {
            kill_group(&mut child);
            let _ = child.wait();
            return Err(error.to_string());
        }
    };
    let start = Instant::now();
    let mut stopping = None;
    let mut seen = 0u64;
    let mut tail = String::new();
    let result = (|| loop {
        let length = reader.metadata().map_err(|e| e.to_string())?.len();
        if length > 8 * 1024 * 1024 {
            kill_group(&mut child);
            let _ = child.wait();
            return Err("Kokoro helper exceeded its 8 MiB log budget".into());
        }
        if length > seen {
            reader
                .seek(SeekFrom::Start(seen))
                .map_err(|e| e.to_string())?;
            let mut bytes = vec![0; (length - seen).min(4096) as usize];
            let count = reader.read(&mut bytes).map_err(|e| e.to_string())?;
            seen += count as u64;
            tail = String::from_utf8_lossy(&bytes[..count]).into_owned();
            for line in tail.lines().take(16) {
                let message = serde_json::from_str::<serde_json::Value>(line)
                    .ok()
                    .and_then(|value| value["message"].as_str().map(str::to_owned))
                    .unwrap_or_else(|| line.to_owned());
                progress(updates, message);
            }
        }
        if cancel.load(Ordering::Acquire) || start.elapsed() > timeout {
            if stopping.is_none() {
                let _ = fs::write(work.join("cancel"), b"cancel");
                stopping = Some(Instant::now());
            }
            if stopping.is_some_and(|time| time.elapsed() > grace) {
                kill_group(&mut child);
            }
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if stopping.is_some() {
                return Err(if cancel.load(Ordering::Acquire) {
                    "Kokoro task cancelled"
                } else {
                    "Kokoro task timed out"
                }
                .into());
            }
            return if status.success() {
                Ok(())
            } else {
                Err(format!("Kokoro helper failed ({status}): {tail}"))
            };
        }
        thread::sleep(Duration::from_millis(100));
    })();
    if result.is_err() {
        kill_group(&mut child);
        let _ = child.wait();
    }
    result
}
fn kill_group(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{}", child.id())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}
fn execute(
    task: Task,
    cancel: &AtomicBool,
    updates: &SyncSender<Update>,
) -> Result<Outcome, String> {
    if let Task::ConnectExisting { root } = task {
        return profile(&root).map(Outcome::Connected);
    }
    if matches!(
        &task,
        Task::Setup {
            allow_downloads: false,
            ..
        }
    ) {
        return Err("Download consent is required before installing Kokoro".into());
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let work = std::env::temp_dir().join(format!("kilsat-kokoro-{}-{nonce}", std::process::id()));
    fs::create_dir(&work).map_err(|e| e.to_string())?;
    let result = (|| match task {
        Task::Setup {
            python,
            root,
            installer,
            manifest,
            ..
        } => {
            progress(updates, "Checking local Python…");
            let python = host_python(python, cancel, updates, &work)?;
            let mut command = Command::new(python);
            command
                .arg(installer)
                .arg("--root")
                .arg(&root)
                .arg("--manifest")
                .arg(manifest)
                .arg("--allow-downloads")
                .arg("--cancel-file")
                .arg(work.join("cancel"));
            run(
                command,
                &work,
                Duration::from_secs(3600),
                Duration::from_secs(40),
                cancel,
                updates,
            )?;
            profile(&root).map(Outcome::Connected)
        }
        Task::Test {
            profile,
            smoke_script,
            voice_preset,
            output_root,
        } => {
            if voice_preset.is_empty()
                || !voice_preset
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                return Err("Invalid voice preset".into());
            }
            fs::create_dir_all(&output_root).map_err(|e| e.to_string())?;
            let wav = output_root.join(format!("kokoro-{nonce}.wav"));
            let report = output_root.join(format!("kokoro-{nonce}.json"));
            let voice = profile.voices.join(format!("{voice_preset}.pt"));
            if !voice.is_file() {
                return Err(format!("Missing voice: {}", voice.display()));
            }
            let mut command = Command::new(profile.python);
            command
                .arg(smoke_script)
                .arg("--allow-local-inference")
                .arg("--config")
                .arg(profile.config)
                .arg("--model")
                .arg(profile.model)
                .arg("--voice")
                .arg(voice)
                .arg("--output")
                .arg(&wav)
                .arg("--report")
                .arg(&report);
            run(
                command,
                &work,
                Duration::from_secs(240),
                Duration::from_secs(3),
                cancel,
                updates,
            )?;
            if fs::metadata(&wav).map_err(|e| e.to_string())?.len() > 32 * 1024 * 1024 {
                return Err("Test audio exceeds 32 MiB".into());
            }
            let audio = motion_audio::read_wav(&wav).map_err(|e| e.to_string())?;
            if audio.sample_rate != 24000
                || audio.channels != 1
                || audio.frames() == 0
                || !audio.samples.iter().any(|sample| *sample != 0.0)
            {
                return Err("Test must produce nonempty 24 kHz mono audio".into());
            }
            let metrics = read_json(&report)?;
            if metrics["schema_version"].as_u64() != Some(1)
                || metrics["audio"]["sample_count"].as_u64() != Some(audio.frames() as u64)
                || metrics["audio"]["nonzero_signal"].as_bool() != Some(true)
            {
                return Err("Test evidence does not match the audio".into());
            }
            Ok(Outcome::Tested {
                wav,
                report,
                metrics,
                audio: Arc::new(audio),
            })
        }
        Task::ConnectExisting { .. } => unreachable!(),
    })();
    let _ = fs::remove_dir_all(work);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temporary() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "kilsat-setup-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        path
    }
    #[test]
    fn missing_consent_never_launches_installer() {
        let root = temporary();
        let (tx, _) = mpsc::sync_channel(32);
        let result = execute(
            Task::Setup {
                python: Some("/no/python".into()),
                root: root.join("install"),
                installer: "/no/installer".into(),
                manifest: "/no/manifest".into(),
                allow_downloads: false,
            },
            &AtomicBool::new(false),
            &tx,
        );
        assert!(result.unwrap_err().contains("consent"));
        assert!(!root.join("install").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn profile_rejects_escape_and_unsupported_schema() {
        let root = temporary();
        fs::write(root.join("profile.json"), r#"{"schema_version":2}"#).unwrap();
        assert!(profile(&root).unwrap_err().contains("version"));
        fs::write(
            root.join("profile.json"),
            serde_json::json!({"schema_version":1,"revision":"test","python":"/bin/sh"})
                .to_string(),
        )
        .unwrap();
        assert!(profile(&root).unwrap_err().contains("inside"));
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn stub_process_timeout_and_cancellation_are_reaped() {
        let root = temporary();
        let (tx, _) = mpsc::sync_channel(32);
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "while :; do :; done"]);
        let result = run(
            command,
            &root,
            Duration::from_millis(10),
            Duration::from_millis(10),
            &AtomicBool::new(false),
            &tx,
        );
        assert!(result.unwrap_err().contains("timed out"));
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "exit 0"]);
        assert!(run(
            command,
            &root,
            Duration::from_secs(1),
            Duration::from_millis(10),
            &AtomicBool::new(true),
            &tx
        )
        .unwrap_err()
        .contains("cancelled"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn sibling_helpers_are_resolved_without_cwd() {
        let (installer, manifest) = helper_paths(Path::new("/bundle/runtime.py"));
        assert_eq!(installer, Path::new("/bundle/setup_kokoro.py"));
        assert_eq!(manifest, Path::new("/bundle/setup-manifest.json"));
    }
}
