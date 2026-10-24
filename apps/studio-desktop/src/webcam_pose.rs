//! Explicit local camera measurement. Construction never opens a camera.
use crate::direct_manipulation::HeadSample;
use eframe::egui;
use std::{
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};

const MAX_LINE: usize = 4096;
const MAX_STREAM_BYTES: usize = 512 * 1024;
const MAX_STREAM_SAMPLES: usize = 1200;
const MAX_RECORD_SAMPLES: usize = 600;
const MAX_RECORD_SECONDS: f64 = 30.0;
const FRESH: Duration = Duration::from_millis(750);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Shot,
    Stream,
}
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Pose {
    roll: f64,
    yaw: Option<f64>,
    pitch: Option<f64>,
}
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StreamPose {
    seq: u64,
    time: f64,
    roll: f64,
    yaw: Option<f64>,
    pitch: Option<f64>,
}
fn validate_angles(roll: f64, yaw: Option<f64>, pitch: Option<f64>) -> Result<(), String> {
    if [Some(roll), yaw, pitch]
        .into_iter()
        .flatten()
        .any(|v| !v.is_finite() || v.abs() > std::f64::consts::PI)
        || roll.abs() > std::f64::consts::FRAC_PI_2
    {
        return Err("Kameran kulma ei kelpaa; katso kohti kameraa ja yritä uudelleen".into());
    }
    Ok(())
}
fn measured_angle(value: Option<f64>) -> String {
    value.map_or_else(
        || "ei mittausta".into(),
        |v| format!("{:.1}°", v.to_degrees()),
    )
}
fn decode_pose(bytes: &[u8]) -> Result<f64, String> {
    if bytes.len() > MAX_LINE {
        return Err("Camera pose exceeds protocol limit".into());
    }
    let pose: Pose = serde_json::from_slice(bytes).map_err(|e| format!("Camera pose: {e}"))?;
    validate_angles(pose.roll, pose.yaw, pose.pitch)?;
    Ok(pose.roll.to_degrees())
}
#[derive(Default)]
struct Protocol {
    previous: Option<(u64, f64)>,
}
impl Protocol {
    fn decode(&mut self, bytes: &[u8]) -> Result<StreamPose, String> {
        if bytes.is_empty() || bytes.len() > MAX_LINE {
            return Err("Camera JSONL line exceeds protocol limit".into());
        }
        let pose: StreamPose =
            serde_json::from_slice(bytes).map_err(|e| format!("Camera stream: {e}"))?;
        validate_angles(pose.roll, pose.yaw, pose.pitch)?;
        if !pose.time.is_finite()
            || !(0.0..=60.0).contains(&pose.time)
            || pose.seq >= MAX_STREAM_SAMPLES as u64
        {
            return Err("Camera stream time/sample budget exceeded".into());
        }
        if self.previous.map_or(pose.seq != 0, |(seq, time)| {
            pose.seq != seq + 1 || pose.time <= time
        }) {
            return Err("Camera stream sequence/time is not strictly increasing".into());
        }
        self.previous = Some((pose.seq, pose.time));
        Ok(pose)
    }
}

fn bundled_helper() -> Result<PathBuf, String> {
    if !cfg!(target_os = "macos") {
        return Err("Kameraseuranta tarvitsee paketoidun Mac-sovelluksen".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let macos = exe.parent().ok_or("Missing app directory")?;
    if macos.file_name().and_then(|s| s.to_str()) != Some("MacOS") {
        return Err("Use the packaged Mac app containing its camera helper".into());
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

#[derive(Default)]
struct Control {
    cancel: AtomicBool,
    stop: AtomicBool,
    record: AtomicBool,
}
#[derive(Default)]
struct Shared {
    running: bool,
    latest: Option<(StreamPose, Instant)>,
    recording: bool,
    recorded: Option<Result<Vec<HeadSample>, String>>,
    finished: Option<Result<Option<f64>, String>>,
}
struct Session {
    mode: Mode,
    shared: Arc<Mutex<Shared>>,
    control: Arc<Control>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.control.cancel.store(true, Ordering::Release);
    }
}

enum ReaderEvent {
    Sample(StreamPose),
    Shot(f64),
    End,
    Error(String),
}
fn read_output(
    mut output: impl Read,
    mode: Mode,
    tx: &mpsc::SyncSender<ReaderEvent>,
) -> Result<(), String> {
    if mode == Mode::Shot {
        let mut bytes = Vec::new();
        output
            .take((MAX_LINE + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let angle = decode_pose(&bytes)?;
        tx.send(ReaderEvent::Shot(angle))
            .map_err(|_| "Camera receiver closed".to_string())?;
    } else {
        let mut protocol = Protocol::default();
        let mut line = Vec::new();
        let mut total = 0usize;
        let mut buffer = [0u8; 512];
        loop {
            let n = output.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            total = total.checked_add(n).ok_or("Camera output overflow")?;
            if total > MAX_STREAM_BYTES {
                return Err("Camera stream byte budget exceeded".into());
            }
            for byte in &buffer[..n] {
                if *byte == b'\n' {
                    let sample = protocol.decode(&line)?;
                    line.clear();
                    tx.send(ReaderEvent::Sample(sample))
                        .map_err(|_| "Camera receiver closed".to_string())?;
                } else {
                    if line.len() >= MAX_LINE {
                        return Err("Camera JSONL line exceeds protocol limit".into());
                    }
                    line.push(*byte);
                }
            }
        }
        if !line.is_empty() {
            return Err("Incomplete camera JSONL line".into());
        }
    }
    tx.send(ReaderEvent::End)
        .map_err(|_| "Camera receiver closed".to_string())?;
    Ok(())
}

#[derive(Default)]
struct Recording {
    origin: Option<f64>,
    samples: Vec<HeadSample>,
    received: Option<Instant>,
    started: Option<Instant>,
}
impl Recording {
    fn push(&mut self, pose: &StreamPose) -> Result<bool, String> {
        let origin = *self.origin.get_or_insert(pose.time);
        let seconds = pose.time - origin;
        if seconds > MAX_RECORD_SECONDS || self.samples.len() >= MAX_RECORD_SAMPLES {
            return Ok(false);
        }
        if self
            .samples
            .last()
            .is_some_and(|last| seconds <= last.seconds || seconds - last.seconds > 1.0)
        {
            return Err("Kasvoseurannassa on yli sekunnin katkos; nauhoita uudelleen".into());
        }
        self.samples.push(HeadSample {
            seconds,
            tilt_degrees: pose.roll.to_degrees(),
        });
        self.received = Some(Instant::now());
        Ok(self.samples.len() < MAX_RECORD_SAMPLES && seconds < MAX_RECORD_SECONDS)
    }
    fn finish(self) -> Result<Vec<HeadSample>, String> {
        if self.samples.len() < 2
            || self.samples.last().is_none_or(|s| s.seconds <= 0.0)
            || self
                .received
                .is_none_or(|at| at.elapsed() > Duration::from_secs(1))
        {
            return Err(
                "Nauhoitus tarvitsee vähintään kaksi tuoretta mittausta; yritä uudelleen".into(),
            );
        }
        Ok(self.samples)
    }
}
fn save_recording(
    recording: &mut Option<Recording>,
    shared: &Arc<Mutex<Shared>>,
    control: &Control,
) -> Result<(), String> {
    control.record.store(false, Ordering::Release);
    if let Some(recording) = recording.take() {
        let result = recording.finish();
        let mut state = shared.lock().map_err(|_| "Camera state unavailable")?;
        state.recording = false;
        state.recorded = Some(result);
    }
    Ok(())
}

fn capture(
    path: PathBuf,
    mode: Mode,
    shared: &Arc<Mutex<Shared>>,
    control: &Arc<Control>,
) -> Result<Option<f64>, String> {
    let mut command = Command::new(path);
    if mode == Mode::Stream {
        command.args(["--stream", "--duration", "60", "--fps", "12"]);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Start camera helper: {e}"))?;
    let output = child.stdout.take().ok_or("Missing camera output")?;
    let error = child.stderr.take().ok_or("Missing camera error output")?;
    let (tx, rx) = mpsc::sync_channel(4);
    let reader = std::thread::spawn(move || {
        if let Err(error) = read_output(output, mode, &tx) {
            let _ = tx.send(ReaderEvent::Error(error));
        }
    });
    let errors = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = error.take(4097).read_to_end(&mut bytes);
        (result, bytes)
    });
    let started = Instant::now();
    let mut recording: Option<Recording> = None;
    let mut angle = None;
    let mut output_done = false;
    let mut exit_status = None;
    let outcome = (|| loop {
        if control.cancel.load(Ordering::Acquire) {
            return Err("Kamerakaappaus peruttiin".into());
        }
        if started.elapsed() > Duration::from_secs(if mode == Mode::Stream { 95 } else { 60 }) {
            return Err("Kamerakaappauksen aikaraja ylittyi".into());
        }
        if control.stop.load(Ordering::Acquire) {
            save_recording(&mut recording, shared, control)?;
            return Ok(angle);
        }
        if control.record.load(Ordering::Acquire) && recording.is_none() {
            recording = Some(Recording {
                started: Some(Instant::now()),
                ..Default::default()
            });
        }
        if !control.record.load(Ordering::Acquire) && recording.is_some() {
            save_recording(&mut recording, shared, control)?;
        }
        if recording
            .as_ref()
            .and_then(|r| r.started)
            .is_some_and(|at| at.elapsed() > Duration::from_secs(30))
        {
            save_recording(&mut recording, shared, control)?;
        }
        if recording.as_ref().is_some_and(|r| {
            r.received
                .or(r.started)
                .is_some_and(|at| at.elapsed() > Duration::from_secs(1))
        }) {
            recording = None;
            control.record.store(false, Ordering::Release);
            let mut state = shared.lock().map_err(|_| "Camera state unavailable")?;
            state.recording = false;
            state.recorded = Some(Err(
                "Kasvoseurannassa on yli sekunnin katkos; nauhoita uudelleen".into(),
            ));
        }
        match rx.recv_timeout(Duration::from_millis(25)) {
            Ok(ReaderEvent::Sample(pose)) => {
                shared
                    .lock()
                    .map_err(|_| "Camera state unavailable")?
                    .latest = Some((pose.clone(), Instant::now()));
                if let Some(active) = &mut recording {
                    match active.push(&pose) {
                        Ok(true) => (),
                        Ok(false) => save_recording(&mut recording, shared, control)?,
                        Err(error) => {
                            recording = None;
                            control.record.store(false, Ordering::Release);
                            let mut state =
                                shared.lock().map_err(|_| "Camera state unavailable")?;
                            state.recording = false;
                            state.recorded = Some(Err(error));
                        }
                    }
                }
            }
            Ok(ReaderEvent::Shot(value)) => angle = Some(value),
            Ok(ReaderEvent::End) => output_done = true,
            Ok(ReaderEvent::Error(error)) => return Err(error),
            Err(mpsc::RecvTimeoutError::Timeout) => (),
            Err(mpsc::RecvTimeoutError::Disconnected) => output_done = true,
        }
        if exit_status.is_none() {
            exit_status = child.try_wait().map_err(|e| e.to_string())?;
        }
        if let Some(status) = exit_status.filter(|_| output_done) {
            if !status.success() {
                return Err(format!("Camera helper exited {status}"));
            }
            if mode == Mode::Shot && angle.is_none() {
                return Err("Camera returned no head measurement".into());
            }
            save_recording(&mut recording, shared, control)?;
            return Ok(angle);
        }
    })();
    // Also on cancellation/error, close the receiver before joining a bounded sender.
    let _ = child.kill();
    let _ = child.wait();
    drop(rx);
    let _ = reader.join();
    let detail = errors.join().map_err(|_| "Camera error reader stopped")?;
    match outcome {
        Err(error) if detail.0.is_ok() && !detail.1.is_empty() => {
            let text = String::from_utf8_lossy(&detail.1[..detail.1.len().min(4096)]);
            Err(format!("{error}: {}", text.trim()))
        }
        result => result,
    }
}

#[derive(Default)]
pub struct Panel {
    context: String,
    session: Option<Session>,
    angle: Option<f64>,
    requested: Option<f64>,
    invert: bool,
    draft: Option<Vec<HeadSample>>,
    requested_recording: Option<Vec<HeadSample>>,
    status: String,
}
impl Panel {
    fn clear(&mut self) {
        self.session = None;
        self.angle = None;
        self.requested = None;
        self.draft = None;
        self.requested_recording = None;
    }
    pub fn set_context(&mut self, context: &str) {
        if self.context != context {
            self.clear();
            self.status.clear();
            self.context = context.into();
        }
    }
    fn start(&mut self, mode: Mode) {
        self.clear();
        let path = match bundled_helper() {
            Ok(path) => path,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        let shared = Arc::new(Mutex::new(Shared {
            running: true,
            ..Default::default()
        }));
        let control = Arc::new(Control::default());
        let worker_shared = shared.clone();
        let worker_control = control.clone();
        std::thread::spawn(move || {
            let result = capture(path, mode, &worker_shared, &worker_control);
            if let Ok(mut state) = worker_shared.lock() {
                state.running = false;
                state.recording = false;
                state.finished = Some(result);
            };
        });
        self.session = Some(Session {
            mode,
            shared,
            control,
        });
        self.status = "Salli kameran käyttö macOS:n kysyessä ja katso kohti kameraa.".into();
    }
    fn poll(&mut self) {
        let Some(session) = &self.session else {
            return;
        };
        let shared = session.shared.clone();
        let values = shared
            .lock()
            .map(|mut state| (state.recorded.take(), state.finished.take()))
            .map_err(|_| ());
        let (recorded, finished) = match values {
            Ok(values) => values,
            Err(_) => {
                self.clear();
                self.status = "Kameran tila ei ole luettavissa".into();
                return;
            }
        };
        if let Some(result) = finished {
            self.session = None;
            match result {
                Ok(Some(angle)) => {
                    self.angle = Some(angle);
                    self.status = "Kallistus mitattu; kopioi halutessasi pään luonnokseen.".into();
                }
                Ok(None) => self.status = "Kameraseuranta päättyi.".into(),
                Err(error) => self.status = error,
            }
        }
        if let Some(result) = recorded {
            match result {
                Ok(samples) => {
                    self.draft = Some(samples);
                    self.status="Nauhoitus on luonnos. Tarkista se ja lisää animaatioon erillisellä painikkeella.".into();
                }
                Err(error) => {
                    self.draft = None;
                    self.status = error;
                }
            }
        }
    }
    fn recording(&self) -> bool {
        self.session
            .as_ref()
            .and_then(|s| s.shared.lock().ok().map(|s| s.recording))
            .unwrap_or(false)
    }
    fn begin_recording(&mut self) {
        if self.live_roll_degrees().is_none() || self.recording() {
            return;
        }
        if let Some(session) = &self.session {
            if session.mode != Mode::Stream {
                return;
            }
            if let Ok(mut state) = session.shared.lock() {
                state.recording = true;
                state.recorded = None;
            }
            session.control.record.store(true, Ordering::Release);
            self.draft = None;
            self.requested_recording = None;
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.poll();
        if let Some(session) = &self.session {
            ui.colored_label(
                egui::Color32::YELLOW,
                if self.recording() {
                    "● Kamera käytössä · NAUHOITETAAN"
                } else {
                    "● Kamera käytössä"
                },
            );
            ui.horizontal(|ui| {
                if ui.button("Pysäytä kamera").clicked() {
                    session.control.stop.store(true, Ordering::Release);
                }
                if ui.button("Peru kamera ja luonnos").clicked() {
                    session.control.cancel.store(true, Ordering::Release);
                }
            });
        }
        let cancel_requested = self
            .session
            .as_ref()
            .is_some_and(|s| s.control.cancel.load(Ordering::Acquire));
        if cancel_requested {
            self.clear();
            self.status = "Kamerakaappaus ja luonnos peruttiin.".into();
        }
        egui::CollapsingHeader::new("Webkamera · seuranta ja nauhoitus").show(ui, |ui| {
            ui.weak("Paikallinen pään 2D-kallistus. Kuvia tai ääntä ei tallenneta eikä lähetetä. Yaw/pitch ovat mittaustietoa, eivät vartalo- tai 3D-mocapia.");
            if self.session.is_none() {
                if ui.button("Kaappaa yksi pään kallistus").clicked() { self.start(Mode::Shot); }
                if ui.button("Käynnistä jatkuva seuranta").clicked() { self.start(Mode::Stream); }
            } else if self.session.as_ref().is_some_and(|s|s.mode==Mode::Stream) {
                if self.recording() {
                    if ui.button("Pysäytä nauhoitus ja tarkista").clicked() {
                        if let Some(session)=&self.session { session.control.record.store(false,Ordering::Release); }
                    }
                } else if ui.add_enabled(self.live_roll_degrees().is_some(),egui::Button::new("Nauhoita kallistus (enintään 30 s)")).clicked() { self.begin_recording(); }
                if let Some(session)=&self.session {
                    if let Ok(state)=session.shared.lock() {
                        if let Some((pose,received))=&state.latest {
                            ui.label(format!("Kallistus {:.1}° · yaw {} · pitch {}{}",pose.roll.to_degrees(),measured_angle(pose.yaw),measured_angle(pose.pitch),if received.elapsed()>FRESH {" · vanhentunut mittaus"} else {""}));
                        }
                    }
                }
            }
            ui.add_enabled_ui(!self.recording(),|ui| {ui.checkbox(&mut self.invert,"Käännä kallistuksen suunta");});
            if let Some(angle)=self.angle {
                ui.label(format!("Kaapattu kallistus: {angle:.1}°"));
                if ui.button("Kopioi kallistus luonnokseen").clicked() { self.requested=Some(if self.invert {-angle} else {angle}); }
            }
            if let Some(samples)=&self.draft {
                ui.label(format!("Luonnos: {} näytettä · {:.2} s",samples.len(),samples.last().map_or(0.0,|s|s.seconds)));
                if ui.add_enabled(!self.recording(),egui::Button::new("Lisää nauhoitus animaatioon")).clicked() {
                    self.requested_recording=Some(samples.iter().map(|s|HeadSample {seconds:s.seconds,tilt_degrees:if self.invert {-s.tilt_degrees} else {s.tilt_degrees}}).collect());
                }
                ui.weak("Vasta tämä lisäys tallentaa avainruudut. Kumoa peruu koko lisäyksen. Jos nauhoitus ylittää kohtauksen lopun, siirrä toistopää aiemmaksi tai nauhoita lyhyemmin.");
            }
            if !self.status.is_empty() {ui.label(&self.status);}
        });
        if self.session.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(25));
        }
    }
    pub fn live_roll_degrees(&self) -> Option<f64> {
        let session = self.session.as_ref().filter(|s| s.mode == Mode::Stream)?;
        let state = session.shared.lock().ok()?;
        if !state.running {
            return None;
        }
        let (pose, _) = state
            .latest
            .as_ref()
            .filter(|(_, at)| at.elapsed() <= FRESH)?;
        let angle = pose.roll.to_degrees();
        Some(if self.invert { -angle } else { angle })
    }
    pub fn take_roll_degrees(&mut self) -> Option<f64> {
        self.requested.take()
    }
    pub fn take_recording(&mut self) -> Option<Vec<HeadSample>> {
        self.requested_recording.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pose(seq: u64, time: f64, roll: f64) -> StreamPose {
        StreamPose {
            seq,
            time,
            roll,
            yaw: None,
            pitch: None,
        }
    }
    #[test]
    fn camera_protocol_is_bounded_and_angles_validated() {
        assert_eq!(measured_angle(None), "ei mittausta");
        assert_eq!(measured_angle(Some(0.0)), "0.0°");
        assert!(
            (decode_pose(br#"{"roll":0.5,"yaw":null,"pitch":0.1}"#).unwrap()
                - 0.5_f64.to_degrees())
            .abs()
                < 0.0001
        );
        for bytes in [
            br#"{"roll":3.2}"#.as_slice(),
            br#"{"roll":0,"unexpected":1}"#,
            br#"{"roll":null}"#,
            br#"{"yaw":0}"#,
        ] {
            assert!(decode_pose(bytes).is_err());
        }
        assert!(decode_pose(&vec![b' '; 4097]).is_err());
    }
    #[test]
    fn jsonl_uses_strict_sequence_and_actual_monotonic_times() {
        let mut decoder = Protocol::default();
        decoder.decode(br#"{"seq":0,"time":0.3,"roll":0}"#).unwrap();
        assert!(decoder.decode(br#"{"seq":1,"time":0.3,"roll":0}"#).is_err());
        decoder
            .decode(br#"{"seq":1,"time":0.51,"roll":0.1,"yaw":null}"#)
            .unwrap();
        assert!(decoder.decode(br#"{"seq":3,"time":0.7,"roll":0}"#).is_err());
        assert!(decoder.decode(br#"{"seq":2,"time":61,"roll":0}"#).is_err());
        assert!(Protocol::default()
            .decode(br#"{"seq":0,"time":0,"roll":0,"image":"no"}"#)
            .is_err());
    }
    #[test]
    fn bounded_reader_rejects_partial_or_oversized_jsonl_without_a_camera() {
        let (tx, rx) = mpsc::sync_channel(4);
        read_output(
            &b"{\"seq\":0,\"time\":0.2,\"roll\":0}\n"[..],
            Mode::Stream,
            &tx,
        )
        .unwrap();
        assert!(matches!(
            rx.recv().unwrap(),
            ReaderEvent::Sample(StreamPose { seq: 0, .. })
        ));
        assert!(matches!(rx.recv().unwrap(), ReaderEvent::End));
        assert!(read_output(
            &b"{\"seq\":0,\"time\":0.2,\"roll\":0}"[..],
            Mode::Stream,
            &tx
        )
        .is_err());
        assert!(read_output(&vec![b'x'; MAX_LINE + 1][..], Mode::Stream, &tx).is_err());
    }
    #[test]
    fn recording_keeps_actual_time_and_rejects_long_gaps() {
        let mut recording = Recording::default();
        recording.push(&pose(10, 12.0, 0.1)).unwrap();
        recording.push(&pose(11, 12.21, 0.2)).unwrap();
        recording.push(&pose(12, 12.58, 0.3)).unwrap();
        let samples = recording.finish().unwrap();
        assert_eq!(samples[0].seconds, 0.0);
        assert!((samples[1].seconds - 0.21).abs() < 1e-8);
        assert!((samples[2].seconds - 0.58).abs() < 1e-8);
        let mut broken = Recording::default();
        broken.push(&pose(0, 0.0, 0.0)).unwrap();
        assert!(broken.push(&pose(1, 1.01, 0.0)).is_err());
    }
    #[test]
    fn context_change_cancels_live_capture_and_discards_all_drafts() {
        let control = Arc::new(Control::default());
        let shared = Arc::new(Mutex::new(Shared {
            running: true,
            latest: Some((pose(0, 0.0, 0.2), Instant::now())),
            ..Default::default()
        }));
        let mut panel = Panel {
            session: Some(Session {
                mode: Mode::Stream,
                shared,
                control: control.clone(),
            }),
            requested: Some(5.0),
            draft: Some(vec![HeadSample {
                seconds: 0.0,
                tilt_degrees: 2.0,
            }]),
            ..Default::default()
        };
        assert!(panel.live_roll_degrees().is_some());
        panel.set_context("different source/actor/time");
        assert!(control.cancel.load(Ordering::Acquire));
        assert!(panel.live_roll_degrees().is_none());
        assert!(
            panel.take_roll_degrees().is_none()
                && panel.take_recording().is_none()
                && panel.draft.is_none()
        );
    }
    #[test]
    fn stop_is_review_only_and_apply_is_consumed_once() {
        let samples = vec![
            HeadSample {
                seconds: 0.0,
                tilt_degrees: 1.0,
            },
            HeadSample {
                seconds: 0.1,
                tilt_degrees: 3.0,
            },
        ];
        let shared = Arc::new(Mutex::new(Shared {
            recorded: Some(Ok(samples.clone())),
            finished: Some(Ok(None)),
            ..Default::default()
        }));
        let mut panel = Panel {
            session: Some(Session {
                mode: Mode::Stream,
                shared,
                control: Arc::new(Control::default()),
            }),
            ..Default::default()
        };
        panel.poll();
        assert_eq!(panel.draft, Some(samples.clone()));
        assert!(panel.take_recording().is_none());
        panel.requested_recording = Some(samples.clone());
        assert_eq!(panel.take_recording(), Some(samples));
        assert!(panel.take_recording().is_none());
        let stale = Shared {
            running: true,
            latest: Some((pose(0, 0.0, 0.0), Instant::now() - Duration::from_secs(2))),
            ..Default::default()
        };
        panel.session = Some(Session {
            mode: Mode::Stream,
            shared: Arc::new(Mutex::new(stale)),
            control: Arc::new(Control::default()),
        });
        assert!(panel.live_roll_degrees().is_none());
    }
}
