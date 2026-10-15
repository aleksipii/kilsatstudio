//! Native dialogue workbench. Audio assets are prepared on bounded background workers.
mod demo;
mod playback;
mod runtime_check;
mod scene_worker;

use eframe::egui;
use motion_audio::WaveformPeak;
use motion_lipsync::{Alignment, Viseme};
use motion_tts::{CancellationToken, KokoroConfig, KokoroProvider, SpeechResult};
use motion_voice::{dialogue_jobs, timeline::DialogueClip, DialogueJob, VoiceUpdate, VoiceWorker};
use std::{
    path::Path,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

fn main() -> eframe::Result {
    eframe::run_native(
        "Kilsat Studio — Dialogue",
        eframe::NativeOptions::default(),
        Box::new(|_| Ok(Box::new(Studio::default()))),
    )
}

struct Preview {
    scene: String,
    clip: DialogueClip,
    peaks: Vec<WaveformPeak>,
    result: SpeechResult,
    playback_audio: Arc<motion_audio::AudioBuffer>,
}
struct ScenePreview {
    scene: String,
    clips: Vec<DialogueClip>,
    audio: Arc<motion_audio::AudioBuffer>,
    peaks: Vec<WaveformPeak>,
}
struct Prepare {
    job: DialogueJob,
    result: SpeechResult,
    alignment: String,
    cancel: CancellationToken,
}
type PreviewResult = Result<Preview, String>;
struct Studio {
    demo_assets: Option<demo::DemoAssets>,
    source: String,
    script: Option<motion_script::Script>,
    scene_index: usize,
    scene_transport: Option<motion_audio::transport::AudioTransport>,
    scene_tick: Instant,
    scene_fraction: f64,
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
    active_cancel: Option<CancellationToken>,
    playback: Option<playback::Playback>,
    playback_error: Option<String>,
    scene_worker: scene_worker::Worker,
    scene_preview: Option<ScenePreview>,
    scene_alignment: String,
    scene_audio_active: bool,
    diagnostics: runtime_check::Worker,
    diagnostics_busy: bool,
    diagnostics_report: Option<Result<String, String>>,
}
impl Default for Studio {
    fn default() -> Self {
        let (prepare_tx, jobs) = mpsc::sync_channel::<Prepare>(1);
        let (results, prepare_rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            while let Ok(task) = jobs.recv() {
                let result = (|| {
                    task.cancel.check().map_err(|e| e.to_string())?;
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
                    let playback_audio = Arc::new(clip.audio.clone());
                    task.cancel.check().map_err(|e| e.to_string())?;
                    Ok(Preview {
                        scene: task.job.scene.clone(),
                        playback_audio,
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
            demo_assets: None,
            source: include_str!("../../../examples/hello-emma.yaml").into(),
            script: None,
            scene_index: 0,
            scene_transport: None,
            scene_tick: Instant::now(),
            scene_fraction: 0.0,
            script_path: std::env::temp_dir()
                .join("kilsatstudio-screenplay.yaml")
                .to_string_lossy()
                .into_owned(),
            jobs: Vec::new(),
            selected: 0,
            python: "python3".into(),
            runtime: demo::runtime_path(std::env::current_exe().ok().as_deref())
                .to_string_lossy()
                .into_owned(),
            config: String::new(),
            weights: String::new(),
            voices: String::new(),
            cache: std::env::temp_dir()
                .join("kilsatstudio-voice-cache")
                .to_string_lossy()
                .into_owned(),
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
            active_cancel: None,
            playback: None,
            playback_error: None,
            scene_worker: scene_worker::Worker::new(),
            scene_preview: None,
            scene_alignment: String::new(),
            scene_audio_active: false,
            diagnostics: runtime_check::Worker::new(),
            diagnostics_busy: false,
            diagnostics_report: None,
        }
    }
}
impl Studio {
    fn load_sound_check(&mut self) {
        self.stop_playback();
        match demo::DemoAssets::create() {
            Ok(assets) => {
                self.source = assets.source.clone();
                self.project_root = assets.root.to_string_lossy().into_owned();
                self.script_path = assets
                    .root
                    .join("sound-check.yaml")
                    .to_string_lossy()
                    .into_owned();
                self.scene_alignment = assets.alignments.clone();
                self.alignment.clear();
                self.demo_assets = Some(assets);
                self.validate();
                if self.script.is_some() {
                    self.prepare_scene_audio();
                    self.status = "Preparing synthetic sound check (tones, not Kokoro speech). Manual mouth cues are illustrative. Then use Play scene audio.".into();
                }
            }
            Err(error) => self.status = format!("Sound check: {error}"),
        }
    }
    fn invalidate_scene(&mut self) {
        self.scene_preview = None;
        self.scene_audio_active = false;
        self.script = None;
        self.scene_transport = None;
        self.playhead = 0.0;
    }
    fn reset_scene(&mut self) {
        self.stop_playback();
        self.scene_preview = None;
        self.scene_audio_active = false;
        self.playhead = 0.0;
        self.scene_fraction = 0.0;
        self.scene_tick = Instant::now();
        self.scene_transport = self
            .script
            .as_ref()
            .and_then(|s| s.scenes.get(self.scene_index))
            .and_then(|scene| {
                motion_audio::transport::AudioTransport::new(
                    24000,
                    (scene.duration * 24000.0).round() as u64,
                )
                .ok()
            });
    }
    fn stop_playback(&mut self) {
        if let Some(clock) = &mut self.scene_transport {
            clock.pause();
        }
        if let Some(player) = &self.playback {
            let _ = player.stop();
        }
    }
    fn open_script(&mut self) {
        self.stop_playback();
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
                self.invalidate_scene();
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
        self.stop_playback();
        match motion_script::parse(&self.source)
            .map_err(|e| e.to_string())
            .and_then(|script| {
                let jobs = dialogue_jobs(&script, Path::new(&self.project_root))
                    .map_err(|e| e.to_string())?;
                Ok((script, jobs))
            }) {
            Ok((script, jobs)) => {
                self.scene_index = self.scene_index.min(script.scenes.len().saturating_sub(1));
                self.script = Some(script);
                self.reset_scene();

                self.jobs = jobs;
                self.selected = self.selected.min(self.jobs.len().saturating_sub(1));
                self.preview = None;
                self.status = format!(
                    "Valid: {} dialogue events. Voice presets, speed and language come from YAML.",
                    self.jobs.len()
                );
            }
            Err(e) => {
                self.invalidate_scene();
                self.jobs.clear();
                self.preview = None;
                self.status = e;
            }
        }
    }
    fn scene_audio_ui(&mut self, ui: &mut egui::Ui) {
        if ui
            .add_enabled(
                self.script.is_some(),
                egui::Button::new("Prepare full scene audio"),
            )
            .clicked()
        {
            self.prepare_scene_audio();
        }
        let Some(preview) = &self.scene_preview else {
            return;
        };
        ui.label(format!(
            "Mixed scene {} · {} clips · {:.3}s",
            preview.scene,
            preview.clips.len(),
            preview.audio.duration_seconds()
        ));
        let audio = Arc::clone(&preview.audio);
        let duration = audio.duration_seconds();
        waveform(ui, &preview.peaks, self.playhead / duration.max(0.001));
        ui.horizontal(|ui| {
            if ui.button("Play scene audio").clicked() {
                if let Some(clock) = &mut self.scene_transport {
                    clock.pause();
                }
                self.playback_error = None;
                if self.playback.is_none() {
                    match playback::Playback::new() {
                        Ok(player) => self.playback = Some(player),
                        Err(e) => self.playback_error = Some(e),
                    }
                }
                if let Some(player) = &self.playback {
                    match player.play(audio.clone(), (self.playhead * 24000.0).round() as u64) {
                        Ok(()) => self.scene_audio_active = true,
                        Err(e) => self.playback_error = Some(e),
                    }
                }
            }
            if ui.button("Pause audio").clicked() {
                if let Some(player) = &self.playback {
                    if let Err(e) = player.pause() {
                        self.playback_error = Some(e);
                    }
                }
            }
            if ui.button("Resume audio").clicked() && self.scene_audio_active {
                if let Some(player) = &self.playback {
                    if let Err(e) = player.resume() {
                        self.playback_error = Some(e);
                    }
                }
            }
            if ui.button("Stop audio").clicked() {
                self.stop_playback();
                self.playhead = 0.0;
            }
        });
        if ui
            .add(egui::Slider::new(&mut self.playhead, 0.0..=duration).text("Mixed scene seek (s)"))
            .changed()
        {
            if let Some(clock) = &mut self.scene_transport {
                clock.pause();
            }
            if self.scene_audio_active {
                if let Some(player) = &self.playback {
                    if let Err(e) = player.seek((self.playhead * 24000.0).round() as u64) {
                        self.playback_error = Some(e);
                    }
                }
            }
        }
        ui.label("One mixed source clock drives all actor mouths. Missing alignment keeps mouths at rest. Device latency and perceptual timing require Mac validation.");
        if let Some(error) = &self.playback_error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
    }
    fn scene_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Scene preview");
        self.scene_audio_ui(ui);
        let mut selected = self.scene_index;
        if let Some(script) = &self.script {
            egui::ComboBox::from_label("Scene")
                .selected_text(
                    script
                        .scenes
                        .get(selected)
                        .map(|s| s.id.as_str())
                        .unwrap_or("No scene"),
                )
                .show_ui(ui, |ui| {
                    for (index, scene) in script.scenes.iter().enumerate() {
                        ui.selectable_value(&mut selected, index, &scene.id);
                    }
                });
        }
        if selected != self.scene_index {
            self.scene_index = selected;
            self.reset_scene();
            self.preview = None;
        }
        ui.add_enabled_ui(self.scene_transport.is_some(), |ui| {
            ui.horizontal(|ui| {
                if ui.button("Play scene (silent)").clicked() {
                    self.scene_audio_active = false;
                    self.stop_playback();
                    if let Some(t) = &mut self.scene_transport {
                        t.seek_frame((self.playhead * 24000.0).round() as u64);
                        t.play();
                    }
                    self.scene_tick = Instant::now();
                }
                if ui.button("Pause scene").clicked() {
                    self.stop_playback();
                    if let Some(t) = &mut self.scene_transport {
                        t.pause();
                    }
                }
                if ui.button("Restart scene").clicked() {
                    self.reset_scene();
                    if let Some(t) = &mut self.scene_transport {
                        t.restart();
                    }
                }
            });
            if let Some(t) = &mut self.scene_transport {
                let duration = t.duration_seconds();
                if ui
                    .add(
                        egui::Slider::new(&mut self.playhead, 0.0..=duration)
                            .text("Scene time (s)"),
                    )
                    .changed()
                {
                    t.pause();
                    let _ = t.seek_seconds(self.playhead);
                    if let Some(player) = &self.playback {
                        if self.scene_audio_active {
                            let _ = player.seek((self.playhead * 24000.0).round() as u64);
                        } else {
                            let _ = player.stop();
                        }
                    }
                    self.scene_fraction = 0.0;
                }
            }
        });
        if let Some(script) = &self.script {
            match motion_scene::evaluate(script, self.scene_index, self.playhead) {
                Ok(scene) => {
                    let frame = (self.playhead * 24000.0).round() as u64;
                    let mouths: Vec<_> = if let Some(preview) = &self.scene_preview {
                        scene_mouths(&preview.clips, frame)
                    } else {
                        self.preview
                            .as_ref()
                            .filter(|p| script.scenes[self.scene_index].id == p.scene)
                            .and_then(|p| {
                                p.clip
                                    .mouth_at(frame)
                                    .ok()
                                    .map(|v| (p.clip.actor.as_str(), v))
                            })
                            .into_iter()
                            .collect()
                    };
                    draw_scene(ui, &scene, &mouths);
                }
                Err(error) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, error.to_string());
                }
            }
        } else {
            ui.label("Validate screenplay to enable deterministic scene preview.");
        }
        ui.label("Original procedural character preview. Silent scene playback uses quantized sample time; loaded dialogue playback supplies the source clock. Device latency remains unverified.");
    }
    fn kokoro_config(&self) -> KokoroConfig {
        KokoroConfig {
            python: self.python.clone().into(),
            runtime_script: self.runtime.clone().into(),
            model_config: self.config.clone().into(),
            model_weights: self.weights.clone().into(),
            voices_directory: self.voices.clone().into(),
            cache_directory: self.cache.clone().into(),
            model_version: self.revision.clone(),
            timeout: Duration::from_secs(180),
        }
    }
    fn prepare_scene_audio(&mut self) {
        self.stop_playback();
        self.scene_audio_active = false;
        self.scene_preview = None;
        self.preview = None;
        let Some(script) = self.script.clone() else {
            return;
        };
        let cancel = CancellationToken::new();
        let request = scene_worker::Request {
            cancel: cancel.clone(),
            script,
            index: self.scene_index,
            root: self.project_root.clone().into(),
            config: self.kokoro_config(),
            alignments: self.scene_alignment.clone(),
        };
        match self.scene_worker.submit(request) {
            Ok(()) => {
                self.active_cancel = Some(cancel);
                self.busy = true;
                self.status = "Preparing full scene on background worker…".into();
            }
            Err(error) => self.status = error,
        }
    }
    fn generate(&mut self) {
        self.stop_playback();
        let Some(job) = self.jobs.get(self.selected).cloned() else {
            return;
        };
        if let Some(index) = self
            .script
            .as_ref()
            .and_then(|script| script.scenes.iter().position(|scene| scene.id == job.scene))
        {
            if index != self.scene_index {
                self.scene_index = index;
                self.reset_scene();
            }
        }
        let provider = KokoroProvider {
            config: self.kokoro_config(),
        };
        self.scene_preview = None;
        self.scene_audio_active = false;
        let worker = VoiceWorker::new(Arc::new(provider), 1);
        let cancel = CancellationToken::new();
        match worker.submit_cancellable(job.clone(), cancel.clone()) {
            Ok(()) => {
                self.active_cancel = Some(cancel);
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
    fn check_runtime(&mut self) {
        let script = motion_script::parse(&self.source);
        let mut presets = std::collections::BTreeSet::new();
        if let Ok(script) = script {
            for character in script.characters.values() {
                if let Some(voice) = &character.voice {
                    presets.insert(voice.preset.clone());
                }
            }
        }
        if presets.is_empty() {
            presets.insert("af_heart".into());
        }
        if presets
            .iter()
            .any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        {
            self.diagnostics_report = Some(Err(
                "Invalid voice preset in screenplay; validate it before checking local assets."
                    .into(),
            ));
            return;
        }
        let request = runtime_check::Request {
            python: self.python.clone(),
            doctor: Path::new(&self.runtime).with_file_name("doctor.py"),
            config: self.config.clone().into(),
            model: self.weights.clone().into(),
            voices: presets
                .into_iter()
                .map(|p| Path::new(&self.voices).join(format!("{p}.pt")))
                .collect(),
        };
        match self.diagnostics.submit(request) {
            Ok(()) => {
                self.diagnostics_busy = true;
                self.diagnostics_report = None;
            }
            Err(error) => self.diagnostics_report = Some(Err(error)),
        }
    }
    fn poll(&mut self) {
        if let Some(report) = self.diagnostics.poll() {
            self.diagnostics_busy = false;
            self.diagnostics_report = Some(report);
        }
        while let Some(update) = self.scene_worker.poll() {
            match update {
                scene_worker::Update::Progress(p) => {
                    self.status = format!(
                        "Scene audio: {}/{} · dialogue {} · {}",
                        p.completed, p.total, p.dialogue_id, p.actor
                    )
                }
                scene_worker::Update::Finished(result) => {
                    self.busy = false;
                    if self.active_cancel.take().is_some_and(|c| c.is_cancelled()) {
                        self.status = "Scene preparation cancelled. You can retry.".into();
                        self.scene_preview = None;
                        continue;
                    }
                    match result {
                        Ok(ready) => {
                            let prepared = ready.prepared;
                            if !self
                                .script
                                .as_ref()
                                .and_then(|s| s.scenes.get(self.scene_index))
                                .is_some_and(|s| s.id == prepared.scene_id)
                            {
                                self.status =
                                    "Discarded preparation for an invalidated scene.".into();
                                continue;
                            }
                            let audio = prepared.rendered.audio;
                            self.status = format!(
                                "Scene ready: {} dialogue clips · {:.3}s",
                                prepared.clips.len(),
                                audio.duration_seconds()
                            );
                            self.scene_preview = Some(ScenePreview {
                                scene: prepared.scene_id,
                                clips: prepared.clips,
                                audio: Arc::new(audio),
                                peaks: ready.peaks,
                            });
                            self.playhead = 0.0;
                        }
                        Err(error) => self.status = format!("Scene audio: {error}"),
                    }
                }
            }
        }
        let now = Instant::now();
        let elapsed = now.duration_since(self.scene_tick).as_secs_f64();
        self.scene_tick = now;
        let audio_clock = self.playback.as_ref().is_some_and(|p| p.snapshot().loaded)
            && (self.scene_audio_active
                || self.preview.as_ref().is_some_and(|p| {
                    self.script
                        .as_ref()
                        .and_then(|s| s.scenes.get(self.scene_index))
                        .is_some_and(|scene| scene.id == p.scene)
                }));
        if !audio_clock {
            if let Some(clock) = &mut self.scene_transport {
                if clock.is_playing() {
                    self.scene_fraction += elapsed * 24000.0;
                    let frames = self.scene_fraction.floor() as u64;
                    self.scene_fraction -= frames as f64;
                    clock.advance_frames(frames);
                    self.playhead = clock.position_seconds();
                }
            }
        }

        if let Some(player) = &self.playback {
            let state = player.snapshot();
            if state.loaded && audio_clock {
                if self.scene_audio_active {
                    self.playhead = state.position_frame as f64 / 24000.0;
                } else if let Some(preview) = &self.preview {
                    self.playhead = (preview
                        .clip
                        .start_frame
                        .saturating_add(state.position_frame))
                        as f64
                        / 24000.0;
                }
            }
            if let Some(error) = state.error {
                self.playback_error = Some(error);
            }
        }
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
                                cancel: self.active_cancel.clone().unwrap_or_default(),
                            }) {
                                Ok(()) => self.status = "Preparing waveform and alignment…".into(),
                                Err(e) => {
                                    self.status = e.to_string();
                                    self.busy = false;
                                    self.active_cancel = None;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        self.status = e.to_string();
                        self.pending_job = None;
                        self.busy = false;
                        self.active_cancel = None;
                    }
                }
            }
            None => {}
        }
        if let Ok(result) = self.prepare_rx.try_recv() {
            self.busy = false;
            if self.active_cancel.take().is_some_and(|c| c.is_cancelled()) {
                self.preview = None;
                self.status = "Dialogue preparation cancelled. You can retry.".into();
                return;
            }
            match result {
                Ok(preview) => {
                    let fits_scene = self
                        .script
                        .as_ref()
                        .and_then(|script| script.scenes.get(self.scene_index))
                        .is_some_and(|scene| {
                            scene.id == preview.scene
                                && preview
                                    .clip
                                    .start_frame
                                    .saturating_add(preview.clip.audio.frames() as u64)
                                    <= (scene.duration * 24000.0).round() as u64
                        });
                    if !fits_scene {
                        self.preview = None;
                        self.status = "Dialogue extends beyond the selected scene or belongs to an invalidated scene. Increase scene duration or adjust dialogue timing, then validate and generate again.".into();
                        return;
                    }
                    if self
                        .script
                        .as_ref()
                        .and_then(|s| s.scenes.get(self.scene_index))
                        .is_some_and(|scene| scene.id == preview.scene)
                    {
                        self.playhead = preview.clip.start_frame as f64 / 24000.0;
                    }
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
        if self.busy
            || self.diagnostics_busy
            || self.playback.is_some()
            || self
                .scene_transport
                .as_ref()
                .is_some_and(|t| t.is_playing())
        {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.heading("Kilsat Studio / Dialogue workbench");
            ui.label(
                "Local Kokoro CPU synthesis · imported audio fallback · sample-based mouth preview",
            );
            if ui.add_enabled(!self.busy && !self.diagnostics_busy, egui::Button::new("Load sound-check demo")).clicked() {
                self.load_sound_check();
            }
            if self.demo_assets.is_some() {
                ui.label("Sound-check assets: synthetic tones, not Kokoro speech. Mouth cues are illustrative manual poses, not measured alignment. Demo files are temporary; copy assets before saving a lasting project.");
            }
            ui.label(&self.status);
            if self.diagnostics_busy { ui.spinner(); ui.label("Checking local Kokoro setup (up to 30 seconds)…"); }
            if let Some(report) = &self.diagnostics_report {
                match report { Ok(message) => { ui.label(message); }, Err(message) => { ui.colored_label(egui::Color32::LIGHT_RED, message); } }
            }
            if self.busy {
                ui.spinner();
                if let Some(cancel) = &self.active_cancel {
                    if cancel.is_cancelled() {
                        ui.label("Cancellation requested; waiting for the worker to stop safely…");
                    } else if ui.button("Cancel generation").clicked() {
                        cancel.cancel();
                        self.status = "Cancellation requested…".into();
                    }
                }
            }
        });
        egui::SidePanel::left("settings").min_width(300.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_enabled_ui(!self.busy && !self.diagnostics_busy, |ui| {
                    let previous_runtime = format!("{:?}", self.kokoro_config());
                    ui.heading("Offline runtime");
                    ui.label("Supply previously installed local assets. This app never downloads model files.");
                    field(ui, "Python executable", &mut self.python);
                    field(ui, "Sidecar script", &mut self.runtime);
                    field(ui, "Model config JSON", &mut self.config);
                    field(ui, "Model weights", &mut self.weights);
                    field(ui, "Voice .pt directory", &mut self.voices);
                    field(ui, "Cache directory", &mut self.cache);
                    field(ui, "Immutable model + dependency revision", &mut self.revision);
                    if ui.button("Check local Kokoro setup").clicked() { self.check_runtime(); }
                    ui.label("Preflight checks local packages and files only; it does not run inference or prove compatibility.");
                    if format!("{:?}", self.kokoro_config()) != previous_runtime { self.stop_playback(); self.scene_preview = None; self.scene_audio_active = false; self.preview = None; }
                    let previous_root = self.project_root.clone();
                    field(ui, "Project root (imported WAV paths)", &mut self.project_root);
                    if self.project_root != previous_root {
                        self.stop_playback(); self.invalidate_scene(); self.jobs.clear(); self.preview = None;
                        self.status = "Project root changed; validate screenplay to resolve imported paths again.".into();
                    }
                    ui.separator();
                    ui.heading("Scene alignments JSON");
                    ui.label("Map dialogue IDs to Alignment objects, for example {\"0\": {...}}. Blank keeps all mouths at rest. Prepare again after editing.");
                    if ui.add(egui::TextEdit::multiline(&mut self.scene_alignment).desired_rows(5).char_limit(100_000).code_editor()).changed() { self.stop_playback(); self.scene_preview = None; self.scene_audio_active = false; }
                    for job in self.jobs.iter().filter(|job| self.script.as_ref().and_then(|s| s.scenes.get(self.scene_index)).is_some_and(|s| s.id == job.scene)) { ui.label(format!("ID {} · {} · {}", job.id, job.actor, job.request.text)); }
                    ui.heading("Selected-line alignment JSON");
                    ui.label("Paste manual or external forced-alignment JSON. Blank keeps mouth at rest. Timings are structurally checked, never claimed perceptually validated.");
                    ui.add(egui::TextEdit::multiline(&mut self.alignment).desired_rows(7).char_limit(100_000).code_editor());
                });
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_enabled_ui(!self.busy && !self.diagnostics_busy, |ui| self.scene_ui(ui));
                ui.separator();
                ui.heading("MotionScript");
                ui.label("Configure character voice preset / language / speed in YAML; optional speak.audio imports 24 kHz mono PCM16 WAV.");
                ui.add_enabled_ui(!self.busy && !self.diagnostics_busy, |ui| {
                    field(ui, "Screenplay path (Save replaces this file)", &mut self.script_path);
                    ui.horizontal(|ui| {
                        if ui.button("Open screenplay").clicked() { self.open_script(); }
                        if ui.button("Save screenplay").clicked() { self.save_script(); }
                    });
                    if ui.add(egui::TextEdit::multiline(&mut self.source).desired_rows(17).desired_width(f32::INFINITY).char_limit(100_000).code_editor()).changed() {
                        self.stop_playback(); self.invalidate_scene(); self.jobs.clear(); self.preview = None;
                        self.status = "Screenplay changed; validate before generation.".into();
                    }
                    if ui.button("Validate screenplay").clicked() { self.validate(); }
                    egui::ComboBox::from_label("Dialogue event").selected_text(self.jobs.get(self.selected)
                        .map(|j| format!("{} / {} @ {:.2}s", j.scene, j.actor, j.at)).unwrap_or_else(|| "Validate first".into()))
                        .show_ui(ui, |ui| {
                            for (index, job) in self.jobs.iter().enumerate() {
                                if ui.selectable_value(&mut self.selected, index, format!("{} / {}: {}", job.scene, job.actor, job.request.text)).changed() { if let Some(p) = &self.playback { let _ = p.stop(); } self.preview = None; }
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
                    if let Some(player) = &self.playback {
                        let state = player.snapshot();
                        ui.label(if state.finished { "Playback ended" } else if state.playing { "Playing" } else { "Paused / stopped" });
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Play from cursor").clicked() {
                            self.scene_audio_active = false;
                            self.playback_error = None;
                            if let Some(clock) = &mut self.scene_transport { clock.pause(); }
                            if self.playback.is_none() {
                                match playback::Playback::new() {
                                    Ok(player) => self.playback = Some(player),
                                    Err(error) => self.playback_error = Some(error),
                                }
                            }
                            if let Some(player) = &self.playback {
                                let local_frame = ((self.playhead - start).max(0.0) * 24000.0).round() as u64;
                                if let Err(error) = player.play(Arc::clone(&preview.playback_audio), local_frame) { self.playback_error = Some(error); }
                            }
                        }
                        if ui.button("Pause").clicked() { if let Some(p) = &self.playback { if let Err(e) = p.pause() { self.playback_error = Some(e); } } }
                        if ui.button("Resume").clicked() { if let Some(p) = &self.playback { if let Err(e) = p.resume() { self.playback_error = Some(e); } } }
                        if ui.button("Stop").clicked() { if let Some(p) = &self.playback { if let Err(e) = p.stop() { self.playback_error = Some(e); } } self.playhead = start; }
                    });
                    if ui.add(egui::Slider::new(&mut self.playhead, start..=end.max(start + 0.01)).text("Scene playhead (s)")).changed() {
                        if let Some(p) = &self.playback {
                            let local_frame = ((self.playhead - start).max(0.0) * 24000.0).round() as u64;
                            if let Err(error) = p.seek(local_frame) { self.playback_error = Some(error); }
                        }
                    }
                    let scene_frame = (self.playhead * 24000.0).round() as u64;
                    let mouth = preview.clip.mouth_at(scene_frame).unwrap_or(Viseme::Rest);
                    ui.label(format!("Sample frame {scene_frame} · mouth {mouth:?}"));
                    waveform(ui, &preview.peaks, (self.playhead - start) / (end - start));
                    mouth_preview(ui, mouth);
                    ui.label("macOS playback uses the source playback position for mouth preview. Device latency and perceptual sync require Mac validation. Video export is not implemented.");
                    if let Some(error) = &self.playback_error { ui.colored_label(egui::Color32::LIGHT_RED, error); }
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
    ui.label("Symbolic viseme detail; the scene preview applies this mouth to the matching actor.");
}

fn scene_mouths(clips: &[DialogueClip], frame: u64) -> Vec<(&str, Viseme)> {
    clips
        .iter()
        .filter(|clip| {
            frame >= clip.start_frame
                && frame - clip.start_frame
                    < (clip.audio.frames() as u64).saturating_sub(clip.trim_start_frame)
        })
        .filter_map(|clip| clip.mouth_at(frame).ok().map(|v| (clip.actor.as_str(), v)))
        .collect()
}

fn draw_scene(ui: &mut egui::Ui, scene: &motion_scene::EvaluatedScene, mouths: &[(&str, Viseme)]) {
    let width = ui.available_width().max(1.0);
    let height = (width * scene.height as f32 / scene.width.max(1) as f32).clamp(120.0, 420.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 5.0, egui::Color32::from_rgb(24, 31, 44));
    let scale =
        (rect.width() / scene.width.max(1) as f32).min(rect.height() / scene.height.max(1) as f32);
    let origin = rect.center() - egui::vec2(scene.width as f32, scene.height as f32) * scale * 0.5;
    for actor in &scene.actors {
        let point = |dx: f32, dy: f32| {
            origin + egui::vec2(actor.x as f32 + dx, actor.y as f32 + dy) * scale
        };
        let stroke = egui::Stroke::new(
            (5.0 * scale).max(1.0),
            egui::Color32::from_rgb(126, 185, 226),
        );
        for (a, b) in [
            ((0.0, -65.0), (0.0, -10.0)),
            ((0.0, -10.0), (-20.0, 25.0)),
            ((0.0, -10.0), (20.0, 25.0)),
            ((0.0, -50.0), (-30.0, -20.0)),
            ((0.0, -50.0), (30.0, -30.0 - actor.wave as f32 * 35.0)),
        ] {
            painter.line_segment([point(a.0, a.1), point(b.0, b.1)], stroke);
        }
        painter.circle_filled(
            point(0.0, -90.0),
            25.0 * scale,
            egui::Color32::from_rgb(245, 200, 162),
        );
        for x in [-8.0, 8.0] {
            painter.circle_filled(
                point(x, -94.0),
                (2.5 * scale).max(1.0),
                egui::Color32::BLACK,
            );
        }
        let viseme = mouths
            .iter()
            .find(|(id, _)| *id == actor.id)
            .map(|(_, v)| *v)
            .unwrap_or(Viseme::Rest);
        let (w, h) = match viseme {
            Viseme::A => (14.0, 14.0),
            Viseme::O | Viseme::U => (8.0, 11.0),
            Viseme::E | Viseme::I => (18.0, 6.0),
            Viseme::Rest | Viseme::Closed => (13.0, 2.0),
            _ => (12.0, 5.0),
        };
        painter.rect_filled(
            egui::Rect::from_center_size(point(0.0, -80.0), egui::vec2(w, h) * scale),
            2.0,
            egui::Color32::from_rgb(125, 45, 57),
        );
        if actor.emotion == "happy" && matches!(viseme, Viseme::Rest | Viseme::Closed) {
            painter.line_segment(
                [point(-8.0, -81.0), point(0.0, -76.0)],
                egui::Stroke::new((2.0 * scale).max(1.0), egui::Color32::from_rgb(125, 45, 57)),
            );
            painter.line_segment(
                [point(0.0, -76.0), point(8.0, -81.0)],
                egui::Stroke::new((2.0 * scale).max(1.0), egui::Color32::from_rgb(125, 45, 57)),
            );
        }
        if actor.emotion == "surprised" {
            for x in [-8.0, 8.0] {
                painter.line_segment(
                    [point(x - 4.0, -103.0), point(x + 4.0, -105.0)],
                    egui::Stroke::new(scale.max(1.0), egui::Color32::BLACK),
                );
            }
        }
        painter.text(
            point(0.0, 45.0),
            egui::Align2::CENTER_CENTER,
            format!("{} · {}", actor.id, actor.emotion),
            egui::FontId::proportional(12.0),
            egui::Color32::WHITE,
        );
    }
}

impl Drop for Studio {
    fn drop(&mut self) {
        if let Some(cancel) = &self.active_cancel {
            cancel.cancel();
        }
    }
}

#[cfg(test)]
mod scene_preview_tests {
    use super::*;
    fn clip(start_frame: u64, actor: &str) -> DialogueClip {
        DialogueClip {
            actor: actor.into(),
            start_frame,
            audio: motion_audio::AudioBuffer {
                sample_rate: 24000,
                channels: 1,
                samples: vec![0.0; 24000],
            },
            alignment: Some(Alignment {
                source: motion_lipsync::AlignmentSource::Manual,
                duration_seconds: 1.0,
                phonemes: vec![motion_lipsync::TimedPhoneme {
                    phoneme: "ɑ".into(),
                    start_seconds: 0.0,
                    end_seconds: 1.0,
                }],
            }),
            gain: 1.0,
            trim_start_frame: 0,
        }
    }
    #[test]
    fn later_actor_line_is_not_masked_by_earlier_rest() {
        let clips = vec![clip(0, "EMMA"), clip(48000, "EMMA"), clip(48000, "ALEX")];
        assert!(scene_mouths(&clips, 24000).is_empty());
        assert_eq!(
            scene_mouths(&clips, 48000),
            vec![("EMMA", Viseme::A), ("ALEX", Viseme::A)]
        );
        assert!(scene_mouths(&clips, 72000).is_empty());
    }
    #[test]
    fn trimmed_clip_boundaries_are_half_open() {
        let mut clips = vec![clip(24000, "EMMA")];
        clips[0].trim_start_frame = 12000;
        assert!(scene_mouths(&clips, 23999).is_empty());
        assert_eq!(scene_mouths(&clips, 35999), vec![("EMMA", Viseme::A)]);
        assert!(scene_mouths(&clips, 36000).is_empty());
    }
}
