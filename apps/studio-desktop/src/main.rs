//! Native dialogue workbench. Audio assets are prepared on bounded background workers.
use eframe::egui;
use motion_audio::WaveformPeak;
use motion_lipsync::{Alignment, Viseme};
use motion_tts::{KokoroConfig, KokoroProvider, SpeechResult};
use motion_voice::{dialogue_jobs, timeline::DialogueClip, DialogueJob, VoiceUpdate, VoiceWorker};
use std::{
    path::Path,
    sync::{mpsc, Arc},
    time::Duration,
};

fn main() -> eframe::Result {
    eframe::run_native(
        "Kilsat Studio — Dialogue",
        eframe::NativeOptions::default(),
        Box::new(|_| Ok(Box::new(Studio::default()))),
    )
}

struct Preview {
    clip: DialogueClip,
    peaks: Vec<WaveformPeak>,
    result: SpeechResult,
}
struct Prepare {
    job: DialogueJob,
    result: SpeechResult,
    alignment: String,
}
type PreviewResult = Result<Preview, String>;
struct Studio {
    source: String,
    script_path: String,
    jobs: Vec<DialogueJob>,
    selected: usize,
    python: String,
    runtime: String,
    config: String,
    weights: String,
    voices: String,
    cache: String,
    revision: String,
    project_root: String,
    alignment: String,
    status: String,
    worker: Option<VoiceWorker>,
    pending_job: Option<DialogueJob>,
    preview: Option<Preview>,
    playhead: f64,
    prepare_tx: mpsc::SyncSender<Prepare>,
    prepare_rx: mpsc::Receiver<PreviewResult>,
    busy: bool,
}
impl Default for Studio {
    fn default() -> Self {
        let (prepare_tx, jobs) = mpsc::sync_channel::<Prepare>(1);
        let (results, prepare_rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            while let Ok(task) = jobs.recv() {
                let result = (|| {
                    let alignment = if task.alignment.trim().is_empty() {
                        None
                    } else {
                        Some(
                            serde_json::from_str::<Alignment>(&task.alignment)
                                .map_err(|e| e.to_string())?,
                        )
                    };
                    let clip = DialogueClip::prepare(&task.job, &task.result, alignment)
                        .map_err(|e| e.to_string())?;
                    let bin = (clip.audio.frames() / 700).max(1);
                    let peaks = motion_audio::waveform_peaks(&clip.audio, bin)
                        .map_err(|e| e.to_string())?;
                    Ok(Preview {
                        clip,
                        peaks,
                        result: task.result,
                    })
                })();
                if results.send(result).is_err() {
                    break;
                }
            }
        });
        Self {
            source: include_str!("../../../examples/dialogue.yaml").into(),
            script_path: "dialogue.yaml".into(),
            jobs: Vec::new(),
            selected: 0,
            python: "python3".into(),
            runtime: "services/kokoro-runtime/runtime.py".into(),
            config: String::new(),
            weights: String::new(),
            voices: String::new(),
            cache: "voice-cache".into(),
            revision: String::new(),
            project_root: ".".into(),
            alignment: String::new(),
            status: "Validate screenplay to select dialogue. No assets are downloaded.".into(),
            worker: None,
            pending_job: None,
            preview: None,
            playhead: 0.0,
            prepare_tx,
            prepare_rx,
            busy: false,
        }
    }
}
impl Studio {
    fn open_script(&mut self) {
        use std::io::Read;
        let result = (|| -> Result<String, String> {
            let file = std::fs::File::open(&self.script_path)
                .map_err(|e| format!("Open {}: {e}", self.script_path))?;
            let mut bytes = Vec::new();
            file.take(100_001)
                .read_to_end(&mut bytes)
                .map_err(|e| format!("Read screenplay: {e}"))?;
            if bytes.len() > 100_000 {
                return Err("Screenplay exceeds 100 KB editor limit".into());
            }
            String::from_utf8(bytes).map_err(|e| format!("Screenplay must be UTF-8: {e}"))
        })();
        match result {
            Ok(source) => {
                self.source = source;
                self.jobs.clear();
                self.preview = None;
                self.status = "Opened screenplay; validate before generation. Imported audio uses configured project root.".into();
            }
            Err(e) => self.status = e,
        }
    }
    fn save_script(&mut self) {
        if self.source.len() > 100_000 {
            self.status = "Screenplay exceeds 100 KB save limit".into();
            return;
        }
        self.status = match std::fs::write(&self.script_path, &self.source) {
            Ok(()) => format!("Saved screenplay to {}", self.script_path),
            Err(e) => format!("Save {}: {e}", self.script_path),
        };
    }
    fn validate(&mut self) {
        match motion_script::parse(&self.source)
            .map_err(|e| e.to_string())
            .and_then(|script| {
                dialogue_jobs(&script, Path::new(&self.project_root)).map_err(|e| e.to_string())
            }) {
            Ok(jobs) => {
                self.jobs = jobs;
                self.selected = self.selected.min(self.jobs.len().saturating_sub(1));
                self.preview = None;
                self.status = format!(
                    "Valid: {} dialogue events. Voice presets, speed and language come from YAML.",
                    self.jobs.len()
                );
            }
            Err(e) => {
                self.jobs.clear();
                self.preview = None;
                self.status = e;
            }
        }
    }
    fn generate(&mut self) {
        let Some(job) = self.jobs.get(self.selected).cloned() else {
            return;
        };
        let provider = KokoroProvider {
            config: KokoroConfig {
                python: self.python.clone().into(),
                runtime_script: self.runtime.clone().into(),
                model_config: self.config.clone().into(),
                model_weights: self.weights.clone().into(),
                voices_directory: self.voices.clone().into(),
                cache_directory: self.cache.clone().into(),
                model_version: self.revision.clone(),
                timeout: Duration::from_secs(180),
            },
        };
        let worker = VoiceWorker::new(Arc::new(provider), 1);
        match worker.submit(job.clone()) {
            Ok(()) => {
                self.pending_job = Some(job);
                self.worker = Some(worker);
                self.busy = true;
                self.preview = None;
                self.status =
                    "Queued. Imported WAV takes precedence when configured in YAML.".into();
            }
            Err(e) => self.status = e.to_string(),
        }
    }
    fn poll(&mut self) {
        let update = self.worker.as_ref().and_then(VoiceWorker::poll);
        match update {
            Some(VoiceUpdate::Started(_)) => {
                self.status = "Generating/importing audio on background worker…".into()
            }
            Some(VoiceUpdate::Finished { result, .. }) => {
                self.worker = None;
                match result {
                    Ok(result) => {
                        if let Some(job) = self.pending_job.take() {
                            match self.prepare_tx.try_send(Prepare {
                                job,
                                result,
                                alignment: self.alignment.clone(),
                            }) {
                                Ok(()) => self.status = "Preparing waveform and alignment…".into(),
                                Err(e) => {
                                    self.status = e.to_string();
                                    self.busy = false;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        self.status = e.to_string();
                        self.pending_job = None;
                        self.busy = false;
                    }
                }
            }
            None => {}
        }
        if let Ok(result) = self.prepare_rx.try_recv() {
            self.busy = false;
            match result {
                Ok(preview) => {
                    self.playhead = preview.clip.start_frame as f64 / 24000.0;
                    self.status = format!(
                        "Ready: {:.3} s. {}",
                        preview.result.duration_seconds(),
                        if preview.result.cached {
                            "Loaded from content cache."
                        } else {
                            "Audio asset prepared."
                        }
                    );
                    self.preview = Some(preview);
                }
                Err(e) => self.status = format!("Audio/ alignment preview: {e}"),
            }
        }
    }
}
fn field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.text_edit_singleline(value);
}
impl eframe::App for Studio {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.poll();
        if self.busy {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.heading("Kilsat Studio / Dialogue workbench");
            ui.label(
                "Local Kokoro CPU synthesis · imported audio fallback · sample-based mouth preview",
            );
            ui.label(&self.status);
            if self.busy {
                ui.spinner();
            }
        });
        egui::SidePanel::left("settings").min_width(300.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_enabled_ui(!self.busy, |ui| {
                    ui.heading("Offline runtime");
                    ui.label("Supply previously installed local assets. This app never downloads model files.");
                    field(ui, "Python executable", &mut self.python);
                    field(ui, "Sidecar script", &mut self.runtime);
                    field(ui, "Model config JSON", &mut self.config);
                    field(ui, "Model weights", &mut self.weights);
                    field(ui, "Voice .pt directory", &mut self.voices);
                    field(ui, "Cache directory", &mut self.cache);
                    field(ui, "Immutable model + dependency revision", &mut self.revision);
                    let previous_root = self.project_root.clone();
                    field(ui, "Project root (imported WAV paths)", &mut self.project_root);
                    if self.project_root != previous_root {
                        self.jobs.clear(); self.preview = None;
                        self.status = "Project root changed; validate screenplay to resolve imported paths again.".into();
                    }
                    ui.separator();
                    ui.heading("Alignment JSON");
                    ui.label("Paste manual or external forced-alignment JSON. Blank keeps mouth at rest. Timings are structurally checked, never claimed perceptually validated.");
                    ui.add(egui::TextEdit::multiline(&mut self.alignment).desired_rows(7).char_limit(100_000).code_editor());
                });
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("MotionScript");
                ui.label("Configure character voice preset / language / speed in YAML; optional speak.audio imports 24 kHz mono PCM16 WAV.");
                ui.add_enabled_ui(!self.busy, |ui| {
                    field(ui, "Screenplay path (Save replaces this file)", &mut self.script_path);
                    ui.horizontal(|ui| {
                        if ui.button("Open screenplay").clicked() { self.open_script(); }
                        if ui.button("Save screenplay").clicked() { self.save_script(); }
                    });
                    if ui.add(egui::TextEdit::multiline(&mut self.source).desired_rows(17).desired_width(f32::INFINITY).char_limit(100_000).code_editor()).changed() {
                        self.jobs.clear(); self.preview = None;
                        self.status = "Screenplay changed; validate before generation.".into();
                    }
                    if ui.button("Validate screenplay").clicked() { self.validate(); }
                    egui::ComboBox::from_label("Dialogue event").selected_text(self.jobs.get(self.selected)
                        .map(|j| format!("{} / {} @ {:.2}s", j.scene, j.actor, j.at)).unwrap_or_else(|| "Validate first".into()))
                        .show_ui(ui, |ui| {
                            for (index, job) in self.jobs.iter().enumerate() {
                                if ui.selectable_value(&mut self.selected, index, format!("{} / {}: {}", job.scene, job.actor, job.request.text)).changed() { self.preview = None; }
                            }
                        });
                    if let Some(job) = self.jobs.get_mut(self.selected) {
                        ui.add(egui::DragValue::new(&mut job.at).range(0.0..=86_400.0).speed(0.01).prefix("Preview offset (s) "));
                        ui.label("Offset changes apply to the next generation/import; edit YAML to persist them.");
                    }
                    if ui.add_enabled(!self.jobs.is_empty(), egui::Button::new("Generate / load selected dialogue")).clicked() { self.generate(); }
                });
                if let Some(preview) = &self.preview {
                    ui.separator();
                    ui.label(format!("Audio asset: {}", preview.result.audio_path.display()));
                    ui.label(format!("Duration {:.3}s · {} Hz · {} frames", preview.result.duration_seconds(), preview.result.sample_rate, preview.result.sample_count));
                    let start = preview.clip.start_frame as f64 / 24000.0;
                    let end = start + preview.clip.audio.duration_seconds();
                    ui.add(egui::Slider::new(&mut self.playhead, 0.0..=end.max(0.01)).text("Scene playhead (s)"));
                    let scene_frame = (self.playhead * 24000.0).round() as u64;
                    let mouth = preview.clip.mouth_at(scene_frame).unwrap_or(Viseme::Rest);
                    ui.label(format!("Sample frame {scene_frame} · mouth {mouth:?}"));
                    waveform(ui, &preview.peaks, (self.playhead - start) / (end - start));
                    mouth_preview(ui, mouth);
                    ui.label("Scrub-only visual preview. Audio-device playback and synchronized video export are not implemented here.");
                    if preview.clip.alignment.is_none() { ui.label("No timed alignment: mouth stays at rest. Kokoro tokens are not timestamps."); }
                }
            });
        });
    }
}
fn waveform(ui: &mut egui::Ui, peaks: &[WaveformPeak], position: f64) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 110.0),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, egui::Color32::from_gray(25));
    for (i, peak) in peaks.iter().enumerate() {
        let x = rect.left() + rect.width() * i as f32 / peaks.len().max(1) as f32;
        let y = rect.center().y;
        painter.line_segment(
            [
                egui::pos2(x, y - peak.max * 48.0),
                egui::pos2(x, y - peak.min * 48.0),
            ],
            egui::Stroke::new(1.0_f32, egui::Color32::LIGHT_BLUE),
        );
    }
    if (0.0..=1.0).contains(&position) {
        let x = rect.left() + rect.width() * position as f32;
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            egui::Stroke::new(2.0_f32, egui::Color32::YELLOW),
        );
    }
}
fn mouth_preview(ui: &mut egui::Ui, viseme: Viseme) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(170.0, 100.0), egui::Sense::hover());
    let center = rect.center();
    let (width, height) = match viseme {
        Viseme::Rest | Viseme::Closed => (45.0, 2.0),
        Viseme::A => (40.0, 28.0),
        Viseme::O | Viseme::U => (20.0, 22.0),
        Viseme::I | Viseme::E => (48.0, 11.0),
        _ => (35.0, 8.0),
    };
    ui.painter().rect_filled(
        egui::Rect::from_center_size(center, egui::vec2(width, height)),
        6.0,
        egui::Color32::from_rgb(190, 80, 100),
    );
    ui.label("Symbolic viseme display; character rig integration remains separate.");
}
