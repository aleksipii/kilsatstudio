//! Explicit, process-isolated Metal fixture evidence. Never opens an adapter on the UI thread.
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct Evidence {
    pub report: String,
    pub rgba: Vec<u8>,
}
pub struct Worker {
    tasks: mpsc::SyncSender<(PathBuf, PathBuf, Arc<AtomicBool>)>,
    results: mpsc::Receiver<Result<Evidence, String>>,
    cancel: Option<Arc<AtomicBool>>,
    busy: bool,
}
impl Default for Worker {
    fn default() -> Self {
        let (tasks, rx) = mpsc::sync_channel::<(PathBuf, PathBuf, Arc<AtomicBool>)>(1);
        let (tx, results) = mpsc::sync_channel(1);
        thread::spawn(move || {
            while let Ok((helper, output, cancel)) = rx.recv() {
                let result = std::panic::catch_unwind(|| run(&helper, &output, &cancel))
                    .unwrap_or_else(|_| Err("GPU helper worker panicked".into()));
                if tx.send(result).is_err() {
                    break;
                }
            }
        });
        Self {
            tasks,
            results,
            cancel: None,
            busy: false,
        }
    }
}
impl Worker {
    pub fn start(&mut self, helper: PathBuf, output: PathBuf) -> Result<(), String> {
        if self.busy {
            return Err("GPU check already running".into());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.tasks
            .try_send((helper, output, cancel.clone()))
            .map_err(|e| e.to_string())?;
        self.cancel = Some(cancel);
        self.busy = true;
        Ok(())
    }
    pub fn poll(&mut self) -> Option<Result<Evidence, String>> {
        let result = self.results.try_recv().ok()?;
        self.busy = false;
        self.cancel = None;
        Some(result)
    }
    pub fn busy(&self) -> bool {
        self.busy
    }
    pub fn cancel(&self) {
        if let Some(c) = &self.cancel {
            c.store(true, Ordering::Relaxed)
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel();
    }
}

pub fn bundled_helper() -> Result<PathBuf, String> {
    if !cfg!(target_os = "macos") {
        return Err("Metal check requires the packaged macOS preview on a physical Mac.".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let macos = exe.parent().ok_or("Missing app executable directory")?;
    if macos.file_name().and_then(|s| s.to_str()) != Some("MacOS") {
        return Err("Use the packaged macOS preview containing the signed GPU helper.".into());
    }
    let contents = macos.parent().ok_or("Missing app Contents")?;
    let helper = contents.join("Resources/bin/gpu_probe");
    if std::fs::symlink_metadata(&helper)
        .map_err(|_| "This preview has no GPU helper. Install a newer Mac preview package.")?
        .file_type()
        .is_symlink()
    {
        return Err("Symlinked GPU helper refused".into());
    }
    if !helper.is_file() {
        return Err("Bundled GPU helper is missing".into());
    }
    Ok(helper)
}
fn run(helper: &Path, output: &Path, cancel: &AtomicBool) -> Result<Evidence, String> {
    if !output.is_absolute() || output.file_name().is_none() {
        return Err("Choose a new absolute output directory".into());
    }
    if output.symlink_metadata().is_ok() {
        return Err("Output already exists; choose a new directory".into());
    }
    if !output.parent().is_some_and(Path::is_dir) {
        return Err("Output parent must already exist".into());
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".into());
    }
    let mut child = Command::new(helper)
        .arg("--metal")
        .arg(output)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Cannot start bundled GPU helper: {e}"))?;
    let started = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) || started.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(
                "GPU check cancelled or timed out. Any partial output is retained for inspection."
                    .into(),
            );
        }
        match child.try_wait(){Ok(Some(status)) if status.success()=>break,Ok(Some(status))=>return Err(format!("GPU helper failed ({status}); inspect output if present. No hardware acceptance recorded.")),Err(e)=>{let _=child.kill();let _=child.wait();return Err(e.to_string());},Ok(None)=>thread::sleep(Duration::from_millis(20))}
    }
    validate(output)
}
fn bounded(path: &Path, max: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > max {
        return Err("Invalid or oversized GPU evidence file".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > max {
        return Err("GPU evidence grew beyond limit".into());
    }
    Ok(bytes)
}
fn validate(output: &Path) -> Result<Evidence, String> {
    let bytes = bounded(&output.join("report.json"), 65536)?;
    let report: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if report["schema_version"] != 1
        || report["fixture"] != "original-offscreen-v1"
        || report["backend"] != "Metal"
        || !matches!(
            report["device_type"].as_str(),
            Some("IntegratedGpu" | "DiscreteGpu")
        )
        || report["width"] != 256
        || report["height"] != 256
        || report["format"] != "rgba8unorm-linear"
        || report["row_padding_removed"] != true
        || report["interior_probes"] != "passed-tolerance-2"
        || report["full_scene_parity"] != false
        || report["validation_errors"]
            .as_array()
            .is_none_or(|a| !a.is_empty())
        || report["adapter"].as_str().is_none_or(str::is_empty)
    {
        return Err("GPU report failed Metal hardware fixture checks".into());
    }
    let rgba = bounded(&output.join("fixture.rgba"), 256 * 256 * 4)?;
    if rgba.len() != 256 * 256 * 4 {
        return Err("GPU raw image has incorrect size".into());
    }
    Ok(Evidence {
        report: String::from_utf8(bytes).map_err(|e| e.to_string())?,
        rgba,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_must_be_absolute() {
        assert!(run(
            Path::new("unused"),
            Path::new("relative"),
            &AtomicBool::new(false)
        )
        .unwrap_err()
        .contains("absolute"));
    }
    #[test]
    fn evidence_rejects_software() {
        let root = std::env::temp_dir().join(format!("kilsat-gpu-evidence-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("report.json"),br#"{"schema_version":1,"fixture":"original-offscreen-v1","backend":"Metal","device_type":"Cpu"}"#).unwrap();
        assert!(validate(&root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn explicit_process_is_cancelled_and_reaped() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("kilsat-gpu-cancel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        let helper = root.join("helper");
        std::fs::write(&helper, "#!/bin/sh\nexec sleep 20\n").unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let trigger = cancel.clone();
        let thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(80));
            trigger.store(true, Ordering::Relaxed);
        });
        let started = Instant::now();
        assert!(run(&helper, &root.join("output"), &cancel)
            .unwrap_err()
            .contains("cancelled"));
        thread.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(!root.join("output").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn existing_output_is_preserved() {
        let root = std::env::temp_dir().join(format!("kilsat-gpu-existing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("keep"), b"mine").unwrap();
        assert!(run(Path::new("unused"), &root, &AtomicBool::new(false))
            .unwrap_err()
            .contains("already exists"));
        assert_eq!(std::fs::read(root.join("keep")).unwrap(), b"mine");
        std::fs::remove_dir_all(root).unwrap();
    }
}
