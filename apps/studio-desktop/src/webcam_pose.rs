//! Explicit local one-shot acquisition. Construction never opens a camera.
use eframe::egui;
use std::{
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Panel {
    context: String,
    receiver: Option<mpsc::Receiver<Result<f64, String>>>,
    cancel: Option<Arc<AtomicBool>>,
    angle: Option<f64>,
    requested: Option<f64>,
    invert: bool,
    status: String,
}

fn decode_pose(bytes: &[u8]) -> Result<f64, String> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Pose {
        roll: f64,
        yaw: Option<f64>,
        pitch: Option<f64>,
    }
    if bytes.len() > 4096 {
        return Err("Camera pose exceeds protocol limit".into());
    }
    let pose: Pose = serde_json::from_slice(bytes).map_err(|e| format!("Camera pose: {e}"))?;
    if [Some(pose.roll), pose.yaw, pose.pitch]
        .into_iter()
        .flatten()
        .any(|v| !v.is_finite() || v.abs() > std::f64::consts::PI)
    {
        return Err("Camera returned an invalid angle".into());
    }
    let angle = pose.roll.to_degrees();
    if angle.abs() > 90.0 {
        return Err("Head tilt exceeds 90 degrees; face the camera and retry".into());
    }
    Ok(angle)
}

fn bundled_helper() -> Result<PathBuf, String> {
    if !cfg!(target_os = "macos") {
        return Err("Head tilt capture requires the packaged macOS app".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let macos = exe.parent().ok_or("Missing app directory")?;
    if macos.file_name().and_then(|s| s.to_str()) != Some("MacOS") {
        return Err("Use the packaged Mac app containing the signed head-pose helper".into());
    }
    let path = macos
        .parent()
        .ok_or("Missing Contents directory")?
        .join("Resources/bin/head_pose");
    let metadata = std::fs::symlink_metadata(&path).map_err(|e| format!("Camera helper: {e}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Camera helper must be a packaged regular file".into());
    }
    Ok(path)
}

fn run(path: PathBuf, cancel: Arc<AtomicBool>) -> Result<f64, String> {
    let mut child = Command::new(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Start camera helper: {e}"))?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
        if cancel.load(Ordering::Relaxed) || started.elapsed() > Duration::from_secs(60) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Camera capture cancelled or timed out".into());
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let mut stdout = Vec::new();
    child
        .stdout
        .take()
        .ok_or("Missing camera output")?
        .take(4097)
        .read_to_end(&mut stdout)
        .map_err(|e| e.to_string())?;
    if !status.success() {
        let mut stderr = String::new();
        child
            .stderr
            .take()
            .ok_or("Missing camera error")?
            .take(4096)
            .read_to_string(&mut stderr)
            .map_err(|e| e.to_string())?;
        return Err(format!("Camera: {}", stderr.trim()));
    }
    decode_pose(&stdout)
}

impl Panel {
    pub fn set_context(&mut self, context: &str) {
        if self.context != context {
            if let Some(cancel) = &self.cancel {
                cancel.store(true, Ordering::Relaxed);
            }
            self.receiver = None;
            self.cancel = None;
            self.angle = None;
            self.requested = None;
            self.status.clear();
            self.context = context.into();
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(result) => {
                    self.receiver = None;
                    self.cancel = None;
                    match result {
                        Ok(angle) => {
                            self.angle = Some(angle);
                            self.status =
                                "Kallistus mitattu. Tarkista tulos ennen kopiointia pään luonnokseen.".into();
                        }
                        Err(e) => {
                            self.angle = None;
                            self.status = e;
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.cancel = None;
                    self.status = "Camera worker stopped unexpectedly".into();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        egui::CollapsingHeader::new("Webkamera · yksittäinen pään kallistus").show(ui, |ui| {
            ui.weak("Mittaus tehdään paikallisesti Macin kameralla. Kuvia tai ääntä ei tallenneta eikä lähetetä. Ei jatkuvaa liikkeenseurantaa.");
            if self.receiver.is_none() {
                if ui.button("Kaappaa pään kallistus").clicked() {
                    self.angle=None; self.requested=None;
                    match bundled_helper() {
                        Ok(path) => {
                            let (tx,rx)=mpsc::sync_channel(1);
                            let cancel=Arc::new(AtomicBool::new(false));
                            let worker_cancel=cancel.clone();
                            std::thread::spawn(move || { let _=tx.send(run(path,worker_cancel)); });
                            self.receiver=Some(rx); self.cancel=Some(cancel);
                            self.status="Allow camera access in the macOS prompt, then face the camera.".into();
                        }
                        Err(e) => self.status=e,
                    }
                }
            } else {
                ui.spinner(); ui.ctx().request_repaint_after(Duration::from_millis(50));
                if ui.button("Peru kaappaus").clicked() {
                    if let Some(cancel)=&self.cancel { cancel.store(true,Ordering::Relaxed); }
                }
            }
            if let Some(angle)=self.angle {
                ui.label(format!("Captured tilt: {angle:.1}°"));
                ui.checkbox(&mut self.invert,"Käännä kallistuksen suunta");
                if ui.button("Kopioi kallistus luonnokseen").clicked() {
                    self.requested=Some(if self.invert {-angle} else {angle});
                }
                ui.weak("Only 2D roll is copied. Use Apply to save a head keyframe; inspect orientation on your Mac.");
            }
            if !self.status.is_empty() {ui.label(&self.status);}
        });
    }
    pub fn take_roll_degrees(&mut self) -> Option<f64> {
        self.requested.take()
    }
}
impl Drop for Panel {
    fn drop(&mut self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_protocol_is_bounded_and_angles_validated() {
        assert!(
            (decode_pose(br#"{"roll":0.5,"yaw":null,"pitch":0.1}"#).unwrap()
                - 0.5_f64.to_degrees())
            .abs()
                < 0.0001
        );
        for v in [
            br#"{"roll":3.2}"#.as_slice(),
            br#"{"roll":0,"unexpected":1}"#,
            br#"{"roll":null}"#,
            br#"{"yaw":0}"#,
        ] {
            assert!(decode_pose(v).is_err());
        }
        assert!(decode_pose(&vec![b' '; 4097]).is_err());
    }
    #[test]
    fn default_is_off_and_context_change_discards_and_cancels() {
        let mut p = Panel::default();
        assert!(p.receiver.is_none());
        p.requested = Some(12.0);
        assert_eq!(p.take_roll_degrees(), Some(12.0));
        assert_eq!(p.take_roll_degrees(), None);
        let cancel = Arc::new(AtomicBool::new(false));
        p.cancel = Some(cancel.clone());
        p.angle = Some(2.0);
        p.requested = Some(2.0);
        p.set_context("other actor");
        assert!(cancel.load(Ordering::Relaxed));
        assert!(p.angle.is_none());
        assert!(p.take_roll_degrees().is_none());
    }
}
