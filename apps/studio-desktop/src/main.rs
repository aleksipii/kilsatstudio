//! Native dialogue workbench. Audio assets are prepared on bounded background workers.
mod demo;
mod edit_history;
mod event_editor;
mod event_factory;
mod export_worker;
mod playback;
mod runtime_check;
mod scene_settings;
mod scene_worker;
mod timeline_edit;
mod voice_editor;

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
        "Kilsat Studio",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1440.0, 900.0])
                .with_min_inner_size([1000.0, 700.0]),
            ..Default::default()
        },
        Box::new(|cc| {
            studio_theme(&cc.egui_ctx);
            let mut studio = Studio::default();
            if let Some(path) = std::env::args().nth(1) {
                studio.script_path = path;
                studio.open_script();
            } else {
                studio.validate();
            }
            Ok(Box::new(studio))
        }),
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
    manifest: Arc<motion_voice::export::SceneManifest>,
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
    lifecycle_id: String,
    lifecycle_asset: String,
    lifecycle_delete: Option<(bool, String, String)>,
    settings_draft: Option<scene_settings::Draft>,
    settings_draft_source: Option<String>,
    settings_error: Option<String>,
    voice_draft: Option<voice_editor::Draft>,
    voice_draft_source: Option<String>,
    voice_edit_error: Option<String>,
    inserting_event: bool,
    event_draft_source: Option<String>,
    pending_event_delete: Option<(usize, usize, String)>,
    timeline_drag: Option<timeline_edit::Drag>,
    timeline_drag_source: Option<String>,
    timeline_seek_drag: bool,
    history: edit_history::History,
    pending_text_edit: Option<String>,
    event_draft: Option<event_editor::Draft>,
    event_edit_error: Option<String>,
    exporter: export_worker::Worker,
    export_busy: bool,
    export_path: String,
    export_format: export_worker::Format,
    ffmpeg_path: String,
    export_progress: Option<motion_export::Progress>,
    selected_actor: Option<String>,
    selected_event: Option<usize>,
    editor_tab: bool,
    demo_assets: Option<demo::DemoAssets>,
    project_document: Option<motion_project::ProjectDocument>,
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
            lifecycle_id: String::new(),
            lifecycle_asset: "procedural".into(),
            lifecycle_delete: None,
            settings_draft: None,
            settings_draft_source: None,
            settings_error: None,
            voice_draft: None,
            voice_draft_source: None,
            voice_edit_error: None,
            inserting_event: false,
            event_draft_source: None,
            pending_event_delete: None,
            timeline_drag: None,
            timeline_drag_source: None,
            timeline_seek_drag: false,
            history: edit_history::History::new(include_str!("../../../examples/hello-emma.yaml")),
            pending_text_edit: None,
            event_draft: None,
            event_edit_error: None,
            selected_actor: None,
            selected_event: None,
            editor_tab: false,
            demo_assets: None,
            project_document: None,
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
            exporter: export_worker::Worker::new(),
            export_busy: false,
            export_path: String::new(),
            export_format: Default::default(),
            ffmpeg_path: String::new(),
            export_progress: None,
        }
    }
}
impl Studio {
    fn apply_structure(&mut self, result: Result<String, motion_script::Error>) {
        if self.busy || self.diagnostics_busy || self.export_busy {
            return;
        }
        match result {
            Ok(source) if source.len() <= 100_000 => {
                self.flush_text_edit();
                self.history.record(&self.source, &source);
                self.source = source;
                self.alignment.clear();
                self.scene_alignment.clear();
                self.preview = None;
                self.selected_actor = None;
                self.validate();
                self.status = "Project structure updated. Undo restores the previous source; prepare audio again.".into();
            }
            Ok(_) => self.status = "Edit exceeds the 100 KB desktop source limit.".into(),
            Err(error) => self.status = error.to_string(),
        }
    }

    fn structure_ui(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Add / duplicate / delete").show(ui, |ui| {
            ui.add_enabled_ui(self.script.is_some() && !self.busy && !self.diagnostics_busy && !self.export_busy, |ui| {
                field(ui, "New stable identifier", &mut self.lifecycle_id);
                field(ui, "New character asset reference", &mut self.lifecycle_asset);
                ui.weak("Asset references are preserved; this preview renders procedural rigs.");
                let id = self.lifecycle_id.trim().to_string();
                if ui.button("Add scene").clicked() {
                    let result = motion_script::add_scene(&self.source, motion_script::Scene { id: id.clone(), duration: 5.0, background: None, events: vec![] });
                    self.apply_structure(result);
                }
                if ui.button("Duplicate selected scene").clicked() {
                    self.apply_structure(motion_script::duplicate_scene(&self.source, self.scene_index, &id));
                }
                if ui.button("Delete selected scene…").clicked() {
                    if let Some(scene) = self.script.as_ref().and_then(|s| s.scenes.get(self.scene_index)) {
                        self.lifecycle_delete = Some((true, scene.id.clone(), self.source.clone()));
                    }
                }
                if ui.button("Add character").clicked() {
                    self.apply_structure(motion_script::add_character(&self.source, &id, motion_script::Character { asset: self.lifecycle_asset.trim().into(), default_pose: None, voice: None }));
                }
                if ui.button("Duplicate selected character").clicked() {
                    if let Some(character) = self.selected_actor.as_ref().and_then(|actor| self.script.as_ref()?.characters.get(actor)).cloned() {
                        self.apply_structure(motion_script::add_character(&self.source, &id, character));
                    }
                }
                if ui.button("Delete selected character…").clicked() {
                    if let Some(actor) = &self.selected_actor {
                        self.lifecycle_delete = Some((false, actor.clone(), self.source.clone()));
                    }
                }
                if let Some((scene, id, source)) = self.lifecycle_delete.clone() {
                    ui.colored_label(egui::Color32::LIGHT_RED, format!("Delete {} {id}?", if scene {"scene and its events"} else {"character"}));
                    ui.weak("Referenced characters cannot be deleted. Undo can restore successful changes.");
                    if ui.button("Confirm deletion").clicked() {
                        self.lifecycle_delete = None;
                        if source != self.source { self.status = "Source changed; select deletion again.".into(); }
                        else if scene {
                            if let Some(index) = self.script.as_ref().and_then(|s| s.scenes.iter().position(|s| s.id == id)) {
                                self.apply_structure(motion_script::remove_scene(&self.source, index));
                            }
                        } else { self.apply_structure(motion_script::remove_character(&self.source, &id)); }
                    }
                    if ui.button("Cancel deletion").clicked() { self.lifecycle_delete = None; }
                }
            });
        });
    }

    fn apply_settings_draft(&mut self) {
        if self.busy || self.diagnostics_busy || self.export_busy {
            return;
        }
        let (Some(draft), Some(script)) = (self.settings_draft.as_ref(), self.script.as_ref())
        else {
            return;
        };
        if draft.scene_index != self.scene_index
            || self.settings_draft_source.as_deref() != Some(self.source.as_str())
        {
            self.settings_error =
                Some("Screenplay or scene changed; reset the settings draft.".into());
            return;
        }
        let Some(scene) = script.scenes.get(draft.scene_index) else {
            return;
        };
        let unchanged = script.project.title == draft.project.title
            && script.project.fps == draft.project.fps
            && script.project.width == draft.project.width
            && script.project.height == draft.project.height
            && scene.id == draft.scene_id
            && scene.duration == draft.duration_seconds;
        if unchanged {
            self.settings_error = None;
            self.status = "Project and scene settings unchanged.".into();
            return;
        }
        let source = match motion_script::apply_scene_settings(
            &self.source,
            draft.scene_index,
            draft.project.clone(),
            &draft.scene_id,
            draft.duration_seconds,
        ) {
            Ok(source) => source,
            Err(error) => {
                self.settings_error = Some(error.to_string());
                return;
            }
        };
        if source.len() > 100_000 {
            self.settings_error =
                Some("Edited screenplay exceeds the 100 KB desktop limit.".into());
            return;
        }
        if draft.duration_seconds < scene.duration {
            let mut required_end = 0u64;
            let clips = self
                .scene_preview
                .as_ref()
                .filter(|preview| preview.scene == scene.id)
                .into_iter()
                .flat_map(|preview| preview.clips.iter())
                .chain(
                    self.preview
                        .as_ref()
                        .filter(|preview| preview.scene == scene.id)
                        .map(|preview| &preview.clip),
                );
            for clip in clips {
                let Some(end) = (clip.audio.frames() as u64)
                    .checked_sub(clip.trim_start_frame)
                    .and_then(|length| clip.start_frame.checked_add(length))
                else {
                    self.settings_error = Some("Prepared dialogue timing is invalid; reprepare audio before changing duration.".into());
                    return;
                };
                required_end = required_end.max(end);
            }
            if ((draft.duration_seconds * 24_000.0).round() as u64) < required_end {
                self.settings_error = Some(format!("Prepared dialogue ends at {:.3}s; move or shorten the line before reducing scene duration.",required_end as f64/24_000.0));
                return;
            }
        }
        self.flush_text_edit();
        self.history.record(&self.source, &source);
        self.source = source;
        self.validate();
        if let Some(script) = &self.script {
            self.settings_draft = scene_settings::Draft::new(script, self.scene_index);
            self.settings_draft_source = Some(self.source.clone());
        }
        self.status = "Project / scene settings applied. Event times stay in seconds; prepare audio again before playback or export.".into();
    }
    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        let Some(script) = &self.script else {
            return;
        };
        if !self
            .settings_draft
            .as_ref()
            .is_some_and(|draft| draft.scene_index == self.scene_index)
        {
            self.settings_draft = scene_settings::Draft::new(script, self.scene_index);
            self.settings_draft_source = Some(self.source.clone());
            self.settings_error = None;
        }
        let mut apply = false;
        let mut reset = false;
        egui::CollapsingHeader::new("Project / scene settings").show(ui, |ui| {
            ui.add_enabled_ui(!self.busy && !self.diagnostics_busy && !self.export_busy, |ui| {
                if let Some(draft) = &mut self.settings_draft { apply = draft.ui(ui); }
                reset = ui.button("Reset settings draft").clicked();
            });
            ui.weak("Apply resets pending edits and prepared audio. Speech lengths not yet generated are checked on the next preparation.");
        });
        if reset {
            self.settings_draft = self
                .script
                .as_ref()
                .and_then(|script| scene_settings::Draft::new(script, self.scene_index));
            self.settings_draft_source = Some(self.source.clone());
            self.settings_error = None;
        } else if apply {
            self.apply_settings_draft();
        }
        if let Some(error) = &self.settings_error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
    }
    fn apply_voice_draft(&mut self) {
        if self.busy || self.diagnostics_busy || self.export_busy {
            return;
        }
        let (Some(draft), Some(script)) = (self.voice_draft.as_ref(), self.script.as_ref()) else {
            return;
        };
        let actor = draft.actor.clone();
        let selected = self
            .selected_actor
            .as_deref()
            .filter(|id| script.characters.contains_key(*id))
            .or_else(|| script.characters.keys().next().map(String::as_str));
        if self.voice_draft_source.as_deref() != Some(self.source.as_str())
            || selected != Some(actor.as_str())
        {
            self.voice_edit_error =
                Some("Screenplay or character changed; reopen its voice settings.".into());
            return;
        }
        let unchanged = match (
            script.characters[&actor].voice.as_ref(),
            draft.voice.as_ref(),
        ) {
            (None, None) => true,
            (Some(old), Some(new)) => {
                old.provider == new.provider
                    && old.preset == new.preset
                    && old.language == new.language
                    && old.speed == new.speed
            }
            _ => false,
        };
        if unchanged {
            self.voice_edit_error = None;
            self.status = "Voice settings unchanged.".into();
            return;
        }
        match motion_script::apply_voice(&self.source, &actor, draft.voice.clone()) {
            Ok(source) => {
                if source.len() > 100_000 {
                    self.voice_edit_error =
                        Some("Edited screenplay exceeds the 100 KB desktop limit.".into());
                    return;
                }
                self.flush_text_edit();
                self.history.record(&self.source, &source);
                let affected: Vec<_> = self
                    .jobs
                    .iter()
                    .filter(|job| job.actor == actor && job.imported_audio.is_none())
                    .map(|job| job.id)
                    .collect();
                if self
                    .jobs
                    .get(self.selected)
                    .is_none_or(|job| job.actor == actor && job.imported_audio.is_none())
                {
                    self.alignment.clear();
                }
                if !self.scene_alignment.trim().is_empty() {
                    match serde_json::from_str::<std::collections::BTreeMap<u64, serde_json::Value>>(
                        &self.scene_alignment,
                    ) {
                        Ok(mut alignments) => {
                            for id in affected {
                                alignments.remove(&id);
                            }
                            self.scene_alignment =
                                serde_json::to_string(&alignments).unwrap_or_default();
                        }
                        Err(_) => self.scene_alignment.clear(),
                    }
                }
                self.source = source;
                self.selected_actor = Some(actor.clone());
                self.validate();
                if let Some(script) = &self.script {
                    self.voice_draft = voice_editor::Draft::new(script, &actor);
                    self.voice_draft_source = Some(self.source.clone());
                }
                self.status = "Voice applied. Reprepare scene audio; affected synthesized-speech alignment was cleared. Cache entries are retained.".into();
            }
            Err(error) => self.voice_edit_error = Some(error.to_string()),
        }
    }
    fn voice_inspector_ui(&mut self, ui: &mut egui::Ui) {
        let Some(script) = &self.script else {
            return;
        };
        let Some(actor) = self
            .selected_actor
            .as_deref()
            .filter(|id| script.characters.contains_key(*id))
            .or_else(|| script.characters.keys().next().map(String::as_str))
            .map(str::to_owned)
        else {
            return;
        };
        if !self
            .voice_draft
            .as_ref()
            .is_some_and(|draft| draft.actor == actor)
        {
            self.voice_draft = voice_editor::Draft::new(script, &actor);
            self.voice_draft_source = Some(self.source.clone());
            self.voice_edit_error = None;
        }
        let mut apply = false;
        let mut reset = false;
        egui::CollapsingHeader::new("Character voice · Kokoro").default_open(true).show(ui, |ui| {
            ui.add_enabled_ui(!self.busy && !self.diagnostics_busy && !self.export_busy, |ui| {
                if let Some(draft) = &mut self.voice_draft { apply = draft.ui(ui); }
                reset = ui.button("Reset voice draft").clicked();
            });
            ui.weak("Apply resets pending event edits and prepared scene audio. Reprepare to hear this voice.");
        });
        if reset {
            self.voice_draft = self
                .script
                .as_ref()
                .and_then(|script| voice_editor::Draft::new(script, &actor));
            self.voice_draft_source = Some(self.source.clone());
            self.voice_edit_error = None;
        } else if apply {
            self.apply_voice_draft();
        }
        if let Some(error) = &self.voice_edit_error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
    }
    fn begin_event_draft(&mut self, kind: Option<event_factory::Kind>) {
        if self.busy || self.diagnostics_busy || self.export_busy {
            return;
        }
        self.flush_text_edit();
        let Some(script) = &self.script else {
            return;
        };
        let Some(scene) = script.scenes.get(self.scene_index) else {
            return;
        };
        let actor = self
            .selected_actor
            .as_deref()
            .filter(|id| script.characters.contains_key(*id))
            .or_else(|| script.characters.keys().next().map(String::as_str));
        let result = if let Some(kind) = kind {
            match actor {
                Some(actor) => {
                    event_factory::create(script, self.scene_index, actor, self.playhead, kind)
                }
                None => Err("Add a character to MotionScript before adding events.".into()),
            }
        } else if let Some(index) = self.selected_event {
            event_factory::duplicate_at(script, self.scene_index, index, self.playhead)
        } else {
            return;
        };
        let index = scene.events.len();
        match result {
            Ok(event) => {
                self.stop_playback();
                self.cancel_timeline_drag();
                self.selected_event = None;
                self.inserting_event = true;
                self.event_draft_source = Some(self.source.clone());
                self.pending_event_delete = None;
                self.event_edit_error = None;
                self.event_draft = Some(event_editor::Draft {
                    scene_index: self.scene_index,
                    event_index: index,
                    event,
                });
                self.status =
                    "New event draft: edit its timing/properties, then choose Apply event.".into();
            }
            Err(error) => self.status = error,
        }
    }
    fn request_event_delete(&mut self) {
        if self.busy || self.diagnostics_busy || self.export_busy {
            return;
        }
        if let Some(index) = self.selected_event {
            self.pending_event_delete = Some((self.scene_index, index, self.source.clone()));
        }
    }
    fn confirm_event_delete(&mut self) {
        if self.busy || self.diagnostics_busy || self.export_busy {
            return;
        }
        let Some((scene, event, original)) = self.pending_event_delete.take() else {
            return;
        };
        if self.scene_index != scene
            || self.selected_event != Some(event)
            || self.source != original
        {
            self.status = "Event selection changed; delete cancelled.".into();
            return;
        }
        self.flush_text_edit();
        let speech = self
            .script
            .as_ref()
            .and_then(|script| script.scenes.get(scene))
            .and_then(|scene| scene.events.get(event))
            .is_some_and(|event| matches!(event, motion_script::Event::Speak { .. }));
        match motion_script::remove_event(&self.source, scene, event) {
            Ok(source) => {
                if source.len() > 100_000 {
                    self.status = "Edited screenplay exceeds 100 KB.".into();
                    return;
                }
                self.history.record(&self.source, &source);
                self.source = source;
                if speech {
                    self.alignment.clear();
                    self.scene_alignment.clear();
                }
                self.validate();
                self.status = "Event deleted. Undo restores the previous screenplay. Prepare scene audio again.".into();
            }
            Err(error) => self.status = format!("Delete event: {error}"),
        }
    }
    fn cancel_timeline_drag(&mut self) {
        self.timeline_drag = None;
        self.timeline_drag_source = None;
        self.timeline_seek_drag = false;
    }
    fn finish_timeline_drag(&mut self, drag: timeline_edit::Drag) {
        let original_source = self.timeline_drag_source.take();
        if self.busy
            || self.diagnostics_busy
            || original_source.as_deref() != Some(self.source.as_str())
        {
            return;
        }
        if drag.proposed_at == drag.original_at {
            return;
        }
        let Some(script) = &self.script else {
            return;
        };
        if self.scene_index != drag.scene_index {
            return;
        }
        let Some(event) = script
            .scenes
            .get(drag.scene_index)
            .and_then(|scene| scene.events.get(drag.event_index))
        else {
            return;
        };
        self.inserting_event = false;
        self.event_draft_source = None;
        self.pending_event_delete = None;
        self.event_draft = Some(event_editor::Draft {
            scene_index: drag.scene_index,
            event_index: drag.event_index,
            event: drag.replacement(event),
        });
        self.apply_event_draft();
        if let Some(error) = &self.event_edit_error {
            self.status = format!("Timeline edit rejected: {error}");
        }
    }
    fn flush_text_edit(&mut self) {
        if let Some(before) = self.pending_text_edit.take() {
            self.history.record(&before, &self.source);
        }
    }
    fn undo_edit(&mut self, redo: bool) {
        if self.busy || self.diagnostics_busy {
            return;
        }
        self.flush_text_edit();
        let source = if redo {
            self.history.redo(&self.source)
        } else {
            self.history.undo(&self.source)
        };
        if let Some(source) = source {
            self.source = source;
            self.alignment.clear();
            self.scene_alignment.clear();
            self.stop_playback();
            self.invalidate_scene();
            self.jobs.clear();
            self.preview = None;
            self.validate();
        }
    }
    fn apply_event_draft(&mut self) {
        if self.busy || self.diagnostics_busy || self.export_busy {
            return;
        }
        self.flush_text_edit();
        let Some(draft) = self.event_draft.as_ref() else {
            return;
        };
        let scene = draft.scene_index;
        let event = draft.event_index;
        let edits_speech = matches!(draft.event, motion_script::Event::Speak { .. });
        if self.inserting_event && self.event_draft_source.as_deref() != Some(self.source.as_str())
        {
            self.event_edit_error = Some("Screenplay changed; create a new event draft.".into());
            return;
        }
        let insertion = self.inserting_event;
        let result = if insertion {
            motion_script::insert_event(&self.source, scene, event, draft.event.clone())
        } else {
            motion_script::apply_event(&self.source, scene, event, draft.event.clone())
        };
        match result {
            Ok(source) => {
                if source.len() > 100_000 {
                    self.event_edit_error =
                        Some("Edited screenplay exceeds the 100 KB desktop save limit.".into());
                    return;
                }
                self.history.record(&self.source, &source);
                if edits_speech && insertion {
                    self.alignment.clear();
                    self.scene_alignment.clear();
                } else if edits_speech {
                    self.clear_event_alignment(scene, event);
                }
                self.source = source;
                self.scene_index = scene;
                self.validate();
                if let Some(script) = &self.script {
                    self.selected_event = Some(event);
                    self.selected_actor = script
                        .scenes
                        .get(scene)
                        .and_then(|s| s.events.get(event))
                        .map(|e| e.actor_time().1.to_owned());
                    self.event_draft = event_editor::Draft::new(script, scene, event);
                    self.status = "Event applied. Scene audio was invalidated; prepare it again before playback or export.".into();
                }
                self.event_edit_error = None;
            }
            Err(error) => {
                self.event_edit_error = Some(error.to_string());
            }
        }
    }
    fn clear_event_alignment(&mut self, scene: usize, event: usize) {
        self.alignment.clear();
        let Some(script) = &self.script else {
            self.scene_alignment.clear();
            return;
        };
        let mut dialogue_id = 0u64;
        for (scene_index, current_scene) in script.scenes.iter().enumerate() {
            for (event_index, current_event) in current_scene.events.iter().enumerate() {
                if matches!(current_event, motion_script::Event::Speak { .. }) {
                    if (scene_index, event_index) == (scene, event) {
                        if self.scene_alignment.trim().is_empty() {
                            return;
                        }
                        match serde_json::from_str::<
                            std::collections::BTreeMap<u64, serde_json::Value>,
                        >(&self.scene_alignment)
                        {
                            Ok(mut alignments) => {
                                alignments.remove(&dialogue_id);
                                self.scene_alignment =
                                    serde_json::to_string(&alignments).unwrap_or_default();
                            }
                            Err(_) => self.scene_alignment.clear(),
                        }
                        return;
                    }
                    dialogue_id += 1;
                }
            }
        }
    }
    fn load_sound_check(&mut self) {
        if self.busy || self.diagnostics_busy || self.export_busy {
            return;
        }
        self.stop_playback();
        match demo::DemoAssets::create() {
            Ok(assets) => {
                self.project_document = None;
                self.source = assets.source.clone();
                self.history.reset(&self.source);
                self.pending_text_edit = None;
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
        self.settings_draft = None;
        self.settings_draft_source = None;
        self.settings_error = None;
        self.voice_draft = None;
        self.voice_draft_source = None;
        self.voice_edit_error = None;
        self.inserting_event = false;
        self.event_draft_source = None;
        self.pending_event_delete = None;
        self.cancel_timeline_drag();
        self.event_draft = None;
        self.event_edit_error = None;
        self.selected_event = None;
        self.scene_preview = None;
        self.scene_audio_active = false;
        self.script = None;
        self.scene_transport = None;
        self.playhead = 0.0;
    }
    fn reset_scene(&mut self) {
        self.settings_draft = None;
        self.settings_draft_source = None;
        self.settings_error = None;
        self.voice_draft = None;
        self.voice_draft_source = None;
        self.voice_edit_error = None;
        self.inserting_event = false;
        self.event_draft_source = None;
        self.pending_event_delete = None;
        self.cancel_timeline_drag();
        self.event_draft = None;
        self.event_edit_error = None;
        self.selected_event = None;
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
        if self.busy || self.diagnostics_busy || self.export_busy {
            return;
        }
        if Path::new(&self.script_path)
            .extension()
            .is_some_and(|s| s == "kilsat")
        {
            let result = (|| -> Result<(motion_project::ProjectDocument, String), String> {
                let path = std::fs::canonicalize(&self.script_path).map_err(|e| e.to_string())?;
                let doc = motion_project::load(&path).map_err(|e| e.to_string())?;
                if doc.source.len() > 100_000 {
                    return Err("Project source exceeds 100 KB editor limit".into());
                }
                let root = path
                    .parent()
                    .ok_or("Project has no parent directory")?
                    .to_string_lossy()
                    .into_owned();
                Ok((doc, root))
            })();
            match result {
                Ok((doc, root)) => {
                    self.source = doc.source.clone();
                    self.project_document = Some(doc);
                    self.project_root = root;
                    self.history.reset(&self.source);
                    self.pending_text_edit = None;
                    self.alignment.clear();
                    self.scene_alignment.clear();
                    self.preview = None;
                    self.scene_index = 0;
                    self.validate();
                }
                Err(error) => self.status = format!("Open project: {error}"),
            }
            return;
        }
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
                self.project_document = None;
                self.source = source;
                self.history.reset(&self.source);
                self.pending_text_edit = None;
                self.alignment.clear();
                self.scene_alignment.clear();
                self.invalidate_scene();
                self.jobs.clear();
                self.preview = None;
                self.status = "Opened screenplay; validate before generation. Imported audio uses configured project root.".into();
            }
            Err(e) => self.status = e,
        }
    }
    fn save_script(&mut self) {
        if self.busy || self.diagnostics_busy || self.export_busy {
            return;
        }
        if Path::new(&self.script_path)
            .extension()
            .is_some_and(|s| s == "kilsat")
        {
            let result = (|| -> Result<motion_project::ProjectDocument, String> {
                if self.source.len() > 100_000 {
                    return Err("Project source exceeds 100 KB editor limit".into());
                }
                let parent = Path::new(&self.script_path)
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                let root = std::fs::canonicalize(parent).map_err(|e| e.to_string())?;
                let configured =
                    std::fs::canonicalize(&self.project_root).map_err(|e| e.to_string())?;
                if root != configured {
                    return Err("Save .kilsat beside the configured project root to preserve relative asset/audio paths. Copy assets explicitly before relocation.".into());
                }
                let mut doc = self.project_document.clone().unwrap_or(
                    motion_project::ProjectDocument::new(self.source.clone())
                        .map_err(|e| e.to_string())?,
                );
                doc.source = self.source.clone();
                motion_project::save_atomic(Path::new(&self.script_path), &doc)
                    .map_err(|e| e.to_string())?;
                Ok(doc)
            })();
            match result {
                Ok(doc) => {
                    self.project_document = Some(doc);
                    self.flush_text_edit();
                    self.status = "Saved versioned project and asset inventory.".into();
                }
                Err(error) => self.status = format!("Save project: {error}"),
            }
            return;
        }
        self.flush_text_edit();
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
        self.flush_text_edit();
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
        ui.horizontal(|ui| {
            ui.heading("Scene");
            if let Some(script) = &self.script {
                if let Some(scene) = script.scenes.get(self.scene_index) {
                    ui.weak(format!(
                        "{}  /  {} × {}  /  {} fps",
                        scene.id, script.project.width, script.project.height, script.project.fps
                    ));
                }
            }
        });
        ui.add_space(12.0);
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
        ui.add_space(8.0);
        ui.weak("Procedural preview · sample-clock mouth animation");
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
        while let Some(update) = self.exporter.poll() {
            match update {
                export_worker::Update::Progress(progress) => {
                    self.export_progress = Some(progress);
                    self.status = format!(
                        "Export: {}/{} frames",
                        progress.completed_frames, progress.total_frames
                    );
                }
                export_worker::Update::Finished(result) => {
                    self.export_busy = false;
                    self.busy = false;
                    self.active_cancel = None;
                    self.export_progress = None;
                    self.status = match result {
                        Ok(report) => format!(
                            "Exported {:?} ({} frames), scene.wav and timing manifests to {}",
                            self.export_format,
                            report.frame_count,
                            report.output_directory.display()
                        ),
                        Err(error) => format!("Scene export: {error}"),
                    };
                }
            }
        }
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
                                manifest: Arc::new(prepared.rendered.manifest),
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
impl Studio {
    fn runtime_ui(&mut self, ui: &mut egui::Ui) {
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
    }
    fn dialogue_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_enabled_ui(!self.busy && !self.diagnostics_busy, |ui| {
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
    }
    fn line_preview_ui(&mut self, ui: &mut egui::Ui) {
        if let Some(preview) = &self.preview {
            ui.separator();
            ui.label(format!(
                "Audio asset: {}",
                preview.result.audio_path.display()
            ));
            ui.label(format!(
                "Duration {:.3}s · {} Hz · {} frames",
                preview.result.duration_seconds(),
                preview.result.sample_rate,
                preview.result.sample_count
            ));
            let start = preview.clip.start_frame as f64 / 24000.0;
            let end = start + preview.clip.audio.duration_seconds();
            if let Some(player) = &self.playback {
                let state = player.snapshot();
                ui.label(if state.finished {
                    "Playback ended"
                } else if state.playing {
                    "Playing"
                } else {
                    "Paused / stopped"
                });
            }
            ui.horizontal(|ui| {
                if ui.button("Play from cursor").clicked() {
                    self.scene_audio_active = false;
                    self.playback_error = None;
                    if let Some(clock) = &mut self.scene_transport {
                        clock.pause();
                    }
                    if self.playback.is_none() {
                        match playback::Playback::new() {
                            Ok(player) => self.playback = Some(player),
                            Err(error) => self.playback_error = Some(error),
                        }
                    }
                    if let Some(player) = &self.playback {
                        let local_frame =
                            ((self.playhead - start).max(0.0) * 24000.0).round() as u64;
                        if let Err(error) =
                            player.play(Arc::clone(&preview.playback_audio), local_frame)
                        {
                            self.playback_error = Some(error);
                        }
                    }
                }
                if ui.button("Pause").clicked() {
                    if let Some(p) = &self.playback {
                        if let Err(e) = p.pause() {
                            self.playback_error = Some(e);
                        }
                    }
                }
                if ui.button("Resume").clicked() {
                    if let Some(p) = &self.playback {
                        if let Err(e) = p.resume() {
                            self.playback_error = Some(e);
                        }
                    }
                }
                if ui.button("Stop").clicked() {
                    if let Some(p) = &self.playback {
                        if let Err(e) = p.stop() {
                            self.playback_error = Some(e);
                        }
                    }
                    self.playhead = start;
                }
            });
            if ui
                .add(
                    egui::Slider::new(&mut self.playhead, start..=end.max(start + 0.01))
                        .text("Scene playhead (s)"),
                )
                .changed()
            {
                if let Some(p) = &self.playback {
                    let local_frame = ((self.playhead - start).max(0.0) * 24000.0).round() as u64;
                    if let Err(error) = p.seek(local_frame) {
                        self.playback_error = Some(error);
                    }
                }
            }
            let scene_frame = (self.playhead * 24000.0).round() as u64;
            let mouth = preview.clip.mouth_at(scene_frame).unwrap_or(Viseme::Rest);
            ui.label(format!("Sample frame {scene_frame} · mouth {mouth:?}"));
            waveform(ui, &preview.peaks, (self.playhead - start) / (end - start));
            mouth_preview(ui, mouth);
            ui.label("macOS playback uses the source playback position for mouth preview. Device latency and perceptual sync require Mac validation. MP4 export requires an explicitly selected local FFmpeg executable.");
            if let Some(error) = &self.playback_error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
            if preview.clip.alignment.is_none() {
                ui.label(
                    "No timed alignment: mouth stays at rest. Kokoro tokens are not timestamps.",
                );
            }
        }
    }
    fn workspace_ui(&mut self, ctx: &egui::Context) {
        if !self.busy && !self.diagnostics_busy && document_shortcuts_enabled(ctx) {
            let redo = ctx.input_mut(|input| {
                input.consume_shortcut(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::Z,
                ))
            });
            let undo = ctx.input_mut(|input| {
                input.consume_shortcut(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Z,
                ))
            });
            if redo || undo {
                self.undo_edit(redo);
            }
        }
        egui::TopBottomPanel::top("studio_toolbar")
            .exact_height(48.0)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(25, 29, 36))
                    .inner_margin(10.0),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("K").strong().size(20.0).color(ACCENT));
                    ui.label(egui::RichText::new("KILSAT STUDIO").strong().size(14.0));
                    ui.separator();
                    ui.add_enabled_ui(!self.busy && !self.diagnostics_busy, |ui| {
                        if ui
                            .button("Open")
                            .on_hover_text("Open the screenplay path in the MotionScript tab")
                            .clicked()
                        {
                            self.open_script();
                        }
                        if ui
                            .button("Save")
                            .on_hover_text("Replace the screenplay file at the configured path")
                            .clicked()
                        {
                            self.save_script();
                        }
                        if ui.button("Validate").clicked() {
                            self.validate();
                        }
                        let pending_change = self
                            .pending_text_edit
                            .as_ref()
                            .is_some_and(|before| before != &self.source);
                        if ui
                            .add_enabled(
                                pending_change || self.history.can_undo(),
                                egui::Button::new("Undo"),
                            )
                            .on_hover_text("Undo screenplay edit · Cmd/Ctrl+Z")
                            .clicked()
                        {
                            self.undo_edit(false);
                        }
                        if ui
                            .add_enabled(
                                !pending_change && self.history.can_redo(),
                                egui::Button::new("Redo"),
                            )
                            .on_hover_text("Redo screenplay edit · Cmd/Ctrl+Shift+Z")
                            .clicked()
                        {
                            self.undo_edit(true);
                        }
                        if ui
                            .button("Load sound-check demo")
                            .on_hover_text(
                                "Synthetic tones and illustrative mouth cues; no model download",
                            )
                            .clicked()
                        {
                            self.load_sound_check();
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.weak("OFFLINE / CPU");
                        if self.busy || self.diagnostics_busy {
                            ui.spinner();
                        }
                        if self.busy {
                            if let Some(cancel) = &self.active_cancel {
                                if cancel.is_cancelled() {
                                    ui.label("Stopping…");
                                } else if ui
                                    .button(if self.export_busy {
                                        "Cancel export"
                                    } else {
                                        "Cancel generation"
                                    })
                                    .clicked()
                                {
                                    cancel.cancel();
                                    self.status = "Cancellation requested…".into();
                                }
                            }
                        }
                    });
                });
            });
        egui::TopBottomPanel::bottom("studio_status")
            .exact_height(27.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        if self.script.is_some() {
                            ACCENT
                        } else {
                            egui::Color32::from_rgb(238, 185, 107)
                        },
                        "●",
                    );
                    ui.add(egui::Label::new(&self.status).truncate())
                        .on_hover_text(&self.status);
                });
            });
        egui::TopBottomPanel::bottom("studio_timeline")
            .resizable(true)
            .default_height(245.0)
            .min_height(150.0)
            .max_height(480.0)
            .show(ctx, |ui| {
                self.timeline_ui(ui);
            });
        egui::SidePanel::left("studio_project")
            .resizable(true)
            .default_width(280.0)
            .min_width(210.0)
            .max_width(500.0)
            .show(ctx, |ui| {
                self.project_ui(ui);
            });
        egui::SidePanel::right("studio_inspector")
            .resizable(true)
            .default_width(310.0)
            .min_width(260.0)
            .max_width(480.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("inspector_scroll")
                    .show(ui, |ui| {
                        self.inspector_ui(ui);
                    });
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(16, 19, 25))
                    .inner_margin(18.0),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("viewport_scroll")
                    .show(ui, |ui| {
                        self.scene_ui(ui);
                    });
            });
    }

    fn project_ui(&mut self, ui: &mut egui::Ui) {
        let previous_tab = self.editor_tab;
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.editor_tab, false, "Project");
            ui.selectable_value(&mut self.editor_tab, true, "MotionScript");
        });
        if previous_tab != self.editor_tab {
            self.flush_text_edit();
        }
        ui.separator();
        egui::ScrollArea::vertical().id_salt("project_scroll").show(ui, |ui| {
            if self.editor_tab {
                ui.weak("English YAML · changes require validation");
                ui.add_enabled_ui(!self.busy && !self.diagnostics_busy, |ui| {
                    field(ui, "YAML / .kilsat path · Save replaces this file", &mut self.script_path);
                    ui.add_space(6.0);
                    let before = self.source.clone();
                    let response = ui.add(egui::TextEdit::multiline(&mut self.source)
                        .id(egui::Id::new("screenplay_editor"))
                        .desired_rows(32).desired_width(f32::INFINITY).char_limit(100_000)
                        .font(egui::TextStyle::Monospace).code_editor());
                    if response.changed()
                    {
                        if self.pending_text_edit.is_none() { self.pending_text_edit = Some(before); }
                        self.alignment.clear(); self.scene_alignment.clear();
                        self.stop_playback(); self.invalidate_scene(); self.jobs.clear(); self.preview = None;
                        self.status = "Screenplay changed; validate before generation.".into();
                    }
                    if response.lost_focus() { self.flush_text_edit(); }
                });
                return;
            }
            ui.add_space(6.0);
            if let Some(script) = &self.script { ui.label(egui::RichText::new(&script.project.title).strong().size(16.0)); }
            ui.add_space(12.0);
            self.settings_ui(ui);
            self.structure_ui(ui);
            section_label(ui, "SCENES");
            let scenes: Vec<_> = self.script.as_ref().map(|s| s.scenes.iter().enumerate()
                .map(|(i, scene)| (i, scene.id.clone(), scene.duration)).collect()).unwrap_or_default();
            ui.add_enabled_ui(!self.busy && !self.diagnostics_busy, |ui| {
                for (index, id, duration) in scenes {
                    if ui.selectable_label(self.scene_index == index, format!("{id}     {duration:.2}s")).clicked()
                        && self.scene_index != index
                    {
                        self.scene_index = index; self.reset_scene(); self.preview = None;
                    }
                }
            });
            ui.add_space(16.0);
            section_label(ui, "CHARACTERS / ASSETS");
            let characters: Vec<_> = self.script.as_ref().map(|s| s.characters.iter()
                .map(|(id, c)| (id.clone(), c.asset.clone())).collect()).unwrap_or_default();
            for (id, asset) in characters {
                if ui.selectable_label(self.selected_actor.as_deref() == Some(id.as_str()), &id).clicked() {
                    self.selected_actor = Some(id); self.selected_event = None;
                }
                ui.label(egui::RichText::new(asset).small().color(egui::Color32::from_gray(140)));
                ui.add_space(5.0);
            }
            if let Some(document) = &self.project_document {
                egui::CollapsingHeader::new("Project asset inventory").show(ui, |ui| {
                    for asset in &document.assets { ui.label(format!("{} · {:?} · {}", asset.id, asset.kind, asset.path)); }
                    ui.weak("Inventory references are preserved. Image assets are not yet rendered.");
                });
            }
            if self.script.is_none() {
                ui.label("Validate MotionScript to populate the project browser.");
            }
            ui.add_space(16.0);
            ui.weak("Assets are screenplay references. This preview uses original procedural characters.");
            if self.demo_assets.is_some() {
                ui.add_space(8.0);
                ui.colored_label(egui::Color32::from_rgb(238, 185, 107), "SYNTHETIC SOUND CHECK");
                ui.weak("Tones, not Kokoro speech. Temporary assets; copy them before saving a lasting project. Mouth cues are illustrative.");
            }
        });
    }

    fn inspector_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Inspector");
        ui.weak("Select a character or timeline event");
        ui.separator();
        if let Some(script) = &self.script {
            let actor = self
                .selected_actor
                .as_deref()
                .or_else(|| script.characters.keys().next().map(String::as_str));
            if let Some((id, character)) = actor.and_then(|id| script.characters.get_key_value(id))
            {
                ui.label(egui::RichText::new(id).size(18.0).strong());
                ui.weak(format!("Asset: {}", character.asset));
                if let Some(pose) = &character.default_pose {
                    ui.label(format!("Default pose: {pose}"));
                }
            }
        }
        self.voice_inspector_ui(ui);
        self.event_inspector_ui(ui);
        ui.add_space(12.0);
        section_label(ui, "DIALOGUE");
        self.dialogue_ui(ui);
        ui.add_space(8.0);
        ui.add_enabled_ui(!self.busy && !self.diagnostics_busy, |ui| {
            if ui
                .add_enabled(
                    self.script.is_some(),
                    egui::Button::new("Prepare full scene audio"),
                )
                .clicked()
            {
                self.prepare_scene_audio();
            }
        });
        if let Some(preview) = &self.scene_preview {
            ui.colored_label(
                ACCENT,
                format!(
                    "{} clips · {:.3}s mixed",
                    preview.clips.len(),
                    preview.audio.duration_seconds()
                ),
            );
        }
        ui.add_space(10.0);
        egui::CollapsingHeader::new("Scene export · SVG / PNG / MP4")
            .default_open(true)
            .show(ui, |ui| self.export_ui(ui));
        egui::CollapsingHeader::new("Selected audio / waveform")
            .default_open(self.preview.is_some())
            .show(ui, |ui| {
                ui.add_enabled_ui(!self.busy && !self.diagnostics_busy, |ui| {
                    self.line_preview_ui(ui)
                });
            });
        ui.separator();
        egui::CollapsingHeader::new("Kokoro runtime / alignment").show(ui, |ui| {
            self.runtime_ui(ui);
        });
        if self.diagnostics_busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Checking local setup…");
            });
        }
        if let Some(report) = &self.diagnostics_report {
            egui::CollapsingHeader::new("Local setup report")
                .default_open(true)
                .show(ui, |ui| match report {
                    Ok(message) => {
                        ui.label(message);
                    }
                    Err(message) => {
                        ui.colored_label(egui::Color32::LIGHT_RED, message);
                    }
                });
        }
    }

    fn export_ui(&mut self, ui: &mut egui::Ui) {
        ui.weak(
            "Export prepared scene audio and sample-timed mouths. MP4 also retains PNG frames and WAV audio.",
        );
        ui.add_enabled_ui(!self.busy && !self.diagnostics_busy, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.export_format, export_worker::Format::Svg, "SVG");
                ui.selectable_value(&mut self.export_format, export_worker::Format::Png, "PNG");
                ui.selectable_value(&mut self.export_format, export_worker::Format::Mp4, "MP4");
            });
            if self.export_format == export_worker::Format::Mp4 {
                field(ui, "Local FFmpeg executable (absolute path)", &mut self.ffmpeg_path);
                ui.weak("Runs this local program. No encoder is downloaded. MP4 requires even dimensions.");
            }
            field(
                ui,
                "New output directory (absolute path)",
                &mut self.export_path,
            );
            if ui
                .add_enabled(
                    self.scene_preview.is_some() && !self.export_path.trim().is_empty(),
                    egui::Button::new("Export scene bundle"),
                )
                .clicked()
            {
                self.start_export();
            }
        });
        if self.scene_preview.is_none() {
            ui.weak("Prepare full scene audio first, or load the sound-check demo.");
        }
        if let Some(progress) = self.export_progress {
            ui.add(
                egui::ProgressBar::new(
                    progress.completed_frames as f32 / progress.total_frames.max(1) as f32,
                )
                .show_percentage(),
            );
            if progress.completed_frames >= progress.total_frames {
                ui.label("Frames ready; finalizing / encoding…");
            }
            ui.label(format!(
                "{} / {} frames",
                progress.completed_frames, progress.total_frames
            ));
        }
        ui.weak("The directory must not exist. Missing alignment keeps mouths at rest.");
    }

    fn event_inspector_ui(&mut self, ui: &mut egui::Ui) {
        let mut create = None;
        let mut duplicate = false;
        let mut delete = false;
        ui.add_enabled_ui(
            self.script.is_some() && !self.busy && !self.diagnostics_busy && !self.export_busy,
            |ui| {
                ui.horizontal(|ui| {
                    ui.menu_button("+ Event", |ui| {
                        for (label, kind) in [
                            ("Move", event_factory::Kind::Move),
                            ("Expression", event_factory::Kind::Emotion),
                            ("Gesture", event_factory::Kind::Gesture),
                            ("Dialogue", event_factory::Kind::Speak),
                        ] {
                            if ui.button(label).clicked() {
                                create = Some(kind);
                                ui.close_menu();
                            }
                        }
                    });
                    let existing = self.selected_event.is_some() && !self.inserting_event;
                    duplicate = ui
                        .add_enabled(existing, egui::Button::new("Duplicate"))
                        .clicked();
                    delete = ui
                        .add_enabled(existing, egui::Button::new("Delete event"))
                        .clicked();
                });
            },
        );
        if let Some(kind) = create {
            self.begin_event_draft(Some(kind));
        }
        if duplicate {
            self.begin_event_draft(None);
        }
        if delete {
            self.request_event_delete();
        }
        if self
            .pending_event_delete
            .as_ref()
            .is_some_and(|(scene, event, source)| {
                *scene != self.scene_index
                    || Some(*event) != self.selected_event
                    || source != &self.source
            })
        {
            self.pending_event_delete = None;
        }
        if self.pending_event_delete.is_some() {
            ui.label("Remove this event? Undo can restore it.");
            ui.add_enabled_ui(
                !self.busy && !self.diagnostics_busy && !self.export_busy,
                |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Confirm delete").clicked() {
                            self.confirm_event_delete();
                        }
                        if ui.button("Cancel delete").clicked() {
                            self.pending_event_delete = None;
                        }
                    });
                },
            );
        }
        if self.inserting_event && self.selected_event.is_some() {
            self.inserting_event = false;
            self.event_draft_source = None;
            self.event_draft = None;
        }
        let Some(script) = self.script.as_ref() else {
            self.event_draft = None;
            self.inserting_event = false;
            self.event_draft_source = None;
            self.event_edit_error = None;
            return;
        };
        if !self.inserting_event {
            let Some(index) = self.selected_event else {
                self.event_draft = None;
                self.event_edit_error = None;
                return;
            };
            if !self.event_draft.as_ref().is_some_and(|draft| {
                draft.scene_index == self.scene_index && draft.event_index == index
            }) {
                self.event_draft = event_editor::Draft::new(script, self.scene_index, index);
                self.event_edit_error = None;
            }
        }
        let actors: Vec<_> = script.characters.keys().cloned().collect();
        let mut apply = false;
        let mut cancel = false;
        ui.add_space(10.0);
        section_label(
            ui,
            if self.inserting_event {
                "NEW EVENT · UNSAVED DRAFT"
            } else {
                "SELECTED EVENT"
            },
        );
        ui.add_enabled_ui(
            !self.busy && !self.diagnostics_busy && !self.export_busy,
            |ui| {
                if self.inserting_event {
                    cancel = ui.button("Cancel draft").clicked();
                }
                if let Some(draft) = &mut self.event_draft {
                    apply = draft.ui(ui, &actors);
                }
            },
        );
        if cancel {
            self.inserting_event = false;
            self.event_draft_source = None;
            self.event_draft = None;
            self.event_edit_error = None;
            self.status = "Event draft cancelled; screenplay unchanged.".into();
        } else if apply {
            self.apply_event_draft();
        }
        if let Some(error) = &self.event_edit_error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
    }

    fn start_export(&mut self) {
        if self.busy || self.diagnostics_busy {
            return;
        }
        let destination = std::path::PathBuf::from(self.export_path.trim());
        if !destination.is_absolute() {
            self.status = "Export requires a new absolute output directory path.".into();
            return;
        }
        let (Some(script), Some(preview)) = (self.script.as_ref(), self.scene_preview.as_ref())
        else {
            return;
        };
        if !script
            .scenes
            .get(self.scene_index)
            .is_some_and(|scene| scene.id == preview.scene)
        {
            self.status = "Prepare audio for the selected scene before export.".into();
            return;
        }
        let cancel = CancellationToken::new();
        if self.export_format == export_worker::Format::Mp4
            && !Path::new(self.ffmpeg_path.trim()).is_absolute()
        {
            self.status = "Choose an absolute path to a local FFmpeg executable.".into();
            return;
        }
        let request = export_worker::Request {
            format: self.export_format,
            ffmpeg: self.ffmpeg_path.trim().into(),
            script: script.clone(),
            scene_index: self.scene_index,
            audio: Arc::clone(&preview.audio),
            manifest: Arc::clone(&preview.manifest),
            destination,
            cancel: cancel.clone(),
        };
        self.stop_playback();
        if let Some(clock) = &mut self.scene_transport {
            clock.pause();
        }
        match self.exporter.submit(request) {
            Ok(()) => {
                self.export_busy = true;
                self.busy = true;
                self.active_cancel = Some(cancel);
                self.export_progress = None;
                self.status = format!(
                    "Exporting {:?} and prepared scene audio…",
                    self.export_format
                );
            }
            Err(error) => self.status = error,
        }
    }

    fn seek_scene(&mut self, seconds: f64) {
        let Some(duration) = self
            .script
            .as_ref()
            .and_then(|s| s.scenes.get(self.scene_index))
            .map(|s| s.duration)
        else {
            return;
        };
        self.playhead = quantized_seek(seconds, duration);
        self.scene_fraction = 0.0;
        if let Some(clock) = &mut self.scene_transport {
            clock.pause();
            let _ = clock.seek_seconds(self.playhead);
        }
        if let Some(player) = &self.playback {
            let frame = if self.scene_audio_active {
                (self.playhead * 24000.0).round() as u64
            } else if let Some(preview) = &self.preview {
                ((self.playhead * 24000.0).round() as u64).saturating_sub(preview.clip.start_frame)
            } else {
                let _ = player.stop();
                return;
            };
            if let Err(error) = player.seek(frame) {
                self.playback_error = Some(error);
            }
        }
    }

    fn play_scene_audio(&mut self) {
        let Some(preview) = &self.scene_preview else {
            return;
        };
        let audio = Arc::clone(&preview.audio);
        if let Some(clock) = &mut self.scene_transport {
            clock.pause();
        }
        self.playback_error = None;
        if self.playback.is_none() {
            match playback::Playback::new() {
                Ok(player) => self.playback = Some(player),
                Err(error) => self.playback_error = Some(error),
            }
        }
        if let Some(player) = &self.playback {
            match player.play(audio, (self.playhead * 24000.0).round() as u64) {
                Ok(()) => self.scene_audio_active = true,
                Err(error) => self.playback_error = Some(error),
            }
        }
    }

    fn timeline_ui(&mut self, ui: &mut egui::Ui) {
        if self.busy || self.diagnostics_busy {
            self.cancel_timeline_drag();
        } else if (self.timeline_drag.is_some() || self.timeline_seek_drag)
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.cancel_timeline_drag();
            self.status = "Timeline edit cancelled.".into();
        }
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("TIMELINE").strong());
            ui.add_enabled_ui(
                self.script.is_some() && !self.busy && !self.diagnostics_busy,
                |ui| {
                    if ui.button("Play scene (silent)").clicked() {
                        self.scene_audio_active = false;
                        self.stop_playback();
                        if let Some(clock) = &mut self.scene_transport {
                            clock.seek_frame((self.playhead * 24000.0).round() as u64);
                            clock.play();
                        }
                        self.scene_tick = Instant::now();
                    }
                    if ui
                        .add_enabled(
                            self.scene_preview.is_some(),
                            egui::Button::new("Play scene audio"),
                        )
                        .clicked()
                    {
                        self.play_scene_audio();
                    }
                    if ui.button("Pause").clicked() {
                        if let Some(clock) = &mut self.scene_transport {
                            clock.pause();
                        }
                        if let Some(player) = &self.playback {
                            if let Err(error) = player.pause() {
                                self.playback_error = Some(error);
                            }
                        }
                    }
                    if ui.button("Resume").clicked() {
                        if self.scene_audio_active {
                            if let Some(player) = &self.playback {
                                if let Err(error) = player.resume() {
                                    self.playback_error = Some(error);
                                }
                            }
                        } else if let Some(clock) = &mut self.scene_transport {
                            clock.play();
                            self.scene_tick = Instant::now();
                        }
                    }
                    if ui.button("Restart").clicked() {
                        self.stop_playback();
                        self.seek_scene(0.0);
                    }
                },
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let fps = self.script.as_ref().map(|s| s.project.fps).unwrap_or(24);
                ui.monospace(format!(
                    "{:04}f  /  {:07.3}s",
                    (self.playhead * fps as f64).floor() as u64,
                    self.playhead
                ));
                for (name, color) in [
                    ("Speak", SPEAK),
                    ("Gesture", GESTURE),
                    ("Emotion", EMOTION),
                    ("Move", MOVE),
                ] {
                    ui.colored_label(color, name);
                }
            });
        });
        if let Some(error) = &self.playback_error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        let Some(script) = &self.script else {
            ui.weak("Validate a screenplay to show its actor tracks.");
            return;
        };
        let Some(scene) = script.scenes.get(self.scene_index) else {
            return;
        };
        let duration = scene.duration;
        let actor_ids: Vec<_> = script.characters.keys().cloned().collect();
        let mut events = Vec::new();
        for (index, event) in scene.events.iter().enumerate() {
            let (at, actor) = event.actor_time();
            let (lane, label, color, duration) = match event {
                motion_script::Event::Move { duration, .. } => {
                    (0usize, "Move".to_string(), MOVE, *duration)
                }
                motion_script::Event::Emotion { value, .. } => (1, value.clone(), EMOTION, 0.0),
                motion_script::Event::Gesture {
                    value, duration, ..
                } => (2, value.clone(), GESTURE, *duration),
                motion_script::Event::Speak { text, .. } => {
                    let clip_duration = self
                        .scene_preview
                        .as_ref()
                        .and_then(|p| {
                            p.clips.iter().find(|c| {
                                c.actor == actor
                                    && (c.start_frame as f64 / 24000.0 - at).abs() < 1.0 / 24000.0
                            })
                        })
                        .or_else(|| {
                            self.preview
                                .as_ref()
                                .filter(|p| {
                                    p.scene == scene.id
                                        && p.clip.actor == actor
                                        && (p.clip.start_frame as f64 / 24000.0 - at).abs()
                                            < 1.0 / 24000.0
                                })
                                .map(|p| &p.clip)
                        })
                        .map(|c| {
                            (c.audio.frames() as u64).saturating_sub(c.trim_start_frame) as f64
                                / 24000.0
                        })
                        .unwrap_or(0.0);
                    (3, text.clone(), SPEAK, clip_duration)
                }
            };
            if let Some(track) = actor_ids.iter().position(|id| id == actor) {
                events.push((
                    index,
                    track,
                    actor.to_string(),
                    at,
                    duration,
                    lane,
                    label,
                    color,
                ));
            }
        }
        let mut seek = None;
        let mut selection = None;
        let mut commit_drag = None;
        let mut pause_drag = false;
        ui.add_space(4.0);
        egui::ScrollArea::vertical().id_salt("actor_tracks").max_height((ui.available_height() - 26.0).max(60.0)).show(ui, |ui| {
            let width = ui.available_width().max(180.0);
            let label_width = 112.0;
            let track_height = 82.0;
            let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 26.0 + actor_ids.len() as f32 * track_height), egui::Sense::click_and_drag());
            let painter = ui.painter_at(rect);
            let track_left = rect.left() + label_width;
            let track_width = (rect.width() - label_width - 10.0).max(1.0);
            let x_at = |time: f64| track_left + (time / duration).clamp(0.0, 1.0) as f32 * track_width;
            painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(21, 25, 32));
            for i in 0..=10 {
                let time = duration * i as f64 / 10.0;
                let x = x_at(time);
                painter.line_segment([egui::pos2(x, rect.top() + 22.0), egui::pos2(x, rect.bottom())], egui::Stroke::new(1.0_f32, egui::Color32::from_gray(48)));
                painter.text(egui::pos2(x, rect.top() + 3.0), egui::Align2::CENTER_TOP, format!("{time:.1}s"), egui::FontId::monospace(10.0), egui::Color32::from_gray(150));
            }
            for (track, actor) in actor_ids.iter().enumerate() {
                let y = rect.top() + 26.0 + track as f32 * track_height;
                painter.text(egui::pos2(rect.left()+10.0, y+8.0), egui::Align2::LEFT_TOP, actor, egui::FontId::proportional(12.0), if self.selected_actor.as_ref() == Some(actor) { ACCENT } else { egui::Color32::from_gray(210) });
                painter.line_segment([egui::pos2(rect.left(),y+track_height), egui::pos2(rect.right(),y+track_height)], egui::Stroke::new(1.0_f32, egui::Color32::from_gray(47)));
            }
            let mut hit_boxes = Vec::new();
            for (index, track, _, at, length, lane, label, color) in &events {
                let y = rect.top()+26.0+*track as f32*track_height+*lane as f32*18.0+3.0;
                let start = x_at(*at);
                let end = x_at(*at+*length).max(start+8.0).min(track_left+track_width);
                let bar = egui::Rect::from_min_max(egui::pos2(start.min(end-2.0),y), egui::pos2(end,y+15.0));
                painter.rect_filled(bar, 3.0, color.gamma_multiply(if self.selected_event == Some(*index) { 1.0 } else { 0.65 }));
                if self.selected_event == Some(*index) { painter.rect_stroke(bar, 3.0, egui::Stroke::new(1.0_f32, egui::Color32::WHITE), egui::StrokeKind::Inside); }
                if bar.width() > 38.0 { painter.with_clip_rect(bar.shrink2(egui::vec2(4.0,0.0))).text(bar.left_center()+egui::vec2(4.0,0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(10.0), egui::Color32::WHITE); }
                hit_boxes.push((*index, bar));
            }
            let cursor = x_at(self.playhead);
            painter.line_segment([egui::pos2(cursor,rect.top()+20.0), egui::pos2(cursor,rect.bottom())], egui::Stroke::new(2.0_f32, ACCENT));
            painter.circle_filled(egui::pos2(cursor,rect.top()+20.0), 3.5, ACCENT);
            if !self.busy && !self.diagnostics_busy {
                if response.drag_started_by(egui::PointerButton::Primary) {
                    if let Some(origin) = ui.input(|input| input.pointer.press_origin()) {
                        if let Some(index) = timeline_event_at(origin, &hit_boxes) {
                            self.timeline_drag = timeline_edit::Drag::new(self.scene_index, index, &scene.events[index], origin.x);
                            self.timeline_drag_source = self.timeline_drag.as_ref().map(|_| self.source.clone());
                            self.timeline_seek_drag = false;
                            selection = Some(index);
                            pause_drag = self.timeline_drag.is_some();
                        } else {
                            self.timeline_seek_drag = origin.x >= track_left;
                        }
                    }
                }
                if response.dragged_by(egui::PointerButton::Primary) || response.drag_stopped_by(egui::PointerButton::Primary) {
                    if let Some(position) = ui.input(|input| input.pointer.latest_pos()) {
                        if let Some(drag) = &mut self.timeline_drag {
                            if let Err(error) = drag.update(position.x, track_width, duration, script.project.fps) {
                                self.status = format!("Timeline edit: {error}");
                                self.timeline_drag = None;
                                self.timeline_drag_source = None;
                            }
                        } else if self.timeline_seek_drag {
                            seek = Some(timeline_seek(position.x, track_left, track_width, duration));
                        }
                    }
                }
                if let Some(drag) = &self.timeline_drag {
                    if let Some((_,track,_,_,length,lane,_,_)) = events.iter().find(|(index,..)|*index==drag.event_index) {
                        let y=rect.top()+26.0+*track as f32*track_height+*lane as f32*18.0+3.0;
                        let start=x_at(drag.proposed_at);
                        let end=x_at(drag.proposed_at+*length).max(start+8.0).min(track_left+track_width);
                        let ghost=egui::Rect::from_min_max(egui::pos2(start.min(end-2.0),y),egui::pos2(end,y+15.0));
                        painter.rect_filled(ghost,3.0,ACCENT.gamma_multiply(0.25));
                        painter.rect_stroke(ghost,3.0,egui::Stroke::new(2.0_f32,ACCENT),egui::StrokeKind::Inside);
                        painter.text(egui::pos2(start,rect.top()+2.0),egui::Align2::LEFT_TOP,format!("{:.3}s · release to apply",drag.proposed_at),egui::FontId::monospace(11.0),ACCENT);
                    }
                }
                if response.drag_stopped_by(egui::PointerButton::Primary) {
                    commit_drag = self.timeline_drag.take();
                    self.timeline_seek_drag = false;
                }
                if response.clicked_by(egui::PointerButton::Primary) {
                    if let Some(position) = response.interact_pointer_pos() {
                        if position.x >= track_left {
                            seek = Some(timeline_seek(position.x, track_left, track_width, duration));
                            selection = timeline_event_at(position, &hit_boxes);
                        } else if position.y >= rect.top()+26.0 {
                            let track = ((position.y-rect.top()-26.0)/track_height) as usize;
                            if let Some(actor) = actor_ids.get(track) { self.selected_actor = Some(actor.clone()); self.selected_event = None; }
                        }
                    }
                }
            }
            if let Some(position) = response.hover_pos() {
                if let Some((index,_)) = hit_boxes.iter().rev().find(|(_,bar)|bar.contains(position)) {
                    if let Some((_,_,actor,at,length,_,label,_)) = events.iter().find(|(i,..)|i==index) {
                        response.on_hover_text(format!("{actor} · {label}\n{at:.3}s{}\nClick to inspect. Drag to retime; Escape cancels.", if *length > 0.0 { format!(" · {length:.3}s duration") } else { " · point marker / duration not prepared".into() }));
                    }
                }
            }
        });
        if let Some(index) = selection {
            self.selected_event = Some(index);
            if let Some((_, _, actor, at, _, _, _, _)) = events.iter().find(|(i, ..)| *i == index) {
                self.selected_actor = Some(actor.clone());
                if let Some(job_index) = self.jobs.iter().position(|j| {
                    j.scene == scene.id && j.actor == *actor && (j.at - *at).abs() < 1e-9
                }) {
                    if self.selected != job_index {
                        self.selected = job_index;
                        self.preview = None;
                    }
                }
            }
        }
        if pause_drag {
            self.stop_playback();
        }
        if let Some(drag) = commit_drag {
            self.finish_timeline_drag(drag);
        }
        if let Some(time) = seek {
            self.seek_scene(time);
        }
        egui::CollapsingHeader::new("Scene audio / mixed waveform").show(ui, |ui| {
            ui.add_enabled_ui(!self.busy && !self.diagnostics_busy, |ui| {
                self.scene_audio_ui(ui);
            });
        });
    }
}
const ACCENT: egui::Color32 = egui::Color32::from_rgb(111, 219, 178);
fn document_shortcuts_enabled(ctx: &egui::Context) -> bool {
    !ctx.wants_keyboard_input()
        || ctx.memory(|memory| memory.focused() == Some(egui::Id::new("screenplay_editor")))
}
const MOVE: egui::Color32 = egui::Color32::from_rgb(91, 158, 226);
const EMOTION: egui::Color32 = egui::Color32::from_rgb(208, 151, 93);
const GESTURE: egui::Color32 = egui::Color32::from_rgb(162, 132, 216);
const SPEAK: egui::Color32 = egui::Color32::from_rgb(71, 172, 152);

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .small()
            .strong()
            .color(egui::Color32::from_gray(150)),
    );
    ui.add_space(4.0);
}
fn studio_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(27, 31, 39);
    visuals.window_fill = egui::Color32::from_rgb(30, 35, 43);
    visuals.extreme_bg_color = egui::Color32::from_rgb(18, 22, 29);
    visuals.faint_bg_color = egui::Color32::from_rgb(33, 39, 48);
    visuals.selection.bg_fill = egui::Color32::from_rgb(39, 89, 76);
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(40, 95, 80);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(49, 62, 70);
    ctx.set_visuals(visuals);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 7.0);
    style.spacing.button_padding = egui::vec2(9.0, 5.0);
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(12.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(12.0));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(17.0));
    ctx.set_style(style);
}

/// All timeline gestures quantize to the same 24 kHz source clock as playback.
fn quantized_seek(seconds: f64, duration: f64) -> f64 {
    if !seconds.is_finite() || !duration.is_finite() || duration <= 0.0 {
        return 0.0;
    }
    ((seconds.clamp(0.0, duration) * 24000.0).round() / 24000.0).min(duration)
}
fn timeline_event_at(position: egui::Pos2, boxes: &[(usize, egui::Rect)]) -> Option<usize> {
    boxes
        .iter()
        .rev()
        .find(|(_, rect)| rect.contains(position))
        .map(|(index, _)| *index)
}
fn timeline_seek(pointer_x: f32, left: f32, width: f32, duration: f64) -> f64 {
    if !pointer_x.is_finite() || !left.is_finite() || !width.is_finite() || width <= 0.0 {
        return 0.0;
    }
    quantized_seek(
        ((pointer_x - left) / width).clamp(0.0, 1.0) as f64 * duration,
        duration,
    )
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
        self.workspace_ui(ctx);
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
    let height = (width * scene.height as f32 / scene.width.max(1) as f32)
        .clamp(120.0, (ui.available_height() - 35.0).max(120.0));
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
        use motion_scene::{CHEST, HEAD, HIP, LEFT_FOOT, LEFT_HAND, RIGHT_FOOT, RIGHT_HAND};
        for (a, b) in [
            (CHEST, HIP),
            (CHEST, HEAD),
            (HIP, LEFT_FOOT),
            (HIP, RIGHT_FOOT),
            (CHEST, LEFT_HAND),
            (CHEST, RIGHT_HAND),
        ] {
            let a = actor.skeleton[a];
            let b = actor.skeleton[b];
            painter.line_segment(
                [point(a.x as f32, a.y as f32), point(b.x as f32, b.y as f32)],
                stroke,
            );
        }
        let head = actor.skeleton[HEAD];
        let face = |dx: f32, dy: f32| point(head.x as f32 + dx, head.y as f32 + dy);
        painter.circle_filled(
            face(0.0, 0.0),
            25.0 * scale,
            egui::Color32::from_rgb(245, 200, 162),
        );
        for x in [-8.0, 8.0] {
            painter.circle_filled(face(x, -4.0), (2.5 * scale).max(1.0), egui::Color32::BLACK);
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
            egui::Rect::from_center_size(face(0.0, 10.0), egui::vec2(w, h) * scale),
            2.0,
            egui::Color32::from_rgb(125, 45, 57),
        );
        if actor.emotion == "happy" && matches!(viseme, Viseme::Rest | Viseme::Closed) {
            painter.line_segment(
                [face(-8.0, 9.0), face(0.0, 14.0)],
                egui::Stroke::new((2.0 * scale).max(1.0), egui::Color32::from_rgb(125, 45, 57)),
            );
            painter.line_segment(
                [face(0.0, 14.0), face(8.0, 9.0)],
                egui::Stroke::new((2.0 * scale).max(1.0), egui::Color32::from_rgb(125, 45, 57)),
            );
        }
        if actor.emotion == "surprised" {
            for x in [-8.0, 8.0] {
                painter.line_segment(
                    [face(x - 4.0, -13.0), face(x + 4.0, -15.0)],
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
    fn structure_transactions_undo_and_reject_dangling_character() {
        let mut studio = Studio::default();
        studio.validate();
        let original = studio.source.clone();
        studio.apply_structure(motion_script::duplicate_scene(&studio.source, 0, "copy"));
        assert_eq!(studio.script.as_ref().unwrap().scenes.len(), 2);
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        let actor = studio
            .script
            .as_ref()
            .unwrap()
            .characters
            .keys()
            .next()
            .unwrap()
            .clone();
        studio.apply_structure(motion_script::remove_character(&studio.source, &actor));
        assert_eq!(studio.source, original);
        assert!(studio
            .script
            .as_ref()
            .unwrap()
            .characters
            .contains_key(&actor));
        studio.busy = true;
        studio.apply_structure(motion_script::duplicate_scene(&studio.source, 0, "blocked"));
        assert_eq!(studio.source, original);
    }

    #[test]
    fn versioned_project_preserves_inventory_source_and_blocks_relocation() {
        let mut studio = Studio::default();
        let root = std::env::temp_dir().join(format!(
            "kilsat-desktop-project-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("studio.kilsat");
        let mut doc = motion_project::ProjectDocument::new(studio.source.clone()).unwrap();
        doc.assets.push(motion_project::AssetReference {
            id: "sound".into(),
            path: "sound.wav".into(),
            kind: motion_project::AssetKind::Audio,
        });
        motion_project::save_atomic(&path, &doc).unwrap();
        studio.script_path = path.to_string_lossy().into_owned();
        studio.open_script();
        assert_eq!(studio.source, doc.source);
        assert_eq!(
            Path::new(&studio.project_root),
            std::fs::canonicalize(&root).unwrap()
        );
        studio.source.push_str("\n# retained exact source\n");
        studio.save_script();
        let saved = motion_project::load(&path).unwrap();
        assert_eq!(saved.source, studio.source);
        assert_eq!(saved.assets, doc.assets);
        let other = root.join("other");
        std::fs::create_dir(&other).unwrap();
        studio.script_path = other.join("moved.kilsat").to_string_lossy().into_owned();
        studio.save_script();
        assert!(!Path::new(&studio.script_path).exists());
        assert!(studio.status.contains("relative asset/audio"));
        studio.busy = true;
        studio.script_path = path.to_string_lossy().into_owned();
        studio.open_script();
        assert_eq!(studio.source, saved.source);
        std::fs::remove_dir_all(root).unwrap();
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
    #[test]
    fn timeline_seeking_clamps_and_uses_audio_sample_clock() {
        assert_eq!(timeline_seek(-50.0, 100.0, 200.0, 8.0), 0.0);
        assert_eq!(timeline_seek(500.0, 100.0, 200.0, 8.0), 8.0);
        assert_eq!(timeline_seek(200.0, 100.0, 200.0, 8.0), 4.0);
        let seek = timeline_seek(123.45, 100.0, 200.0, 8.0);
        assert!((seek * 24000.0 - (seek * 24000.0).round()).abs() < 1e-8);
        assert_eq!(timeline_seek(f32::NAN, 0.0, 1.0, 8.0), 0.0);
        assert_eq!(timeline_seek(1.0, 0.0, 0.0, 8.0), 0.0);
        assert_eq!(quantized_seek(f64::NAN, 8.0), 0.0);
    }
    #[test]
    fn timeline_selection_matches_topmost_bar_and_ignores_other_lanes() {
        let bar = egui::Rect::from_min_max(egui::pos2(100.0, 10.0), egui::pos2(200.0, 25.0));
        let boxes = [(3, bar), (5, bar.translate(egui::vec2(20.0, 0.0)))];
        assert_eq!(timeline_event_at(egui::pos2(110.0, 15.0), &boxes), Some(3));
        assert_eq!(timeline_event_at(egui::pos2(150.0, 15.0), &boxes), Some(5));
        assert_eq!(timeline_event_at(egui::pos2(150.0, 35.0), &boxes), None);
    }
    #[test]
    fn prepared_demo_exports_off_thread_and_rejects_stale_scene() {
        let mut studio = Studio::default();
        studio.load_sound_check();
        let deadline = Instant::now() + Duration::from_secs(5);
        while studio.busy && Instant::now() < deadline {
            studio.poll();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!studio.busy, "{}", studio.status);
        let root = studio.demo_assets.as_ref().unwrap().root.clone();
        studio.export_path = root.join("exported-scene").to_string_lossy().into_owned();
        let preview = studio
            .scene_preview
            .as_mut()
            .expect("model-free scene prepared");
        let scene = preview.scene.clone();
        preview.scene = "stale-scene".into();
        studio.start_export();
        assert!(!studio.export_busy);
        assert!(!Path::new(&studio.export_path).exists());
        studio.scene_preview.as_mut().unwrap().scene = scene;
        studio.start_export();
        assert!(studio.export_busy && studio.busy);
        let deadline = Instant::now() + Duration::from_secs(5);
        while studio.busy && Instant::now() < deadline {
            studio.poll();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!studio.busy, "{}", studio.status);
        assert!(
            Path::new(&studio.export_path)
                .join("manifest.json")
                .is_file(),
            "{}",
            studio.status
        );
        assert!(Path::new(&studio.export_path).join("scene.wav").is_file());
        studio.invalidate_scene();
        assert!(studio.scene_preview.is_none());
        studio.export_path = root
            .join("after-invalidation")
            .to_string_lossy()
            .into_owned();
        studio.start_export();
        assert!(!studio.export_busy);
        assert!(!Path::new(&studio.export_path).exists());
    }
    #[test]
    fn inspector_edits_validate_atomically_and_undo_restores_exact_source() {
        let mut studio = Studio::default();
        studio.validate();
        let before = studio.source.clone();
        let index = studio.script.as_ref().unwrap().scenes[0]
            .events
            .iter()
            .position(|event| matches!(event, motion_script::Event::Move { .. }))
            .unwrap();
        studio.selected_event = Some(index);
        studio.event_draft = event_editor::Draft::new(studio.script.as_ref().unwrap(), 0, index);
        if let motion_script::Event::Move { duration, .. } =
            &mut studio.event_draft.as_mut().unwrap().event
        {
            *duration = -1.0;
        }
        studio.apply_event_draft();
        assert_eq!(studio.source, before);
        assert!(studio.event_edit_error.is_some());
        assert!(!studio.history.can_undo());
        studio.event_draft = event_editor::Draft::new(studio.script.as_ref().unwrap(), 0, index);
        if let motion_script::Event::Move { x, .. } =
            &mut studio.event_draft.as_mut().unwrap().event
        {
            *x = 321.0;
        }
        studio.apply_event_draft();
        let after = studio.source.clone();
        assert_ne!(after, before);
        assert_eq!(studio.selected_event, Some(index));
        assert!(matches!(
            studio.script.as_ref().unwrap().scenes[0].events[index],
            motion_script::Event::Move { x: 321.0, .. }
        ));
        studio.undo_edit(false);
        assert_eq!(studio.source, before);
        studio.undo_edit(true);
        assert_eq!(studio.source, after);
        studio.busy = true;
        studio.undo_edit(false);
        assert_eq!(studio.source, after);
    }
    #[test]
    fn edited_dialogue_drops_only_its_stale_scene_alignment() {
        let mut studio = Studio::default();
        studio.validate();
        studio.scene_alignment = r#"{"0":{"fixture":"edited"},"1":{"fixture":"keep"}}"#.into();
        studio.alignment = "selected line alignment".into();
        let index = studio.script.as_ref().unwrap().scenes[0]
            .events
            .iter()
            .position(|event| matches!(event, motion_script::Event::Speak { .. }))
            .unwrap();
        studio.event_draft = event_editor::Draft::new(studio.script.as_ref().unwrap(), 0, index);
        if let motion_script::Event::Speak { text, .. } =
            &mut studio.event_draft.as_mut().unwrap().event
        {
            text.push_str(" Changed.");
        }
        studio.apply_event_draft();
        let alignments: serde_json::Value = serde_json::from_str(&studio.scene_alignment).unwrap();
        assert!(alignments.get("0").is_none());
        assert_eq!(alignments["1"]["fixture"], "keep");
        assert!(studio.alignment.is_empty());
        studio.undo_edit(false);
        assert!(studio.scene_alignment.is_empty());
    }
    #[test]
    fn focused_screenplay_changes_form_one_history_transaction() {
        let mut studio = Studio::default();
        let before = studio.source.clone();
        studio.pending_text_edit = Some(before.clone());
        studio.source = format!("# typed comment\n{before}");
        let edited = studio.source.clone();
        studio.undo_edit(false);
        assert_eq!(studio.source, before);
        studio.undo_edit(true);
        assert_eq!(studio.source, edited);
        studio.pending_text_edit = Some(edited.clone());
        studio.flush_text_edit();
        studio.undo_edit(false);
        assert_eq!(studio.source, before);
    }
    #[test]
    fn document_shortcuts_leave_other_text_fields_their_own_undo() {
        for (field, enabled) in [
            ("inspector_dialogue", false),
            ("export_path", false),
            ("screenplay_editor", true),
        ] {
            let ctx = egui::Context::default();
            let mut text = String::from("draft");
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut text).id(egui::Id::new(field)))
                        .request_focus();
                    assert_eq!(document_shortcuts_enabled(ctx), enabled, "{field}");
                });
            });
        }
    }
    #[test]
    fn timeline_retime_commits_once_and_undo_restores_source() {
        let mut studio = Studio::default();
        studio.validate();
        let original = studio.source.clone();
        let event = &studio.script.as_ref().unwrap().scenes[0].events[0];
        let mut drag = timeline_edit::Drag::new(0, 0, event, 0.0).unwrap();
        drag.update(200.0, 1200.0, 12.0, 24).unwrap();
        studio.timeline_drag_source = Some(original.clone());
        studio.finish_timeline_drag(drag);
        let edited = studio.source.clone();
        assert_ne!(edited, original);
        assert_eq!(
            studio.script.as_ref().unwrap().scenes[0].events[0]
                .actor_time()
                .0,
            2.0
        );
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        assert!(!studio.history.can_undo());
        studio.undo_edit(true);
        assert_eq!(studio.source, edited);
    }
    #[test]
    fn timeline_retime_rejects_overlap_stale_source_and_noop() {
        let mut studio = Studio::default();
        studio.validate();
        let original = studio.source.clone();
        let event = studio.script.as_ref().unwrap().scenes[0].events[0].clone();
        let no_op = timeline_edit::Drag::new(0, 0, &event, 0.0).unwrap();
        studio.timeline_drag_source = Some(original.clone());
        studio.finish_timeline_drag(no_op);
        assert_eq!(studio.source, original);
        assert!(!studio.history.can_undo());
        let mut overlapping = timeline_edit::Drag::new(0, 0, &event, 0.0).unwrap();
        overlapping.update(800.0, 1200.0, 12.0, 24).unwrap();
        studio.timeline_drag_source = Some(original.clone());
        studio.finish_timeline_drag(overlapping);
        assert_eq!(studio.source, original);
        assert!(studio.status.contains("Timeline edit rejected"));
        assert!(!studio.history.can_undo());
        let mut stale = timeline_edit::Drag::new(0, 0, &event, 0.0).unwrap();
        stale.update(100.0, 1200.0, 12.0, 24).unwrap();
        studio.timeline_drag_source = Some(original.clone());
        studio.source.push_str("\n# external edit\n");
        let current = studio.source.clone();
        studio.finish_timeline_drag(stale);
        assert_eq!(studio.source, current);
        assert!(!studio.history.can_undo());
        studio.timeline_drag = Some(timeline_edit::Drag::new(0, 0, &event, 0.0).unwrap());
        studio.reset_scene();
        assert!(studio.timeline_drag.is_none());
        assert!(studio.timeline_drag_source.is_none());
    }
    #[test]
    fn new_event_is_a_validated_draft_with_one_undo() {
        let mut studio = Studio::default();
        studio.validate();
        let before = studio.source.clone();
        let count = studio.script.as_ref().unwrap().scenes[0].events.len();
        studio.scene_alignment = r#"{"0":{"fixture":"keep"}}"#.into();
        studio.playhead = 4.0;
        studio.begin_event_draft(Some(event_factory::Kind::Emotion));
        assert_eq!(studio.source, before);
        assert!(!studio.history.can_undo());
        assert!(studio.inserting_event);
        if let motion_script::Event::Emotion { actor, .. } =
            &mut studio.event_draft.as_mut().unwrap().event
        {
            *actor = "MISSING".into();
        }
        studio.apply_event_draft();
        assert_eq!(studio.source, before);
        assert!(studio.event_edit_error.is_some());
        if let motion_script::Event::Emotion { actor, .. } =
            &mut studio.event_draft.as_mut().unwrap().event
        {
            *actor = "EMMA".into();
        }
        studio.apply_event_draft();
        assert_eq!(
            studio.script.as_ref().unwrap().scenes[0].events.len(),
            count + 1
        );
        assert!(studio.scene_alignment.contains("keep"));
        assert_eq!(studio.selected_event, Some(count));
        assert!(!studio.inserting_event);
        studio.undo_edit(false);
        assert_eq!(studio.source, before);
        assert!(!studio.history.can_undo());
    }
    #[test]
    fn duplicate_speech_and_confirmed_delete_invalidate_shifted_alignment_ids() {
        let mut studio = Studio::default();
        studio.validate();
        let count = studio.script.as_ref().unwrap().scenes[0].events.len();
        let index = studio.script.as_ref().unwrap().scenes[0]
            .events
            .iter()
            .position(|event| matches!(event, motion_script::Event::Speak { .. }))
            .unwrap();
        studio.selected_event = Some(index);
        studio.playhead = 10.0;
        studio.scene_alignment = r#"{"0":{"fixture":"stale"},"1":{"fixture":"shifted"}}"#.into();
        studio.alignment = "stale".into();
        studio.begin_event_draft(None);
        assert!(matches!(
            studio.event_draft.as_ref().unwrap().event,
            motion_script::Event::Speak { .. }
        ));
        studio.apply_event_draft();
        assert_eq!(
            studio.script.as_ref().unwrap().scenes[0].events.len(),
            count + 1
        );
        assert!(studio.scene_alignment.is_empty() && studio.alignment.is_empty());
        let duplicated = studio.source.clone();
        studio.request_event_delete();
        assert_eq!(studio.source, duplicated);
        studio.export_busy = true;
        studio.confirm_event_delete();
        assert_eq!(studio.source, duplicated);
        studio.export_busy = false;
        studio.confirm_event_delete();
        assert_eq!(
            studio.script.as_ref().unwrap().scenes[0].events.len(),
            count
        );
        assert!(studio.selected_event.is_none());
        studio.undo_edit(false);
        assert_eq!(studio.source, duplicated);
    }
    #[test]
    fn stale_lifecycle_actions_and_cancelled_drafts_do_not_change_source() {
        let mut studio = Studio::default();
        studio.validate();
        let before = studio.source.clone();
        studio.begin_event_draft(Some(event_factory::Kind::Gesture));
        studio.reset_scene();
        assert!(!studio.inserting_event && studio.event_draft.is_none());
        assert_eq!(studio.source, before);
        assert!(!studio.history.can_undo());
        studio.begin_event_draft(Some(event_factory::Kind::Emotion));
        studio.source.push_str("\n# changed after draft\n");
        let changed = studio.source.clone();
        studio.apply_event_draft();
        assert_eq!(studio.source, changed);
        assert!(studio.event_edit_error.is_some());
        studio.validate();
        studio.selected_event = Some(0);
        studio.request_event_delete();
        studio.selected_event = Some(1);
        studio.confirm_event_delete();
        assert_eq!(studio.source, changed);
        assert!(!studio.history.can_undo());
    }
    #[test]
    fn retiming_existing_event_does_not_insert_an_open_creation_draft() {
        let mut studio = Studio::default();
        studio.validate();
        let count = studio.script.as_ref().unwrap().scenes[0].events.len();
        let event = studio.script.as_ref().unwrap().scenes[0].events[0].clone();
        studio.begin_event_draft(Some(event_factory::Kind::Emotion));
        let mut drag = timeline_edit::Drag::new(0, 0, &event, 0.0).unwrap();
        drag.update(200.0, 1200.0, 12.0, 24).unwrap();
        studio.timeline_drag_source = Some(studio.source.clone());
        studio.finish_timeline_drag(drag);
        assert_eq!(
            studio.script.as_ref().unwrap().scenes[0].events.len(),
            count
        );
        assert!(!studio.inserting_event);
    }
    #[test]
    fn voice_apply_updates_jobs_and_undo_restores_source() {
        let mut studio = Studio::default();
        studio.validate();
        let original = studio.source.clone();
        studio.selected_actor = Some("EMMA".into());
        studio.voice_draft = voice_editor::Draft::new(studio.script.as_ref().unwrap(), "EMMA");
        studio.voice_draft_source = Some(original.clone());
        let voice = studio.voice_draft.as_mut().unwrap().voice.as_mut().unwrap();
        voice.preset = "am_michael".into();
        voice.speed = 1.25;
        studio.scene_alignment = r#"{"0":{"fixture":"changed"},"1":{"fixture":"keep"}}"#.into();
        studio.alignment = "old selected line".into();
        studio.apply_voice_draft();
        let edited = studio.source.clone();
        assert_ne!(edited, original);
        assert!(
            studio
                .jobs
                .iter()
                .filter(|job| job.actor == "EMMA")
                .all(|job| job.request.voice.preset == "am_michael"
                    && job.request.voice.speed == 1.25)
        );
        let alignment: serde_json::Value = serde_json::from_str(&studio.scene_alignment).unwrap();
        assert!(alignment.get("0").is_none());
        assert_eq!(alignment["1"]["fixture"], "keep");
        assert!(studio.alignment.is_empty());
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        studio.undo_edit(true);
        assert_eq!(studio.source, edited);
    }
    #[test]
    fn invalid_stale_and_noop_voice_apply_preserve_source_history_and_pending_text() {
        let mut studio = Studio::default();
        studio.validate();
        let original = studio.source.clone();
        studio.selected_actor = Some("EMMA".into());
        studio.voice_draft = voice_editor::Draft::new(studio.script.as_ref().unwrap(), "EMMA");
        studio.voice_draft_source = Some(original.clone());
        studio.pending_text_edit = Some(original.clone());
        studio.apply_voice_draft();
        assert_eq!(studio.source, original);
        assert!(!studio.history.can_undo());
        assert!(studio.pending_text_edit.is_some());
        studio
            .voice_draft
            .as_mut()
            .unwrap()
            .voice
            .as_mut()
            .unwrap()
            .speed = f32::NAN;
        studio.apply_voice_draft();
        assert!(studio.voice_edit_error.is_some());
        assert_eq!(studio.source, original);
        assert!(!studio.history.can_undo());
        assert!(studio.pending_text_edit.is_some());
        studio.voice_draft = voice_editor::Draft::new(studio.script.as_ref().unwrap(), "EMMA");
        studio.voice_draft.as_mut().unwrap().voice = None;
        studio.apply_voice_draft();
        assert!(studio.voice_edit_error.is_some());
        assert_eq!(studio.source, original);
        studio.voice_draft.as_mut().unwrap().voice = Some(motion_script::VoiceConfig {
            provider: "kokoro".into(),
            preset: "am_michael".into(),
            language: "en-us".into(),
            speed: 1.0,
        });
        studio.export_busy = true;
        studio.apply_voice_draft();
        assert_eq!(studio.source, original);
        studio.export_busy = false;
        studio.selected_actor = Some("ALEX".into());
        studio.apply_voice_draft();
        assert_eq!(studio.source, original);
        assert!(!studio.history.can_undo());
    }
    #[test]
    fn voice_apply_invalidates_generated_alignment_across_scenes_but_preserves_imported_audio() {
        let mut studio = Studio::default();
        let source = format!("{}\n  - id: next\n    duration: 2.0\n    events:\n      - {{type: speak, at: 0.0, actor: EMMA, text: More speech}}\n",studio.source);
        let script = motion_script::parse(&source).unwrap();
        let index = script.scenes[0]
            .events
            .iter()
            .position(|event| matches!(event, motion_script::Event::Speak { .. }))
            .unwrap();
        let mut imported = script.scenes[0].events[index].clone();
        if let motion_script::Event::Speak { audio, .. } = &mut imported {
            *audio = Some("recorded.wav".into());
        }
        studio.source = motion_script::apply_event(&source, 0, index, imported).unwrap();
        studio.history.reset(&studio.source);
        studio.validate();
        studio.selected_actor = Some("EMMA".into());
        studio.voice_draft = voice_editor::Draft::new(studio.script.as_ref().unwrap(), "EMMA");
        studio.voice_draft_source = Some(studio.source.clone());
        studio
            .voice_draft
            .as_mut()
            .unwrap()
            .voice
            .as_mut()
            .unwrap()
            .speed = 1.5;
        studio.scene_alignment=r#"{"0":{"fixture":"imported"},"1":{"fixture":"other"},"2":{"fixture":"changed-next-scene"}}"#.into();
        studio.selected = 0;
        studio.alignment = "imported selected line".into();
        studio.apply_voice_draft();
        let alignment: serde_json::Value = serde_json::from_str(&studio.scene_alignment).unwrap();
        assert_eq!(alignment["0"]["fixture"], "imported");
        assert_eq!(alignment["1"]["fixture"], "other");
        assert!(alignment.get("2").is_none());
        assert_eq!(studio.alignment, "imported selected line");
        assert!(studio.jobs[0].imported_audio.is_some());
    }
    fn settings_studio() -> Studio {
        let mut studio = Studio::default();
        studio.validate();
        studio.settings_draft = scene_settings::Draft::new(studio.script.as_ref().unwrap(), 0);
        studio.settings_draft_source = Some(studio.source.clone());
        studio
    }
    #[test]
    fn settings_apply_preserves_timing_and_supports_exact_undo() {
        let mut studio = settings_studio();
        let original = studio.source.clone();
        let events =
            serde_json::to_string(&studio.script.as_ref().unwrap().scenes[0].events).unwrap();
        studio.scene_alignment = "timing map fixture".into();
        studio.alignment = "selected line timing".into();
        let draft = studio.settings_draft.as_mut().unwrap();
        draft.project.title = "Updated studio".into();
        draft.project.fps = 60;
        draft.scene_id = "renamed".into();
        draft.duration_seconds += 2.0;
        studio.apply_settings_draft();
        assert!(studio.settings_error.is_none());
        let script = studio.script.as_ref().unwrap();
        assert_eq!(script.project.title, "Updated studio");
        assert_eq!(script.project.fps, 60);
        assert_eq!(script.scenes[0].id, "renamed");
        assert_eq!(
            serde_json::to_string(&script.scenes[0].events).unwrap(),
            events
        );
        assert!(studio.jobs.iter().any(|job| job.scene == "renamed"));
        assert_eq!(studio.scene_alignment, "timing map fixture");
        assert_eq!(studio.alignment, "selected line timing");
        let edited = studio.source.clone();
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        studio.undo_edit(true);
        assert_eq!(studio.source, edited);
    }
    #[test]
    fn settings_rejections_and_noop_leave_source_history_intact() {
        let mut studio = settings_studio();
        let original = studio.source.clone();
        studio.apply_settings_draft();
        assert!(!studio.history.can_undo());
        studio.settings_draft.as_mut().unwrap().duration_seconds = 0.01;
        studio.apply_settings_draft();
        assert!(studio.settings_error.is_some());
        assert_eq!(studio.source, original);
        assert!(!studio.history.can_undo());
        studio.settings_draft.as_mut().unwrap().project.title = "Changed".into();
        studio.settings_draft_source = Some("stale".into());
        studio.apply_settings_draft();
        assert!(studio.settings_error.as_ref().unwrap().contains("changed"));
        assert_eq!(studio.source, original);
        studio.settings_draft_source = Some(original.clone());
        studio.busy = true;
        studio.apply_settings_draft();
        assert_eq!(studio.source, original);
        assert!(!studio.history.can_undo());
    }
    #[test]
    fn native_workspace_headless_layout_and_optional_capture() {
        let ctx = egui::Context::default();
        studio_theme(&ctx);
        let mut studio = Studio::default();
        studio.validate();
        let mut textures = Vec::new();
        let mut final_output = None;
        for time in [0.0, 0.1] {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1440.0, 900.0),
                    )),
                    time: Some(time),
                    ..Default::default()
                },
                |ctx| {
                    studio.workspace_ui(ctx);
                },
            );
            for (id, delta) in &output.textures_delta.set {
                let pixels: Vec<[u8; 4]> = match &delta.image {
                    egui::ImageData::Color(image) => {
                        image.pixels.iter().map(|p| p.to_array()).collect()
                    }
                    egui::ImageData::Font(image) => {
                        image.srgba_pixels(None).map(|p| p.to_array()).collect()
                    }
                };
                textures.push(serde_json::json!({"id": format!("{id:?}"), "size": delta.image.size(), "pos": delta.pos, "pixels": pixels}));
            }
            assert!(!output.shapes.is_empty());
            final_output = Some(output);
        }
        if let Ok(path) = std::env::var("KILSAT_UI_CAPTURE") {
            let output = final_output.unwrap();
            let meshes: Vec<_> = ctx.tessellate(output.shapes, output.pixels_per_point).into_iter().filter_map(|primitive| {
                if let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive {
                    let vertices: Vec<_> = mesh.vertices.iter().map(|v| serde_json::json!({"pos": [v.pos.x,v.pos.y],"uv": [v.uv.x,v.uv.y],"color":v.color.to_array()})).collect();
                    Some(serde_json::json!({"texture":format!("{:?}",mesh.texture_id),"vertices":vertices,"indices":mesh.indices,"clip":[primitive.clip_rect.min.x,primitive.clip_rect.min.y,primitive.clip_rect.max.x,primitive.clip_rect.max.y]}))
                } else { None }
            }).collect();
            std::fs::write(path, serde_json::to_vec(&serde_json::json!({"width":1440,"height":900,"pixels_per_point":output.pixels_per_point,"textures":textures,"meshes":meshes})).unwrap()).unwrap();
        }
        // Minimum window size and editor tab must remain runnable without a GPU.
        studio.editor_tab = true;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 700.0),
                )),
                ..Default::default()
            },
            |ctx| {
                studio.workspace_ui(ctx);
            },
        );
        assert!(!output.shapes.is_empty());
    }
}
