//! Native dialogue workbench. Audio assets are prepared on bounded background workers.
mod alignment_panel;
mod alignment_worker;
mod art_editor;
mod art_resources;
mod asset_worker;
mod bone_editor;
mod clip_editor;
mod clip_library;
mod composition_editor;
mod custom_rig_editor;
mod demo;
mod event_editor;
mod event_factory;
mod export_worker;
mod gpu_check_panel;
mod gpu_check_worker;
mod keyframe_editor;
mod kokoro_setup;
mod live_clip_editor;
mod managed_license_evidence;
mod native_compositor;
mod playback;
mod project_history;
mod runtime_check;
mod scene_settings;
mod scene_worker;
mod sequence_panel;
mod sequence_worker;
mod timeline_edit;
mod voice_editor;

use eframe::egui;
use motion_audio::WaveformPeak;
#[cfg(test)]
use motion_lipsync::Alignment;
use motion_lipsync::{
    cache::{AlignmentCache, AlignmentStatus},
    Viseme,
};
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
    line_id: u64,
    transcript: String,
    alignment_status: AlignmentStatus,
    clip: DialogueClip,
    peaks: Vec<WaveformPeak>,
    result: SpeechResult,
    playback_audio: Arc<motion_audio::AudioBuffer>,
}
struct ScenePreview {
    scene: String,
    alignment_statuses: std::collections::BTreeMap<u64, AlignmentStatus>,
    clips: Vec<DialogueClip>,
    audio: Arc<motion_audio::AudioBuffer>,
    manifest: Arc<motion_voice::export::SceneManifest>,
    peaks: Vec<WaveformPeak>,
}
struct Prepare {
    job: DialogueJob,
    result: SpeechResult,
    alignment: String,
    alignment_cache_root: std::path::PathBuf,
    cancel: CancellationToken,
}
type PreviewResult = Result<Preview, String>;
struct NativeCanvas {
    revision: u64,
    request: native_compositor::Request,
    line_audio: Option<Arc<motion_audio::AudioBuffer>>,
    last_requested: Option<u64>,
    shown: Option<u64>,
    failed: Option<u64>,
    texture: Option<egui::TextureHandle>,
    error: Option<String>,
}
fn same_optional_arc<T>(a: &Option<Arc<T>>, b: &Option<Arc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}
struct Studio {
    native_alignment: alignment_panel::Panel,
    alignment_identity: Option<alignment_panel::Identity>,
    alignment_audio: Option<Arc<motion_audio::AudioBuffer>>,
    compositor: native_compositor::Worker,
    native_canvas: Option<NativeCanvas>,
    native_canvas_generation: u64,
    gpu_check: gpu_check_panel::Panel,
    sequence_panel: sequence_panel::Panel,
    sequence_revision: u64,
    document_revision: u64,
    sequence_images: Option<Arc<std::collections::BTreeMap<String, motion_art::ImageResource>>>,
    sequence_art_revision: u64,
    art_editor: art_editor::Editor,
    art_resources: art_resources::Resources,
    keyframe_draft: Option<(String, keyframe_editor::Draft)>,
    bone_draft: Option<(String, bone_editor::Draft)>,
    joint_draft: Option<(String, bone_editor::Draft)>,
    selected_joint: String,
    custom_rig_draft: Option<(String, custom_rig_editor::Draft)>,
    clip_draft: Option<(String, clip_editor::Draft)>,
    clip_track_draft: Option<(String, clip_editor::TrackDraft)>,
    clip_library_draft: Option<(String, usize, String, clip_library::State)>,
    live_clip_draft: Option<(String, usize, String, live_clip_editor::State)>,
    selected_bone: String,
    asset_worker: asset_worker::Worker,
    asset_pending: Option<(String, String, motion_project::ProjectDocument)>,
    asset_id: String,
    asset_path: String,
    asset_kind: usize,
    asset_dirty: bool,
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
    history: project_history::History,
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
    kokoro_setup: kokoro_setup::Worker,
    setup_busy: bool,
    setup_root: String,
    setup_python: String,
    setup_downloads: bool,
    setup_voice: String,
    setup_status: String,
    setup_profile: Option<kokoro_setup::Profile>,
    test_audio: Option<Arc<motion_audio::AudioBuffer>>,
    test_player: Option<playback::Playback>,
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
                    let alignment = scene_worker::parse_alignment_input(&task.alignment)?;
                    let (clip, alignment_status) = DialogueClip::prepare_with_alignment_input(
                        &task.job,
                        &task.result,
                        alignment.as_ref(),
                        &AlignmentCache::new(&task.alignment_cache_root),
                    )
                    .map_err(|e| e.to_string())?;
                    let bin = (clip.audio.frames() / 700).max(1);
                    let peaks = motion_audio::waveform_peaks(&clip.audio, bin)
                        .map_err(|e| e.to_string())?;
                    let playback_audio = Arc::new(clip.audio.clone());
                    task.cancel.check().map_err(|e| e.to_string())?;
                    Ok(Preview {
                        scene: task.job.scene.clone(),
                        line_id: task.job.id,
                        transcript: task.job.request.text.clone(),
                        alignment_status,
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
            art_editor: Default::default(),
            custom_rig_draft: None,
            art_resources: Default::default(),
            native_alignment: alignment_panel::Panel::new(),
            alignment_identity: None,
            alignment_audio: None,
            compositor: native_compositor::Worker::new(),
            native_canvas: None,
            native_canvas_generation: 0,
            gpu_check: Default::default(),
            sequence_panel: Default::default(),
            sequence_revision: 0,
            document_revision: 0,
            sequence_images: None,
            sequence_art_revision: 0,
            keyframe_draft: None,
            bone_draft: None,
            joint_draft: None,
            selected_joint: String::new(),
            clip_library_draft: None,
            live_clip_draft: None,
            clip_draft: None,
            clip_track_draft: None,
            selected_bone: "hip".into(),
            asset_worker: asset_worker::Worker::new(),
            asset_pending: None,
            asset_id: String::new(),
            asset_path: String::new(),
            asset_kind: 0,
            asset_dirty: false,
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
            history: project_history::History::new(include_str!(
                "../../../examples/hello-emma.yaml"
            )),
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
            kokoro_setup: kokoro_setup::Worker::spawn(),
            setup_busy: false,
            setup_root: default_kokoro_root(),
            setup_python: String::new(),
            setup_downloads: false,
            setup_voice: "af_heart".into(),
            setup_status: String::new(),
            setup_profile: None,
            test_audio: None,
            test_player: None,
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
                if let Some(document) = &self.project_document {
                    let mut candidate = document.clone();
                    candidate.source = source.clone();
                    if let Err(error) = candidate.validate() {
                        self.status = format!(
                            "Edit conflicts with artwork: {error}. Remove related layers first."
                        );
                        return;
                    }
                }
                self.flush_text_edit();
                self.record_source_edit(&self.source.clone(), &source);
                self.source = source;
                self.document_changed();
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

    fn apply_timeline_edit(&mut self, edit: motion_scene::editable_timeline::TimelineEdit) {
        if self.busy || self.diagnostics_busy || self.export_busy {
            return;
        }
        let result = (|| -> Result<(String, String), String> {
            let script = motion_script::parse(&self.source).map_err(|e| e.to_string())?;
            let selected = script
                .scenes
                .get(self.scene_index)
                .ok_or("Selected scene is no longer available")?
                .id
                .clone();
            let mut document =
                motion_scene::editable_timeline::EditableTimeline::from_script(&script)
                    .map_err(|e| e.to_string())?;
            let compiled = document.apply(&script, edit).map_err(|e| e.to_string())?;
            let source = motion_script::to_source(&compiled.script).map_err(|e| e.to_string())?;
            Ok((source, selected))
        })();
        match result {
            Ok((source, selected)) => {
                self.apply_structure(Ok(source.clone()));
                if self.source == source {
                    if let Some(index) = self
                        .script
                        .as_ref()
                        .and_then(|s| s.scenes.iter().position(|scene| scene.id == selected))
                    {
                        self.scene_index = index;
                        self.reset_scene();
                    }
                    self.status = "Scene order updated. Transitions and timing were validated; Undo restores the previous source.".into();
                }
            }
            Err(error) => self.status = format!("Timeline edit: {error}"),
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
                    let result = motion_script::add_scene(&self.source, motion_script::Scene { composition: Default::default(), id: id.clone(), duration: 5.0, background: None, events: vec![] });
                    self.apply_structure(result);
                }
                if ui.button("Duplicate selected scene").clicked() {
                    self.apply_structure(motion_script::duplicate_scene(&self.source, self.scene_index, &id));
                }
                let scene_ids: Vec<_> = self.script.as_ref().map(|s| s.scenes.iter()
                    .map(|scene| scene.id.clone()).collect()).unwrap_or_default();
                if let Some(selected) = scene_ids.get(self.scene_index).cloned() {
                    ui.horizontal(|ui| {
                        if ui.add_enabled(self.scene_index > 0, egui::Button::new("Move scene earlier")).clicked() {
                            self.apply_timeline_edit(motion_scene::editable_timeline::TimelineEdit::MoveScene {
                                scene_id: selected.clone(), before: scene_ids.get(self.scene_index - 1).cloned(),
                            });
                        }
                        if ui.add_enabled(self.scene_index + 1 < scene_ids.len(), egui::Button::new("Move scene later")).clicked() {
                            self.apply_timeline_edit(motion_scene::editable_timeline::TimelineEdit::MoveScene {
                                scene_id: selected.clone(), before: scene_ids.get(self.scene_index + 2).cloned(),
                            });
                        }
                    });
                }
                if ui.button("Delete selected scene…").clicked() {
                    if let Some(scene) = self.script.as_ref().and_then(|s| s.scenes.get(self.scene_index)) {
                        self.lifecycle_delete = Some((true, scene.id.clone(), self.source.clone()));
                    }
                }
                if ui.button("Add character").clicked() {
                    self.apply_structure(motion_script::add_character(&self.source, &id, motion_script::Character { asset: self.lifecycle_asset.trim().into(), default_pose: None, voice: None, rig: None }));
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
            && scene.duration == draft.duration_seconds
            && serde_json::to_value(&scene.composition).ok()
                == serde_json::to_value(&draft.composition).ok();
        if unchanged {
            self.settings_error = None;
            self.status = "Project and scene settings unchanged.".into();
            return;
        }
        let source = match motion_script::apply_scene_settings_with_composition(
            &self.source,
            draft.scene_index,
            draft.project.clone(),
            &draft.scene_id,
            draft.duration_seconds,
            draft.composition.clone(),
        ) {
            Ok(source) => source,
            Err(error) => {
                self.settings_error = Some(error.to_string());
                return;
            }
        };
        if let Some(document) = &self.project_document {
            let mut candidate = document.clone();
            candidate.source = source.clone();
            if let Err(error) = candidate.validate() {
                self.settings_error = Some(format!(
                    "Edit conflicts with artwork: {error}. Remove related layers first."
                ));
                return;
            }
        }
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
        self.record_source_edit(&self.source.clone(), &source);
        self.source = source;
        self.document_changed();
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
        if self
            .settings_draft
            .as_ref()
            .is_none_or(|draft| draft.scene_index != self.scene_index)
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
                self.record_source_edit(&self.source.clone(), &source);
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
                self.document_changed();
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
        if self
            .voice_draft
            .as_ref()
            .is_none_or(|draft| draft.actor != actor)
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
                self.record_source_edit(&self.source.clone(), &source);
                self.source = source;
                self.document_changed();
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
            || self.native_alignment.generation().is_some()
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
            self.record_source_edit(&before, &self.source.clone());
        }
    }
    fn record_source_edit(&mut self, before: &str, after: &str) {
        self.history.record(
            project_history::Snapshot::capture(before, self.project_document.as_ref()),
            project_history::Snapshot::capture(after, self.project_document.as_ref()),
        );
    }
    fn snapshot(&self) -> project_history::Snapshot {
        project_history::Snapshot::capture(&self.source, self.project_document.as_ref())
    }
    /// Called only after the artwork editor reports an actual committed edit.
    fn record_art_transaction(&mut self) {
        let Some(before_art) = self.art_editor.take_transaction_before() else {
            return;
        };
        let Some(document) = self.project_document.as_mut() else {
            return;
        };
        let mut before = document.clone();
        before.art = before_art;
        before.source = self.source.clone();
        let mut candidate = document.clone();
        candidate.source = self.source.clone();
        if let Err(error) = candidate.validate() {
            document.art = before.art;
            self.art_editor.error = Some(error.to_string());
            return;
        }
        // A canvas click can finish an in-progress screenplay text transaction.
        // Keep its metadata from BEFORE this art edit so undo order stays exact.
        if let Some(source) = self.pending_text_edit.take() {
            self.history.record(
                project_history::Snapshot::capture(&source, Some(&before)),
                project_history::Snapshot::capture(&self.source, Some(&before)),
            );
        }
        self.history.record(
            project_history::Snapshot::capture(&self.source, Some(&before)),
            project_history::Snapshot::capture(&self.source, Some(document)),
        );
    }
    fn undo_edit(&mut self, redo: bool) {
        if self.busy || self.diagnostics_busy || self.export_busy {
            return;
        }
        self.flush_text_edit();
        let current = self.snapshot();
        match self.history.navigate(current, redo) {
            Ok(Some(snapshot)) => {
                let restored_document = snapshot.document();
                let inventory_changed = self.project_document.as_ref().map(|d| &d.assets)
                    != restored_document.as_ref().map(|d| &d.assets);
                let art_changed = self.project_document.as_ref().map(|d| &d.art)
                    != restored_document.as_ref().map(|d| &d.art);
                let art_dirty = self.art_editor.dirty || art_changed;
                self.project_document = restored_document;
                self.source = snapshot.source;
                self.document_changed();
                self.art_editor.reset();
                self.art_editor.dirty = art_dirty;
                self.asset_dirty |= inventory_changed;
                self.alignment.clear();
                self.scene_alignment.clear();
                self.stop_playback();
                self.invalidate_scene();
                self.jobs.clear();
                self.preview = None;
                self.validate();
                if let Some(document) = &self.project_document {
                    let mut document = document.clone();
                    document.source = self.source.clone();
                    if let Err(error) = document.validate() {
                        self.script = None;
                        self.jobs.clear();
                        self.status = format!("History restored an unfinished screenplay: {error}");
                    }
                }
            }
            Ok(None) => {}
            Err(error) => self.status = format!("History unchanged: {error}"),
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
                self.record_source_edit(&self.source.clone(), &source);
                if edits_speech && insertion {
                    self.alignment.clear();
                    self.scene_alignment.clear();
                } else if edits_speech {
                    self.clear_event_alignment(scene, event);
                }
                self.source = source;
                self.document_changed();
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
                self.art_editor.reset();
                self.project_document = None;
                self.asset_dirty = false;
                self.source = assets.source.clone();
                self.document_changed();
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
    /// Document/configuration mutation boundary; idle UI frames never serialize inputs.
    fn sequence_changed(&mut self) {
        self.sequence_revision = self.sequence_revision.wrapping_add(1);
        let was_busy = self.sequence_panel.busy();
        self.sequence_panel
            .invalidate(self.sequence_revision.to_string());
        if was_busy {
            self.busy = false;
        }
    }
    fn sync_sequence_resources(&mut self) {
        // Resources publishes a fresh immutable Arc for every decode/reload result.
        // Retaining the old Arc also prevents allocator address reuse from hiding changes.
        let images_changed = self
            .sequence_images
            .as_ref()
            .is_none_or(|previous| !Arc::ptr_eq(previous, &self.art_resources.images));
        let art_revision = self.art_editor.revision();
        if images_changed || self.sequence_art_revision != art_revision {
            self.sequence_images = Some(Arc::clone(&self.art_resources.images));
            self.sequence_art_revision = art_revision;
            self.sequence_changed();
        }
    }
    fn document_changed(&mut self) {
        self.document_revision = self.document_revision.wrapping_add(1);
        self.sequence_changed();
    }
    fn invalidate_scene(&mut self) {
        self.custom_rig_draft = None;
        self.bone_draft = None;
        self.joint_draft = None;
        self.clip_draft = None;
        self.clip_track_draft = None;
        self.clip_library_draft = None;
        self.live_clip_draft = None;
        self.document_changed();
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
        self.custom_rig_draft = None;
        self.bone_draft = None;
        self.joint_draft = None;
        self.clip_draft = None;
        self.clip_track_draft = None;
        self.clip_library_draft = None;
        self.live_clip_draft = None;
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
        self.sequence_panel.stop_transport();
        self.stop_other_playback();
    }
    fn stop_other_playback(&mut self) {
        if let Some(player) = self.test_player.take() {
            let _ = player.stop();
        }
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
                    self.document_changed();
                    self.project_document = Some(doc);
                    self.art_editor.reset();
                    self.asset_dirty = false;
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
                self.art_editor.reset();
                self.project_document = None;
                self.asset_dirty = false;
                self.source = source;
                self.document_changed();
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
                    self.asset_dirty = false;
                    self.art_editor.dirty = false;
                    self.flush_text_edit();
                    self.asset_dirty = false;
                    self.status = "Saved versioned project and asset inventory.".into();
                }
                Err(error) => self.status = format!("Save project: {error}"),
            }
            return;
        }
        if self
            .project_document
            .as_ref()
            .is_some_and(|d| !d.art.layers.is_empty())
        {
            self.status =
                "Artwork requires .kilsat. Change the save path to .kilsat before saving.".into();
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
                self.sequence_panel.stop_transport();
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
                self.sequence_panel.stop_transport();
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
        if let Some(preview) = &self.scene_preview {
            for (id, status) in &preview.alignment_statuses {
                ui.label(format!("Dialogue {id}: {}", alignment_status_text(*status)));
            }
        }
        ui.label("One mixed source clock drives all actor mouths. Missing alignment keeps mouths at rest. Device latency and perceptual timing require Mac validation.");
        if let Some(error) = &self.playback_error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
    }
    fn eligible_alignment_preview(&self) -> Option<&Preview> {
        let job = self.jobs.get(self.selected)?;
        self.preview.as_ref().filter(|p| {
            p.line_id == job.id
                && p.transcript == job.request.text
                && p.scene == job.scene
                && p.clip.actor == job.actor
                && p.clip.start_frame == (job.at * 24000.0).round() as u64
        })
    }
    fn sync_native_alignment(&mut self) {
        let current = self.eligible_alignment_preview();
        let unchanged = match (
            current,
            self.alignment_identity.as_ref(),
            self.alignment_audio.as_ref(),
        ) {
            (Some(p), Some(id), Some(audio)) => {
                id.generation == self.sequence_revision
                    && id.line_id == p.line_id
                    && Arc::ptr_eq(audio, &p.playback_audio)
            }
            (None, None, None) => true,
            _ => false,
        };
        if !unchanged {
            let next = current.map(|p| {
                (
                    alignment_panel::Identity {
                        generation: self.sequence_revision,
                        line_id: p.line_id,
                        audio_path: p.result.audio_path.clone(),
                        transcript: p.transcript.clone(),
                    },
                    p.playback_audio.clone(),
                )
            });
            self.native_alignment.invalidate();
            (self.alignment_identity, self.alignment_audio) = match next {
                Some((id, audio)) => (Some(id), Some(audio)),
                None => (None, None),
            };
        }
        let was_busy = self.native_alignment.busy();
        self.native_alignment.tick(self.alignment_identity.as_ref());
        if was_busy && !self.native_alignment.busy() {
            self.busy = false;
        }
    }
    fn native_alignment_ui(&mut self, ui: &mut egui::Ui) {
        self.sync_native_alignment();
        let can_run = !self.busy
            && !self.diagnostics_busy
            && !self.export_busy
            && !self.setup_busy
            && self.alignment_identity.is_some();
        if self.native_alignment.ui(ui, can_run) {
            if let (Some(id), Some(audio)) = (
                self.alignment_identity.clone(),
                self.alignment_audio.clone(),
            ) {
                let expected: Arc<dyn alignment_worker::PreparedAudio> = audio;
                let cache_root = self
                    .kokoro_config()
                    .cache_directory
                    .join("estimated-alignments");
                match self.native_alignment.start(
                    id,
                    expected,
                    self.scene_alignment.clone(),
                    cache_root,
                ) {
                    Ok(()) => {
                        self.stop_playback();
                        self.busy = true;
                    }
                    Err(e) => self.status = e,
                }
            }
        }
        if let Some(ready) = self.native_alignment.take_ready() {
            self.apply_native_alignment(ready);
        }
    }
    fn apply_native_alignment(&mut self, ready: alignment_panel::Ready) {
        if self.busy
            || self.diagnostics_busy
            || self.export_busy
            || self.alignment_identity.as_ref() != Some(&ready.identity)
        {
            return;
        }
        let Some(preview) = self.eligible_alignment_preview() else {
            return;
        };
        let result = preview.result.clone();
        let Some(job) = self.jobs.get(self.selected).cloned() else {
            return;
        };
        self.stop_playback();
        self.alignment = ready.json;
        self.scene_alignment = ready.scene_json;
        self.sequence_changed();
        self.scene_preview = None;
        self.scene_audio_active = false;
        self.preview = None;
        self.native_alignment.invalidate();
        self.alignment_identity = None;
        self.alignment_audio = None;
        let cancel = CancellationToken::new();
        match self.prepare_tx.try_send(Prepare {
            job,
            result,
            alignment: self.alignment.clone(),
            alignment_cache_root: self.kokoro_config().cache_directory.join("alignments"),
            cancel: cancel.clone(),
        }) {
            Ok(()) => {
                self.busy = true;
                self.active_cancel = Some(cancel);
                self.status =
                    "Preparing existing audio with estimated timing; no synthesis or download."
                        .into();
            }
            Err(e) => self.status = format!("Timing applied; prepare the line again: {e}"),
        }
    }
    fn advanced_canvas_needed(&self) -> bool {
        // Keep prepared library tracks in the existing bounded compositor worker.
        if self.script.as_ref().is_some_and(|script| {
            script
                .characters
                .values()
                .any(|character| character.rig.is_some())
                || script
                    .scenes
                    .iter()
                    .any(|scene| !scene.composition.clip_instances.is_empty())
        }) {
            return true;
        }
        self.script
            .as_ref()
            .and_then(|s| s.scenes.get(self.scene_index))
            .zip(self.project_document.as_ref())
            .is_some_and(|(scene, doc)| {
                requires_composited_preview(
                    &doc.art,
                    &scene.id,
                    self.playhead.clamp(0.0, scene.duration),
                )
            })
    }
    fn native_canvas_ui(&mut self, ui: &mut egui::Ui) {
        if let Some(document) = &self.project_document {
            self.art_resources
                .sync(ui.ctx(), &self.project_root, &document.assets);
        }
        self.sync_sequence_resources();
        let Some(script) = self.script.as_ref() else {
            return;
        };
        let Some(scene) = script.scenes.get(self.scene_index) else {
            return;
        };
        let total = (scene.duration * 24000.0).round() as u64;
        if total == 0 || !self.playhead.is_finite() {
            ui.label("Invalid preview clock.");
            return;
        }
        let sample = ((self.playhead.max(0.0) * 24000.0).round() as u64).min(total);
        let manifest = self
            .scene_preview
            .as_ref()
            .filter(|p| p.scene == scene.id)
            .map(|p| p.manifest.clone());
        let line_audio = self
            .preview
            .as_ref()
            .filter(|p| p.scene == scene.id)
            .map(|p| p.playback_audio.clone());
        let changed = self.native_canvas.as_ref().is_none_or(|canvas| {
            canvas.revision != self.sequence_revision
                || canvas.request.scene_index != self.scene_index
                || !same_optional_arc(&canvas.request.manifest, &manifest)
                || !same_optional_arc(&canvas.line_audio, &line_audio)
        });
        if changed {
            self.native_canvas_generation = self.native_canvas_generation.wrapping_add(1);
            self.native_canvas = Some(NativeCanvas {
                revision: self.sequence_revision,
                request: native_compositor::Request {
                    generation: self.native_canvas_generation,
                    scene_index: self.scene_index,
                    sample_frame: sample,
                    script: Arc::new(script.clone()),
                    art: Arc::new(motion_export::ArtContext {
                        document: self
                            .project_document
                            .as_ref()
                            .map(|p| p.art.clone())
                            .unwrap_or_default(),
                        images: (*self.art_resources.images).clone(),
                    }),
                    manifest,
                    mouths: None,
                },
                line_audio,
                last_requested: None,
                shown: None,
                failed: None,
                texture: None,
                error: None,
            });
        }
        let playing = self
            .scene_transport
            .as_ref()
            .is_some_and(|p| p.is_playing())
            || self.playback.as_ref().is_some_and(|p| p.snapshot().playing);
        let canvas = self.native_canvas.as_mut().expect("canvas initialized");
        while let Some(update) = self.compositor.poll() {
            if update.generation != canvas.request.generation
                || update.scene_index != self.scene_index
                || (!playing && update.sample_frame != sample)
            {
                continue;
            }
            match update.result {
                Ok(frame) => {
                    canvas.texture = Some(ui.ctx().load_texture(
                        "selected-compositor",
                        egui::ColorImage::from_rgba_premultiplied(
                            [frame.width as usize, frame.height as usize],
                            &frame.rgba,
                        ),
                        egui::TextureOptions::LINEAR,
                    ));
                    canvas.shown = Some(update.sample_frame);
                    canvas.failed = None;
                    canvas.error = None;
                }
                Err(e) => {
                    canvas.failed = Some(update.sample_frame);
                    canvas.error = Some(e);
                }
            }
        }
        if canvas.last_requested != Some(sample) && canvas.failed.is_none() {
            let mouths =
                if let Some(p) = self.scene_preview.as_ref().filter(|p| p.scene == scene.id) {
                    scene_mouths(&p.clips, sample)
                        .into_iter()
                        .map(|(a, v)| (a.to_owned(), v))
                        .collect()
                } else {
                    self.preview
                        .as_ref()
                        .filter(|p| p.scene == scene.id)
                        .and_then(|p| {
                            p.clip
                                .mouth_at(sample)
                                .ok()
                                .map(|v| (p.clip.actor.clone(), v))
                        })
                        .into_iter()
                        .collect()
                };
            let mut request = canvas.request.clone();
            request.sample_frame = sample;
            request.mouths = Some(mouths);
            match self.compositor.request(request) {
                Ok(()) => canvas.last_requested = Some(sample),
                Err(e) => {
                    canvas.error = Some(e);
                    canvas.failed = Some(sample);
                }
            }
        }
        ui.small("Shared scene preview · masks, blending, camera and timed mouths. Select artwork tools in the Inspector.");
        let canvas_rect = if let Some(texture) = &canvas.texture {
            let scale = (ui.available_width() / texture.size_vec2().x).min(1.0);
            Some(ui.image((texture.id(), texture.size_vec2() * scale)).rect)
        } else {
            ui.spinner();
            None
        };
        if let Some(error) = &canvas.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
            if ui.button("Retry scene preview").clicked() {
                canvas.failed = None;
                canvas.last_requested = None;
                ui.ctx().request_repaint();
            }
        }
        if canvas.shown != Some(sample) && canvas.failed.is_none() {
            ui.ctx().request_repaint_after(Duration::from_millis(25));
        }
        if let Some(rect) = canvas_rect {
            if let (Ok(evaluated), Some(document)) = (
                motion_scene::evaluate(script, self.scene_index, self.playhead),
                self.project_document.as_mut(),
            ) {
                let enabled = !self.busy && !self.diagnostics_busy && !self.export_busy
                    && !playing && canvas.shown == Some(sample);
                let response = ui.add_enabled_ui(enabled, |ui| {
                    ui.interact(rect, ui.id().with("art-canvas"), egui::Sense::drag())
                }).inner;
                self.art_editor.gesture(&response, rect, &evaluated, &scene.id, &mut document.art);
            }
        }
        self.record_art_transaction();
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
        if self.advanced_canvas_needed() {
            self.native_canvas_ui(ui);
            return;
        }
        if let Some(script) = &self.script {
            match motion_scene::evaluate(script, self.scene_index, self.playhead) {
                Ok(mut scene) => {
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
                    motion_scene::apply_speech(&mut scene, &mouths);
                    let scene_id = &script.scenes[self.scene_index].id;
                    if let Some(document) = &self.project_document {
                        self.art_resources
                            .sync(ui.ctx(), &self.project_root, &document.assets);
                    }
                    let rect = draw_scene(ui, &scene, &mouths, |painter, rect, plane| {
                        if let Some(document) = &self.project_document {
                            art_editor::paint(
                                painter,
                                rect,
                                &scene,
                                scene_id,
                                &document.art,
                                (&self.art_resources.textures, plane),
                            );
                        }
                    });
                    if let Some(document) = &mut self.project_document {
                        let enabled = !self.busy && !self.diagnostics_busy && !self.export_busy;
                        let response = ui.add_enabled_ui(enabled, |ui| {
                            ui.interact(rect, ui.id().with("art-canvas"), egui::Sense::drag())
                        }).inner;
                        self.art_editor.gesture(
                            &response,
                            rect,
                            &scene,
                            scene_id,
                            &mut document.art,
                        );
                    }
                }
                Err(error) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, error.to_string());
                }
            }
        } else {
            ui.label("Validate screenplay to enable deterministic scene preview.");
        }
        self.record_art_transaction();
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
        self.poll_kokoro_setup();
        if let Some(result) = self.asset_worker.poll() {
            self.busy = false;
            if let Some((source, path, previous)) = self.asset_pending.take() {
                match result {
                    Ok(document) if self.source == source && self.script_path == path && self.project_document.as_ref().is_some_and(|current|current.art==previous.art && current.assets==previous.assets) => {
                        self.history.record(
                            project_history::Snapshot::capture(&self.source, Some(&previous)),
                            project_history::Snapshot::capture(&self.source, Some(&document)),
                        );
                        self.project_document = Some(document);
                        self.document_changed();
                        self.asset_dirty = true;
                        self.status = "Asset inventory updated; Save .kilsat to persist. Physical files are never removed by inventory removal.".into();
                    }
                    Ok(_) => self.status = "Project changed during asset operation; inventory result discarded. Imported files remain available on disk.".into(),
                    Err(error) => self.status = format!("Asset operation: {error}"),
                }
            }
        }

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
                            if self
                                .script
                                .as_ref()
                                .and_then(|s| s.scenes.get(self.scene_index))
                                .is_none_or(|s| s.id != prepared.scene_id)
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
                                alignment_statuses: ready.alignment_statuses,
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
                                alignment_cache_root: self
                                    .kokoro_config()
                                    .cache_directory
                                    .join("alignments"),
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
fn alignment_status_text(status: AlignmentStatus) -> &'static str {
    match status {
        AlignmentStatus::EstimatedImported => {
            "Estimated Rhubarb mouth cues imported · separate from phoneme cache · quality not measured"
        }
        AlignmentStatus::None => "No alignment · mouth at rest",
        AlignmentStatus::LegacyUnbound => {
            "Legacy timing · unbound to audio/transcript · quality not measured"
        }
        AlignmentStatus::BoundImported => "Bound timing imported and cached · quality not measured",
        AlignmentStatus::BoundCacheHit => "Bound timing cache hit · quality not measured",
        AlignmentStatus::BoundCacheMiss => "Bound timing cache miss · mouth at rest",
    }
}
fn default_kokoro_root() -> String {
    kokoro_setup::default_root().to_string_lossy().into_owned()
}
impl Studio {
    fn submit_kokoro(&mut self, task: kokoro_setup::Task) {
        if self.busy || self.diagnostics_busy || self.export_busy {
            return;
        }
        match self.kokoro_setup.submit(task) {
            Ok(()) => {
                self.stop_playback();
                self.setup_busy = true;
                self.diagnostics_busy = true;
                self.setup_status = "Working locally…".into();
            }
            Err(error) => self.setup_status = error,
        }
    }
    fn apply_kokoro_profile(&mut self, profile: kokoro_setup::Profile) {
        self.sequence_changed();
        self.stop_playback();
        self.python = profile.python.to_string_lossy().into_owned();
        self.config = profile.config.to_string_lossy().into_owned();
        self.weights = profile.model.to_string_lossy().into_owned();
        self.voices = profile.voices.to_string_lossy().into_owned();
        self.cache = profile.cache.to_string_lossy().into_owned();
        self.revision = profile.revision.clone();
        self.scene_preview = None;
        self.scene_audio_active = false;
        self.preview = None;
        self.diagnostics_report = None;
        self.test_audio = None;
        self.setup_profile = Some(profile);
    }
    fn poll_kokoro_setup(&mut self) {
        while let Some(update) = self.kokoro_setup.poll() {
            match update {
                kokoro_setup::Update::Progress(message) => self.setup_status = message,
                kokoro_setup::Update::Finished(result) => {
                    self.setup_busy = false;
                    self.diagnostics_busy = false;
                    match result {
                        Ok(kokoro_setup::Outcome::Connected(profile)) => {
                            self.apply_kokoro_profile(profile);
                            self.setup_status = "Local Kokoro connected. Run the English CPU speech test, then listen to the result.".into();
                        }
                        Ok(kokoro_setup::Outcome::Tested {
                            wav,
                            report,
                            metrics,
                            audio,
                        }) => {
                            self.test_audio = Some(audio);
                            self.setup_status = format!("CPU speech test completed. {metrics}\nWAV: {}\nReport: {}\nListen to confirm speech quality. This does not validate lip-sync.", wav.display(), report.display());
                        }
                        Err(error) => self.setup_status = error,
                    }
                }
            }
        }
        if let Some(player) = &self.test_player {
            let state = player.snapshot();
            if let Some(error) = state.error {
                self.setup_status = format!("Test playback: {error}");
                self.test_player = None;
            } else if state.finished {
                self.test_player = None;
            }
        }
    }
    fn kokoro_setup_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Kokoro · local speech setup");
        ui.label("Install and test English CPU speech here. Python 3.10–3.12 must already be installed. The model runs as a local background process.");
        let idle = !self.busy && !self.diagnostics_busy && !self.export_busy;
        ui.add_enabled_ui(idle, |ui| {
            field(ui, "Installation folder", &mut self.setup_root);
            field(ui, "Python 3.10–3.12 executable (blank: detect)", &mut self.setup_python);
            ui.checkbox(&mut self.setup_downloads, "I allow downloading Python dependencies, Kokoro model, voices and English language assets for this installation.");
            ui.weak("Installation needs an internet connection and disk space. After setup, speech generation uses local assets. Existing assets can be connected without downloads.");
            let root = std::path::PathBuf::from(self.setup_root.trim());
            let (installer, manifest) = kokoro_setup::helper_paths(Path::new(&self.runtime));
            if ui.add_enabled(self.setup_downloads && !self.setup_root.trim().is_empty(), egui::Button::new("Install local Kokoro")).clicked() {
                let python = (!self.setup_python.trim().is_empty()).then(|| self.setup_python.trim().into());
                self.submit_kokoro(kokoro_setup::Task::Setup { python, root: root.clone(), installer, manifest, allow_downloads: self.setup_downloads });
            }
            if ui.add_enabled(!self.setup_root.trim().is_empty(), egui::Button::new("Connect existing local installation")).clicked() {
                self.submit_kokoro(kokoro_setup::Task::ConnectExisting { root: root.clone() });
            }
            let previous_voice = self.setup_voice.clone();
            ui.horizontal(|ui| {
                ui.label("English test voice");
                egui::ComboBox::from_id_salt("kokoro-test-voice").selected_text(&self.setup_voice).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.setup_voice, "af_heart".into(), "af_heart");
                    ui.selectable_value(&mut self.setup_voice, "am_michael".into(), "am_michael");
                });
            });
            if self.setup_voice != previous_voice { self.test_audio = None; self.stop_playback(); }
            if ui.add_enabled(self.setup_profile.is_some(), egui::Button::new("Test English CPU speech")).clicked() {
                if let Some(profile) = self.setup_profile.clone() {
                    self.test_audio = None;
                    let output_root = profile.cache.parent().unwrap_or(&profile.cache).join("speech-tests");
                    self.submit_kokoro(kokoro_setup::Task::Test { profile, smoke_script: Path::new(&self.runtime).with_file_name("smoke_macos.py"), voice_preset: self.setup_voice.clone(), output_root });
                }
            }
            if ui.add_enabled(self.test_audio.is_some(), egui::Button::new("Play speech test")).clicked() {
                self.stop_playback();
                if let Some(audio) = self.test_audio.clone() {
                    match playback::Playback::new().and_then(|player| { player.play(audio, 0)?; Ok(player) }) {
                        Ok(player) => self.test_player = Some(player),
                        Err(error) => self.setup_status = error,
                    }
                }
            }
        });
        if self.setup_busy {
            ui.spinner();
            if ui.button("Cancel Kokoro operation").clicked() {
                self.kokoro_setup.cancel();
                self.setup_status = "Cancelling…".into();
            }
        }
        if self.test_player.is_some() && ui.button("Stop speech test").clicked() {
            self.stop_playback();
        }
        if !self.setup_status.is_empty() {
            ui.label(&self.setup_status);
        }
        ui.separator();
    }
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String) -> bool {
    ui.label(label);
    ui.text_edit_singleline(value).changed()
}
impl Studio {
    fn selected_alignment_edited(&mut self) {
        self.stop_playback();
        self.preview = None;
        self.status =
            "Selected-line timing changed; generate/load dialogue again to prepare timing.".into();
    }
    fn runtime_ui(&mut self, ui: &mut egui::Ui) {
        self.kokoro_setup_ui(ui);
        ui.add_enabled_ui(!self.busy && !self.diagnostics_busy && !self.export_busy, |ui| {
                    let mut runtime_changed = false;
                    ui.heading("Advanced local runtime settings");
                    ui.label("Supply existing local assets, or use the explicit installation flow above.");
                    runtime_changed |= field(ui, "Python executable", &mut self.python);
                    runtime_changed |= field(ui, "Sidecar script", &mut self.runtime);
                    runtime_changed |= field(ui, "Model config JSON", &mut self.config);
                    runtime_changed |= field(ui, "Model weights", &mut self.weights);
                    runtime_changed |= field(ui, "Voice .pt directory", &mut self.voices);
                    runtime_changed |= field(ui, "Cache directory", &mut self.cache);
                    runtime_changed |= field(ui, "Immutable model + dependency revision", &mut self.revision);
                    if ui.button("Check local Kokoro setup").clicked() { self.check_runtime(); }
                    ui.label("Preflight checks local packages and files only; it does not run inference or prove compatibility.");
                    if runtime_changed { self.sequence_changed(); self.stop_playback(); self.scene_preview = None; self.scene_audio_active = false; self.preview = None; self.setup_profile = None; self.test_audio = None; }
                    if field(ui, "Project root (imported WAV paths)", &mut self.project_root) {
                        self.pending_text_edit = None; self.history.reset(&self.source);
                        self.stop_playback(); self.invalidate_scene(); self.jobs.clear(); self.preview = None;
                        self.status = "Project root changed; validate screenplay to resolve imported paths again.".into();
                    }
                    ui.separator();
                    ui.heading("Scene alignments JSON");
                    ui.label("Map dialogue IDs to bound timing envelopes or legacy Alignment objects, for example {\"0\": {...}}. Legacy timing is unbound. Blank keeps mouths at rest; prepare again after editing.");
                    if ui.add(egui::TextEdit::multiline(&mut self.scene_alignment).desired_rows(5).char_limit(100_000).code_editor()).changed() { self.sequence_changed(); self.stop_playback(); self.scene_preview = None; self.scene_audio_active = false; }
                    for job in self.jobs.iter().filter(|job| self.script.as_ref().and_then(|s| s.scenes.get(self.scene_index)).is_some_and(|s| s.id == job.scene)) { ui.label(format!("ID {} · {} · {}", job.id, job.actor, job.request.text)); }
                    ui.heading("Selected-line alignment JSON");
                    ui.label("Paste a bound timing envelope (schema_version, key, optional alignment) or a legacy Alignment object (unbound). Blank keeps mouth at rest. Provenance checks do not measure timing quality.");
                    if ui.add(egui::TextEdit::multiline(&mut self.alignment).desired_rows(7).char_limit(100_000).code_editor()).changed() { self.selected_alignment_edited(); }
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
                    self.sequence_panel.stop_transport();
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
                    self.sequence_panel.stop_transport();
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
            ui.label(alignment_status_text(preview.alignment_status));
            if preview.clip.alignment.is_none() && preview.clip.estimated_alignment.is_none() {
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
                            .on_hover_text("Undo project edit · Cmd/Ctrl+Z")
                            .clicked()
                        {
                            self.undo_edit(false);
                        }
                        if ui
                            .add_enabled(
                                !pending_change && self.history.can_redo(),
                                egui::Button::new("Redo"),
                            )
                            .on_hover_text("Redo project edit · Cmd/Ctrl+Shift+Z")
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

    fn start_asset(&mut self, operation: asset_worker::Operation) {
        if self.busy || self.export_busy || self.diagnostics_busy {
            return;
        }
        if Path::new(&self.script_path)
            .extension()
            .is_none_or(|e| e != "kilsat")
            || self.project_document.is_none()
        {
            self.status = "Save this project as .kilsat before managing local assets.".into();
            return;
        }
        let parent = Path::new(&self.script_path)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let roots_match = std::fs::canonicalize(parent)
            .ok()
            .zip(std::fs::canonicalize(&self.project_root).ok())
            .is_some_and(|(root, configured)| root == configured);
        if !roots_match {
            self.status = "Asset operations require the .kilsat path beside the configured project root. Open or save the intended project first.".into();
            return;
        }
        self.flush_text_edit();
        let mut document = self.project_document.clone().unwrap();
        document.source = self.source.clone();
        if let Err(error) = document.validate() {
            self.status = error.to_string();
            return;
        }
        let task = asset_worker::Task {
            project_path: self.script_path.clone().into(),
            document: document.clone(),
            operation,
        };
        match self.asset_worker.start(task) {
            Ok(()) => {
                self.asset_pending =
                    Some((self.source.clone(), self.script_path.clone(), document));
                self.busy = true;
                self.status = "Updating local assets on background worker…".into();
            }
            Err(error) => self.status = error,
        }
    }
    fn assets_ui(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Project asset inventory").show(ui, |ui| {
            if let Some(document) = &self.project_document {
                for asset in &document.assets { ui.label(format!("{} · {:?} · {}", asset.id, asset.kind, asset.path)); }
            }
            if self.asset_dirty { ui.label("Unsaved inventory changes"); }
            ui.weak("Import copies a local file into the project. Relink uses a project-relative path. Remove only forgets the reference. Place imported PNG/JPEG images using Artwork / layers in Inspector.");
            ui.add_enabled_ui(!self.busy && !self.export_busy && !self.diagnostics_busy, |ui| {
                field(ui, "Asset ID", &mut self.asset_id);
                field(ui, "Import local path / relink project-relative path", &mut self.asset_path);
                ui.horizontal(|ui| { ui.selectable_value(&mut self.asset_kind, 0, "Image"); ui.selectable_value(&mut self.asset_kind, 1, "Audio"); ui.selectable_value(&mut self.asset_kind, 2, "Other"); });
                if ui.button("Import local file").clicked() {
                    let kind = match self.asset_kind { 0 => motion_project::AssetKind::Image, 1 => motion_project::AssetKind::Audio, _ => motion_project::AssetKind::Other };
                    self.start_asset(asset_worker::Operation::Import { source: self.asset_path.clone().into(), id: self.asset_id.clone(), kind });
                }
                if ui.button("Relink reference").clicked() { self.start_asset(asset_worker::Operation::Relink { id: self.asset_id.clone(), path: self.asset_path.clone() }); }
                if ui.button("Remove reference (keeps file)").clicked() { self.start_asset(asset_worker::Operation::Remove { id: self.asset_id.clone() }); }
                ui.weak("Undo / Redo in the toolbar also restores inventory references. Imported physical files stay on disk.");
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
            self.assets_ui(ui);
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

    fn art_inspector_ui(&mut self, ui: &mut egui::Ui) {
        let Some(script) = &self.script else {
            return;
        };
        let Some(scene) = script.scenes.get(self.scene_index) else {
            return;
        };
        if self.project_document.is_none() {
            self.project_document = motion_project::ProjectDocument::new(self.source.clone()).ok();
        }
        if let Some(document) = &mut self.project_document {
            self.art_editor
                .clear_history_for_context(format!("{}:{}", self.document_revision, scene.id));
            if ui.button("Reload images from disk").clicked() {
                self.art_resources.reload();
                if self.sequence_panel.busy() {
                    self.busy = false;
                }
                self.sequence_revision = self.sequence_revision.wrapping_add(1);
                self.sequence_panel
                    .invalidate(self.sequence_revision.to_string());
            }
            ui.add_enabled_ui(
                !self.busy && !self.diagnostics_busy && !self.export_busy,
                |ui| {
                    self.art_editor.ui(
                        ui,
                        &mut document.art,
                        art_editor::SceneContext {
                            id: &scene.id,
                            duration: scene.duration,
                            width: script.project.width,
                            height: script.project.height,
                        },
                        &document.assets,
                        &script.characters.keys().cloned().collect::<Vec<_>>(),
                        &self.art_resources.textures,
                    )
                },
            );
            if let Some(error) = &self.art_resources.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
        }
        self.record_art_transaction();
    }
    fn keyframe_inspector_ui(&mut self, ui: &mut egui::Ui) {
        let Some(script) = &self.script else {
            return;
        };
        let Some(scene) = script.scenes.get(self.scene_index) else {
            return;
        };
        let Some(actor) = self
            .selected_actor
            .as_deref()
            .or_else(|| script.characters.keys().next().map(String::as_str))
        else {
            return;
        };
        if !self.keyframe_draft.as_ref().is_some_and(|(source, d)| {
            source == &self.source && d.scene_index == self.scene_index && d.actor == actor
        }) {
            self.keyframe_draft = Some((
                self.source.clone(),
                keyframe_editor::Draft::new(scene, self.scene_index, actor),
            ));
        }
        let mut apply = false;
        ui.add_enabled_ui(
            !self.busy && !self.diagnostics_busy && !self.export_busy,
            |ui| {
                if let Some((_, draft)) = &mut self.keyframe_draft {
                    egui::CollapsingHeader::new("Actor transform keyframes").show(ui, |ui| {
                        apply = draft.ui(ui);
                    });
                }
            },
        );
        if apply {
            if let Some((source, draft)) = &self.keyframe_draft {
                if source == &self.source {
                    let result = motion_script::apply_actor_track(
                        &self.source,
                        draft.scene_index,
                        &draft.actor,
                        &draft.keys,
                    );
                    self.apply_structure(result);
                }
            }
        }
    }
    fn apply_bone_draft(&mut self) {
        let Some((source, draft)) = &self.bone_draft else {
            return;
        };
        let current_actor = self.selected_actor.as_deref().or_else(|| {
            self.script
                .as_ref()?
                .characters
                .keys()
                .next()
                .map(String::as_str)
        });
        if source != &self.source
            || draft.scene_index != self.scene_index
            || current_actor != Some(draft.actor.as_str())
            || draft.bone != self.selected_bone
        {
            self.status = "Bone draft belongs to an earlier source, scene, actor or bone; reload the draft before applying.".into();
            self.bone_draft = None;
            self.joint_draft = None;
            return;
        }
        let result = draft.apply(&self.source);
        self.apply_structure(result);
    }

    fn apply_joint_draft(&mut self) {
        if self.busy || self.diagnostics_busy || self.export_busy || self.setup_busy {
            return;
        }
        let Some((source, draft)) = &self.joint_draft else {
            return;
        };
        let current_actor = self.selected_actor.as_deref().or_else(|| {
            self.script
                .as_ref()?
                .characters
                .keys()
                .next()
                .map(String::as_str)
        });
        if source != &self.source
            || draft.scene_index != self.scene_index
            || current_actor != Some(draft.actor.as_str())
            || draft.bone != self.selected_joint
        {
            self.status = "Joint draft belongs to an earlier source, scene, actor or joint; reload the draft before applying.".into();
            self.joint_draft = None;
            return;
        }
        let result = draft.apply(&self.source);
        self.apply_structure(result);
    }

    fn joint_inspector_ui(&mut self, ui: &mut egui::Ui) {
        let Some(script) = &self.script else {
            return;
        };
        let Some(scene) = script.scenes.get(self.scene_index) else {
            return;
        };
        let Some(actor) = self
            .selected_actor
            .as_deref()
            .or_else(|| script.characters.keys().next().map(String::as_str))
        else {
            return;
        };
        let Some(rig) = script.characters.get(actor).and_then(|c| c.rig.as_ref()) else {
            return;
        };
        let joints: Vec<String> = rig.joints.iter().map(|j| j.id.clone()).collect();
        if !joints.contains(&self.selected_joint) {
            self.selected_joint = joints[0].clone();
        }
        let mut apply = false;
        ui.add_enabled_ui(
            !self.busy && !self.diagnostics_busy && !self.export_busy && !self.setup_busy,
            |ui| {
                egui::CollapsingHeader::new("Custom rig joint keyframes").show(ui, |ui| {
                    egui::ComboBox::from_id_salt("authored_joint")
                        .selected_text(&self.selected_joint)
                        .show_ui(ui, |ui| {
                            for joint in &joints {
                                ui.selectable_value(&mut self.selected_joint, joint.clone(), joint);
                            }
                        });
                    if !self.joint_draft.as_ref().is_some_and(|(source, draft)| {
                        source == &self.source
                            && draft.scene_index == self.scene_index
                            && draft.actor == actor
                            && draft.bone == self.selected_joint
                    }) {
                        self.joint_draft = Some((
                            self.source.clone(),
                            bone_editor::Draft::for_joint(
                                scene,
                                self.scene_index,
                                actor,
                                &self.selected_joint,
                            ),
                        ));
                    }
                    ui.small("Changing scene, actor or joint reloads saved keys and discards unapplied changes.");
                    if let Some((_, draft)) = &mut self.joint_draft {
                        apply = draft.ui(ui);
                    }
                });
            },
        );
        if apply {
            self.apply_joint_draft();
        }
    }

    fn apply_clip_draft(&mut self) {
        if self.busy || self.diagnostics_busy || self.export_busy || self.setup_busy {
            return;
        }
        let Some((source, draft)) = &self.clip_draft else {
            return;
        };
        let current_actor = self.selected_actor.as_deref().or_else(|| {
            self.script
                .as_ref()
                .and_then(|script| script.characters.keys().next().map(String::as_str))
        });
        if source != &self.source
            || draft.scene_index != self.scene_index
            || current_actor != Some(draft.actor.as_str())
        {
            self.status =
                "Clip draft belongs to an earlier source, scene or actor; reload before applying."
                    .into();
            self.clip_draft = None;
            self.clip_track_draft = None;
            self.clip_library_draft = None;
            self.live_clip_draft = None;
            return;
        }
        let result = draft.apply(&self.source);
        self.apply_structure(result);
    }

    fn apply_clip_track_draft(&mut self) {
        if self.busy || self.diagnostics_busy || self.export_busy || self.setup_busy {
            return;
        }
        let Some((source, draft)) = &self.clip_track_draft else {
            return;
        };
        let current_actor = self.selected_actor.as_deref().or_else(|| {
            self.script
                .as_ref()
                .and_then(|script| script.characters.keys().next().map(String::as_str))
        });
        if source != &self.source
            || draft.scene_index != self.scene_index
            || current_actor != Some(draft.actor.as_str())
        {
            self.status =
                "Animated strengths draft belongs to an earlier source, scene or actor; reload before applying."
                    .into();
            self.clip_track_draft = None;
            self.clip_library_draft = None;
            self.live_clip_draft = None;
            return;
        }
        let result = draft.apply(&self.source);
        self.apply_structure(result);
    }

    fn clip_inspector_ui(&mut self, ui: &mut egui::Ui) {
        let Some(script) = &self.script else {
            return;
        };
        let Some(scene) = script.scenes.get(self.scene_index) else {
            return;
        };
        let Some(actor) = self
            .selected_actor
            .as_deref()
            .or_else(|| script.characters.keys().next().map(String::as_str))
        else {
            return;
        };
        if !self.clip_draft.as_ref().is_some_and(|(source, draft)| {
            source == &self.source && draft.scene_index == self.scene_index && draft.actor == actor
        }) {
            self.clip_draft = Some((
                self.source.clone(),
                clip_editor::Draft::new(scene, self.scene_index, actor),
            ));
        }
        if !self
            .clip_track_draft
            .as_ref()
            .is_some_and(|(source, draft)| {
                source == &self.source
                    && draft.scene_index == self.scene_index
                    && draft.actor == actor
            })
        {
            self.clip_track_draft = Some((
                self.source.clone(),
                clip_editor::TrackDraft::new(scene, self.scene_index, actor),
            ));
        }
        let mut apply = false;
        let mut apply_track = false;
        ui.add_enabled_ui(!self.busy && !self.diagnostics_busy && !self.export_busy && !self.setup_busy, |ui| {
            egui::CollapsingHeader::new("Animation layer strengths").show(ui, |ui| {
                ui.small("Changing scene or actor reloads saved strengths and discards unapplied changes.");
                if let Some((_, draft)) = &mut self.clip_draft { apply = draft.ui(ui); }
            });
            egui::CollapsingHeader::new("Animated layer strengths").show(ui, |ui| {
                ui.small("Changing scene or actor discards unapplied keys.");
                if let Some((_, draft)) = &mut self.clip_track_draft {
                    apply_track = draft.ui(ui);
                }
            });
        });
        if apply {
            self.apply_clip_draft();
        } else if apply_track {
            self.apply_clip_track_draft();
        }
    }

    fn apply_library_action(
        &mut self,
        action: clip_library::Action,
        source: &str,
        scene_index: usize,
        actor: &str,
    ) {
        if self.busy || self.diagnostics_busy || self.export_busy || self.setup_busy {
            return;
        }
        let current_actor = self.selected_actor.as_deref().or_else(|| {
            self.script
                .as_ref()
                .and_then(|script| script.characters.keys().next().map(String::as_str))
        });
        if source != self.source || scene_index != self.scene_index || current_actor != Some(actor)
        {
            self.status = "Clip library belongs to an earlier source, scene or actor; reload before applying.".into();
            self.clip_library_draft = None;
            self.live_clip_draft = None;
            return;
        }
        let result = action.apply(&self.source, scene_index, actor);
        self.apply_structure(result);
        // Library edits never remove the target actor; keep its Inspector context.
        if self.source != source
            && self
                .script
                .as_ref()
                .is_some_and(|script| script.characters.contains_key(actor))
        {
            self.selected_actor = Some(actor.into());
        }
    }

    fn clip_library_ui(&mut self, ui: &mut egui::Ui) {
        let Some(script) = &self.script else {
            return;
        };
        let Some(actor) = self
            .selected_actor
            .as_deref()
            .or_else(|| script.characters.keys().next().map(String::as_str))
        else {
            return;
        };
        if !self
            .clip_library_draft
            .as_ref()
            .is_some_and(|(source, index, selected, _)| {
                source == &self.source && *index == self.scene_index && selected == actor
            })
        {
            self.clip_library_draft = Some((
                self.source.clone(),
                self.scene_index,
                actor.into(),
                clip_library::State::default(),
            ));
        }
        let mut action = None;
        ui.add_enabled_ui(
            !self.busy && !self.diagnostics_busy && !self.export_busy && !self.setup_busy,
            |ui| {
                egui::CollapsingHeader::new("Authored clip library").show(ui, |ui| {
                    if let Some((_, _, _, draft)) = &mut self.clip_library_draft {
                        action = draft.ui(ui, script, self.scene_index, actor);
                    }
                });
            },
        );
        if let Some(action) = action {
            let source = self.source.clone();
            let actor = actor.to_string();
            self.apply_library_action(action, &source, self.scene_index, &actor);
        }
    }

    fn apply_live_clip_action(
        &mut self,
        action: live_clip_editor::Action,
        source: &str,
        scene_index: usize,
        actor: &str,
    ) {
        if self.busy || self.diagnostics_busy || self.export_busy || self.setup_busy {
            return;
        }
        let current_actor = self.selected_actor.as_deref().or_else(|| {
            self.script
                .as_ref()
                .and_then(|script| script.characters.keys().next().map(String::as_str))
        });
        if source != self.source || scene_index != self.scene_index || current_actor != Some(actor)
        {
            self.status = "Live clip draft belongs to an earlier source, scene or actor; reload before applying.".into();
            self.live_clip_draft = None;
            return;
        }
        let existing = |id: &str| {
            self.script
                .as_ref()?
                .scenes
                .get(scene_index)?
                .composition
                .clip_instances
                .iter()
                .find(|instance| instance.id == id)
        };
        let targets_actor = match &action {
            live_clip_editor::Action::Upsert { instance } => {
                instance.actor == actor
                    && existing(&instance.id).is_none_or(|old| old.actor == actor)
            }
            live_clip_editor::Action::Remove { id }
            | live_clip_editor::Action::Reorder { id, .. } => {
                existing(id).is_some_and(|old| old.actor == actor)
            }
        };
        if !targets_actor {
            self.status = "Select the instance's actor before editing this live clip.".into();
            return;
        }
        let result = action.apply(&self.source, scene_index);
        self.apply_structure(result);
        if self.source != source
            && self
                .script
                .as_ref()
                .is_some_and(|script| script.characters.contains_key(actor))
        {
            self.selected_actor = Some(actor.into());
        }
    }

    fn live_clip_ui(&mut self, ui: &mut egui::Ui) {
        let Some(script) = &self.script else {
            return;
        };
        let Some(actor) = self
            .selected_actor
            .as_deref()
            .or_else(|| script.characters.keys().next().map(String::as_str))
        else {
            return;
        };
        if !self
            .live_clip_draft
            .as_ref()
            .is_some_and(|(source, index, selected, _)| {
                source == &self.source && *index == self.scene_index && selected == actor
            })
        {
            self.live_clip_draft = Some((
                self.source.clone(),
                self.scene_index,
                actor.into(),
                live_clip_editor::State::default(),
            ));
        }
        let mut action = None;
        ui.add_enabled_ui(
            !self.busy && !self.diagnostics_busy && !self.export_busy && !self.setup_busy,
            |ui| {
                egui::CollapsingHeader::new("Live authored clips").show(ui, |ui| {
                    if let Some((_, _, _, draft)) = &mut self.live_clip_draft {
                        action = draft.ui(ui, script, self.scene_index, actor);
                    }
                });
            },
        );
        if let Some(action) = action {
            let source = self.source.clone();
            let actor = actor.to_string();
            self.apply_live_clip_action(action, &source, self.scene_index, &actor);
        }
    }

    fn custom_rig_ui(&mut self, ui: &mut egui::Ui) {
        let Some(script) = &self.script else {
            return;
        };
        let Some(actor) = self
            .selected_actor
            .as_deref()
            .or_else(|| script.characters.keys().next().map(String::as_str))
        else {
            return;
        };
        if !self
            .custom_rig_draft
            .as_ref()
            .is_some_and(|(source, draft)| source == &self.source && draft.actor == actor)
        {
            self.custom_rig_draft = Some((
                self.source.clone(),
                custom_rig_editor::Draft::for_actor(script, actor),
            ));
        }
        let mut action = 0;
        ui.add_enabled_ui(
            !self.busy && !self.diagnostics_busy && !self.export_busy && !self.setup_busy,
            |ui| {
                egui::CollapsingHeader::new("Custom character rig").show(ui, |ui| {
                    if let Some((_, draft)) = &mut self.custom_rig_draft {
                        action = draft.ui(ui);
                    }
                });
            },
        );
        if action != 0 {
            self.apply_custom_rig_draft(action);
        }
    }

    fn apply_custom_rig_draft(&mut self, action: u8) {
        if self.busy || self.diagnostics_busy || self.export_busy || self.setup_busy {
            return;
        }
        let Some((source, draft)) = &self.custom_rig_draft else {
            return;
        };
        let selected = self.selected_actor.as_deref().or_else(|| {
            self.script
                .as_ref()
                .and_then(|script| script.characters.keys().next().map(String::as_str))
        });
        if source != &self.source || selected != Some(draft.actor.as_str()) {
            self.status = "Character/source changed; reopen the custom rig draft.".into();
            return;
        }
        let result = match action {
            1 => draft.apply(source),
            2 => draft.remove(source),
            _ => return,
        };
        self.apply_structure(result.map_err(motion_script::Error::Validation));
    }

    fn bone_inspector_ui(&mut self, ui: &mut egui::Ui) {
        let Some(script) = &self.script else {
            return;
        };
        let Some(scene) = script.scenes.get(self.scene_index) else {
            return;
        };
        let Some(actor) = self
            .selected_actor
            .as_deref()
            .or_else(|| script.characters.keys().next().map(String::as_str))
        else {
            return;
        };
        let mut apply = false;
        ui.add_enabled_ui(!self.busy && !self.diagnostics_busy && !self.export_busy, |ui| {
            egui::CollapsingHeader::new("Per-bone keyframes").show(ui, |ui| {
                let old_bone = self.selected_bone.clone();
                egui::ComboBox::from_id_salt("authored_bone")
                    .selected_text(&self.selected_bone).show_ui(ui, |ui| {
                        for bone in motion_script::HUMANOID_BONES {
                            ui.selectable_value(&mut self.selected_bone, bone.into(), bone);
                        }
                    });
                if self.selected_bone != old_bone {
                    ui.small("Bone changed: loaded that bone's saved keys; unapplied draft changes were discarded.");
                }
                if !self.bone_draft.as_ref().is_some_and(|(source, draft)| {
                    source == &self.source && draft.scene_index == self.scene_index
                        && draft.actor == actor && draft.bone == self.selected_bone
                }) {
                    self.bone_draft = Some((self.source.clone(), bone_editor::Draft::for_bone(
                        scene, self.scene_index, actor, &self.selected_bone,
                    )));
                }
                ui.small("Changing scene, actor or bone reloads saved keys and discards unapplied changes.");
                if let Some((_, draft)) = &mut self.bone_draft { apply = draft.ui(ui); }
            });
        });
        if apply {
            self.apply_bone_draft();
        }
    }

    fn sequence_ui(&mut self, ui: &mut egui::Ui) {
        self.sync_sequence_resources();
        let own_busy = self.sequence_panel.busy();
        let locked = (self.busy && !own_busy) || self.diagnostics_busy || self.export_busy;
        if self.sequence_panel.ui(ui, locked, self.script.is_some()) {
            self.stop_other_playback();
            self.busy = true;
        }
        if self.sequence_panel.take_prepare_requested() {
            let request = self
                .script
                .as_ref()
                .map(|script| sequence_worker::PrepareRequest {
                    generation: 0,
                    script: script.clone(),
                    root: self.project_root.clone().into(),
                    config: self.kokoro_config(),
                    alignments: self.scene_alignment.clone(),
                    art: motion_export::ArtContext {
                        document: self
                            .project_document
                            .as_ref()
                            .map(|d| d.art.clone())
                            .unwrap_or_default(),
                        images: (*self.art_resources.images).clone(),
                    },
                    cancel: CancellationToken::new(),
                });
            if let Some(request) = request {
                if self.sequence_panel.start_prepare(request) {
                    self.stop_other_playback();
                    self.busy = true;
                }
            }
        }
        if self.sequence_panel.take_play_started() {
            self.stop_other_playback();
        }
    }
    fn inspector_ui(&mut self, ui: &mut egui::Ui) {
        self.sequence_ui(ui);
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
        self.art_inspector_ui(ui);
        self.keyframe_inspector_ui(ui);
        self.custom_rig_ui(ui);
        self.bone_inspector_ui(ui);
        self.joint_inspector_ui(ui);
        self.clip_inspector_ui(ui);
        self.clip_library_ui(ui);
        self.live_clip_ui(ui);
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
        self.native_alignment_ui(ui);
        egui::CollapsingHeader::new("Metal device check").show(ui, |ui| self.gpu_check.ui(ui));
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
        if script
            .scenes
            .get(self.scene_index)
            .is_none_or(|scene| scene.id != preview.scene)
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
        if let Some(document) = &self.project_document {
            let mut document = document.clone();
            document.source = self.source.clone();
            if let Err(error) = document.validate() {
                self.status = error.to_string();
                return;
            }
        }
        let request = export_worker::Request {
            art: motion_export::ArtContext {
                document: self
                    .project_document
                    .as_ref()
                    .map(|d| d.art.clone())
                    .unwrap_or_default(),
                images: (*self.art_resources.images).clone(),
            },
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
        self.sequence_panel.stop_transport();
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
                        self.sequence_panel.stop_transport();
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
        let was_sequence_busy = self.sequence_panel.busy();
        self.sync_sequence_resources();
        self.sequence_panel.tick(ctx);
        if was_sequence_busy && !self.sequence_panel.busy() {
            self.busy = false;
        }
        self.poll();
        self.sync_native_alignment();
        if self.sequence_panel.busy() {
            self.busy = true;
        }
        if self.busy
            || self.diagnostics_busy
            || self.test_player.is_some()
            || self.playback.is_some()
            || self
                .scene_transport
                .as_ref()
                .is_some_and(|t| t.is_playing())
        {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        self.workspace_ui(ctx);
        self.sync_sequence_resources();
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

/// The direct egui painter cannot implement SVG arbitrary masks/blending.
fn requires_composited_preview(art: &motion_art::Document, scene_id: &str, time: f64) -> bool {
    !art.order.is_empty()
        || art.groups.iter().any(|group| group.scene_id == scene_id)
        || art.layers.iter().any(|layer| {
            layer.scene_id == scene_id
                && layer.is_active(time)
                && (layer.clip_shape.is_some() || layer.blend != motion_art::BlendMode::Normal)
        })
}

fn draw_scene(
    ui: &mut egui::Ui,
    scene: &motion_scene::EvaluatedScene,
    mouths: &[(&str, Viseme)],
    paint_artwork: impl Fn(&egui::Painter, egui::Rect, motion_art::Plane),
) -> egui::Rect {
    let width = ui.available_width().max(1.0);
    let height = (width * scene.height as f32 / scene.width.max(1) as f32)
        .clamp(120.0, (ui.available_height() - 35.0).max(120.0));
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 5.0, egui::Color32::BLACK);
    let background = egui::Color32::from_hex(&scene.background_color)
        .unwrap_or(egui::Color32::from_rgb(24, 31, 44));
    painter.rect_filled(rect, 5.0, background);
    paint_artwork(&painter, rect, motion_art::Plane::Background);
    let scale =
        (rect.width() / scene.width.max(1) as f32).min(rect.height() / scene.height.max(1) as f32);
    let origin = rect.center() - egui::vec2(scene.width as f32, scene.height as f32) * scale * 0.5;
    for actor in &scene.actors {
        let point = |dx: f32, dy: f32| {
            let (x, y) = scene.camera.project(
                actor
                    .local_to_world(motion_scene::Vec2::new(dx as f64, dy as f64))
                    .x,
                actor
                    .local_to_world(motion_scene::Vec2::new(dx as f64, dy as f64))
                    .y,
                scene.width,
                scene.height,
            );
            origin + egui::vec2(x as f32, y as f32) * scale
        };
        let scale = scale * scene.camera.zoom as f32;
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
        let ellipse = |cx: f32, cy: f32, rx: f32, ry: f32, color: egui::Color32| {
            painter.add(egui::Shape::convex_polygon(
                (0..32)
                    .map(|i| {
                        let angle = i as f32 * std::f32::consts::TAU / 32.0;
                        face(cx + rx * angle.cos(), cy + ry * angle.sin())
                    })
                    .collect(),
                color,
                egui::Stroke::NONE,
            ));
        };
        ellipse(0.0, 0.0, 25.0, 25.0, egui::Color32::from_rgb(245, 200, 162));
        for x in [-8.0, 8.0] {
            ellipse(x, -4.0, 2.5, 2.5, egui::Color32::BLACK);
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
        painter.add(egui::Shape::convex_polygon(
            vec![
                face(-w * 0.5, 10.0 - h * 0.5),
                face(w * 0.5, 10.0 - h * 0.5),
                face(w * 0.5, 10.0 + h * 0.5),
                face(-w * 0.5, 10.0 + h * 0.5),
            ],
            egui::Color32::from_rgb(125, 45, 57),
            egui::Stroke::NONE,
        ));
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
            egui::FontId::proportional((12.0 * scene.camera.zoom as f32).max(1.0)),
            egui::Color32::WHITE,
        );
    }
    paint_artwork(&painter, rect, motion_art::Plane::Foreground);
    // Composite the completed scene over black once, matching the exported SVG group.
    painter.rect_filled(
        rect,
        0.0,
        egui::Color32::from_black_alpha(
            ((1.0 - scene.opacity.clamp(0.0, 1.0)) * 255.0).round() as u8
        ),
    );
    rect
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
    #[test]
    fn native_scene_fade_covers_both_artwork_planes_once() {
        let ctx = eframe::egui::Context::default();
        let mut studio = super::Studio::default();
        studio.validate();
        let mut scene = motion_scene::evaluate(studio.script.as_ref().unwrap(), 0, 0.0).unwrap();
        scene.opacity = 0.25;
        let output = ctx.run(Default::default(), |ctx| {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                super::draw_scene(ui, &scene, &[], |painter, rect, plane| {
                    painter.rect_filled(
                        rect,
                        0.0,
                        match plane {
                            motion_art::Plane::Background => eframe::egui::Color32::RED,
                            motion_art::Plane::Foreground => eframe::egui::Color32::GREEN,
                        },
                    );
                });
            });
        });
        let fills: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| {
                if let eframe::egui::Shape::Rect(r) = &s.shape {
                    Some(r.fill)
                } else {
                    None
                }
            })
            .collect();
        let background = fills
            .iter()
            .position(|c| *c == eframe::egui::Color32::RED)
            .unwrap();
        let foreground = fills
            .iter()
            .position(|c| *c == eframe::egui::Color32::GREEN)
            .unwrap();
        assert!(background < foreground);
        assert_eq!(
            *fills.last().unwrap(),
            eframe::egui::Color32::from_black_alpha(191)
        );
    }

    #[test]
    fn artwork_references_guard_scene_rename_and_source_undo() {
        let mut studio = super::Studio::default();
        studio.validate();
        let original = studio.source.clone();
        let scene_id = studio.script.as_ref().unwrap().scenes[0].id.clone();
        let mut document = motion_project::ProjectDocument::new(original.clone()).unwrap();
        document.art.layers.push(motion_art::Layer {
            id: "layer".into(),
            scene_id: scene_id.clone(),
            name: "Art".into(),
            visible: true,
            plane: motion_art::Plane::Foreground,
            active: None,
            opacity: 1.0,
            attachment: None,
            clip_shape: None,
            blend: motion_art::BlendMode::Normal,
            clip: None,
            content: motion_art::Content::Rectangle {
                rect: motion_art::Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 50.0,
                    height: 50.0,
                },
                color: motion_art::Color {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 255,
                },
            },
        });
        studio.project_document = Some(document);
        studio.settings_draft =
            super::scene_settings::Draft::new(studio.script.as_ref().unwrap(), 0);
        studio.settings_draft_source = Some(original.clone());
        studio.settings_draft.as_mut().unwrap().scene_id = "renamed".into();
        studio.apply_settings_draft();
        assert_eq!(studio.source, original);
        assert!(studio.settings_error.as_ref().unwrap().contains("artwork"));
        let alternate = motion_script::apply_scene_settings(
            &original,
            0,
            studio.script.as_ref().unwrap().project.clone(),
            "other",
            studio.script.as_ref().unwrap().scenes[0].duration,
        )
        .unwrap();
        studio.history.reset(&alternate);
        studio.record_source_edit(&alternate, &original);
        studio.undo_edit(false);
        assert_eq!(studio.source, alternate);
        assert!(studio.script.is_none());
        assert!(studio.status.contains("unfinished screenplay"));
        assert!(studio.jobs.is_empty());
        studio.undo_edit(true);
        assert_eq!(studio.source, original);
        assert!(studio.script.is_some());
    }
    #[test]
    fn yaml_save_refuses_to_drop_artwork() {
        let mut studio = super::Studio::default();
        studio.validate();
        let mut document = motion_project::ProjectDocument::new(studio.source.clone()).unwrap();
        document.art.layers.push(motion_art::Layer {
            id: "layer".into(),
            scene_id: studio.script.as_ref().unwrap().scenes[0].id.clone(),
            name: "Art".into(),
            visible: true,
            plane: motion_art::Plane::Foreground,
            active: None,
            opacity: 1.0,
            attachment: None,
            clip_shape: None,
            blend: motion_art::BlendMode::Normal,
            clip: None,
            content: motion_art::Content::Rectangle {
                rect: motion_art::Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
                color: motion_art::Color {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 255,
                },
            },
        });
        studio.project_document = Some(document);
        studio.script_path = "/unwritable/art-loss.yaml".into();
        studio.save_script();
        assert!(studio.status.contains("Artwork requires .kilsat"));
    }
    use super::*;
    #[test]
    fn outgoing_transition_apply_and_undo_restore_exact_source() {
        let mut studio = Studio::default();
        studio.source = "project:\n  title: Sequence\ncharacters: {}\nscenes:\n  - id: first\n    duration: 5.0\n    events: []\n  - id: last\n    duration: 5.0\n    events: []\n".into();
        studio.validate();
        let original = studio.source.clone();
        studio.settings_draft = scene_settings::Draft::new(studio.script.as_ref().unwrap(), 0);
        studio.settings_draft_source = Some(original.clone());
        studio
            .settings_draft
            .as_mut()
            .unwrap()
            .composition
            .transition = motion_script::Transition::Crossfade { duration: 0.5 };
        studio.apply_settings_draft();
        assert!(studio.settings_error.is_none());
        assert_eq!(
            studio.script.as_ref().unwrap().scenes[0]
                .composition
                .transition,
            motion_script::Transition::Crossfade { duration: 0.5 }
        );
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
    }

    #[test]
    fn invalid_final_transition_and_shortened_overlap_preserve_document() {
        for (scene_index, shorten) in [(2, false), (1, true)] {
            let mut studio = Studio::default();
            studio.source = "project:\n  title: Sequence\ncharacters: {}\nscenes:\n  - id: first\n    duration: 5.0\n    composition:\n      transition: { type: crossfade, duration: 1.0 }\n    events: []\n  - id: middle\n    duration: 5.0\n    composition:\n      transition: { type: crossfade, duration: 1.0 }\n    events: []\n  - id: last\n    duration: 5.0\n    events: []\n".into();
            studio.validate();
            assert!(studio.script.is_some());
            studio.scene_index = scene_index;
            let original = studio.source.clone();
            let script_before = serde_json::to_value(studio.script.as_ref().unwrap()).unwrap();
            studio.settings_draft =
                scene_settings::Draft::new(studio.script.as_ref().unwrap(), scene_index);
            studio.settings_draft_source = Some(original.clone());
            let draft = studio.settings_draft.as_mut().unwrap();
            if shorten {
                // Each overlap still fits alone, but their sum exceeds this scene.
                draft.duration_seconds = 1.5;
            } else {
                draft.composition.transition =
                    motion_script::Transition::Crossfade { duration: 0.5 };
            }
            studio.apply_settings_draft();
            assert!(studio.settings_error.is_some());
            assert_eq!(studio.source, original);
            assert_eq!(
                serde_json::to_value(studio.script.as_ref().unwrap()).unwrap(),
                script_before
            );
        }
    }

    #[test]
    fn composition_apply_is_atomic_and_undoable() {
        let mut studio = Studio::default();
        studio.validate();
        let original = studio.source.clone();
        studio.settings_draft = scene_settings::Draft::new(studio.script.as_ref().unwrap(), 0);
        studio.settings_draft_source = Some(original.clone());
        let draft = studio.settings_draft.as_mut().unwrap();
        draft.composition.camera.push(motion_script::CameraKey {
            at: 0.0,
            x: 20.0,
            y: 10.0,
            zoom: 1.5,
        });
        draft.composition.layers.insert(
            "EMMA".into(),
            motion_script::Layer {
                z: 2,
                visible: false,
            },
        );
        draft.composition.background_color = "#123456".into();
        studio.apply_settings_draft();
        assert_ne!(studio.source, original);
        let script = studio.script.as_ref().unwrap();
        assert_eq!(script.scenes[0].composition.camera[0].zoom, 1.5);
        assert!(!script.scenes[0].composition.layers["EMMA"].visible);
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        studio.settings_draft = scene_settings::Draft::new(studio.script.as_ref().unwrap(), 0);
        studio.settings_draft_source = Some(original.clone());
        studio
            .settings_draft
            .as_mut()
            .unwrap()
            .composition
            .camera
            .push(motion_script::CameraKey {
                at: 0.0,
                x: 0.0,
                y: 0.0,
                zoom: -1.0,
            });
        studio.apply_settings_draft();
        assert_eq!(studio.source, original);
        assert!(studio.settings_error.is_some());
    }
    #[test]
    fn timed_background_apply_undo_and_invalid_drafts_preserve_source() {
        let mut studio = Studio::default();
        studio.validate();
        let original = studio.source.clone();
        studio.settings_draft = scene_settings::Draft::new(studio.script.as_ref().unwrap(), 0);
        studio.settings_draft_source = Some(original.clone());
        let duration = studio.script.as_ref().unwrap().scenes[0].duration;
        studio
            .settings_draft
            .as_mut()
            .unwrap()
            .composition
            .background_keys = vec![
            motion_script::BackgroundKey {
                at: 0.0,
                color: "#102030".into(),
            },
            motion_script::BackgroundKey {
                at: duration,
                color: "#ABCDEF".into(),
            },
        ];
        studio.apply_settings_draft();
        assert_ne!(studio.source, original);
        let keys = &studio.script.as_ref().unwrap().scenes[0]
            .composition
            .background_keys;
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[1].at, duration);
        assert_eq!(keys[1].color, "#ABCDEF");
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        // Each rejected draft is evaluated against the original, including when
        // other valid settings would otherwise be committed by the same Apply.
        let invalid = [
            vec![(0.0, "red")],
            vec![(-1.0, "#123456")],
            vec![(duration + 1.0, "#123456")],
            vec![(1.0, "#123456"), (1.0, "#654321")],
            vec![(2.0, "#123456"), (1.0, "#654321")],
            vec![(f64::NAN, "#123456")],
        ];
        for keys in invalid {
            studio.settings_draft = scene_settings::Draft::new(studio.script.as_ref().unwrap(), 0);
            studio.settings_draft_source = Some(original.clone());
            let draft = studio.settings_draft.as_mut().unwrap();
            draft.project.title = "Must not be committed".into();
            draft.composition.background_keys = keys
                .into_iter()
                .map(|(at, color)| motion_script::BackgroundKey {
                    at,
                    color: color.into(),
                })
                .collect();
            studio.apply_settings_draft();
            assert_eq!(studio.source, original);
            assert!(studio.settings_error.is_some());
        }
    }
    #[test]
    fn background_and_duration_changes_apply_in_one_validated_transaction() {
        for remove_key in [false, true] {
            let mut studio = Studio::default();
            studio.source = "project:\n  title: Timed background\ncharacters:\n  EMMA:\n    asset: characters/emma\nscenes:\n  - id: first\n    duration: 5.0\n    composition:\n      background_keys:\n        - at: 4.0\n          color: '#123456'\n    events: []\n".into();
            studio.validate();
            let original = studio.source.clone();
            studio.settings_draft = scene_settings::Draft::new(studio.script.as_ref().unwrap(), 0);
            studio.settings_draft_source = Some(original.clone());
            let draft = studio.settings_draft.as_mut().unwrap();
            draft.duration_seconds = 2.0;
            if remove_key {
                draft.composition.background_keys.clear();
            } else {
                draft.composition.background_keys[0].at = 2.0;
            }
            studio.apply_settings_draft();
            assert!(
                studio.settings_error.is_none(),
                "{:?}",
                studio.settings_error
            );
            let scene = &studio.script.as_ref().unwrap().scenes[0];
            assert_eq!(scene.duration, 2.0);
            assert_eq!(
                scene.composition.background_keys.len(),
                usize::from(!remove_key)
            );
            studio.undo_edit(false);
            assert_eq!(studio.source, original);
            studio.settings_draft = scene_settings::Draft::new(studio.script.as_ref().unwrap(), 0);
            studio.settings_draft_source = Some(original.clone());
            let draft = studio.settings_draft.as_mut().unwrap();
            draft.duration_seconds = 8.0;
            draft
                .composition
                .background_keys
                .push(motion_script::BackgroundKey {
                    at: 8.0,
                    color: "#ABCDEF".into(),
                });
            studio.apply_settings_draft();
            assert!(
                studio.settings_error.is_none(),
                "{:?}",
                studio.settings_error
            );
            let scene = &studio.script.as_ref().unwrap().scenes[0];
            assert_eq!(scene.duration, 8.0);
            assert_eq!(scene.composition.background_keys[1].at, 8.0);
            studio.undo_edit(false);
            assert_eq!(studio.source, original);
        }
    }
    #[test]
    fn asset_worker_remove_keeps_source_and_physical_file() {
        let mut studio = Studio::default();
        studio.validate();
        let root = std::env::temp_dir().join(format!("studio-asset-worker-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("image.dat");
        std::fs::write(&file, b"retained").unwrap();
        studio.project_root = root.to_string_lossy().into();
        studio.script_path = root.join("project.kilsat").to_string_lossy().into();
        let mut doc = motion_project::ProjectDocument::new(studio.source.clone()).unwrap();
        doc.assets.push(motion_project::AssetReference {
            id: "image".into(),
            path: "image.dat".into(),
            kind: motion_project::AssetKind::Other,
        });
        studio.project_document = Some(doc);
        let original = studio.source.clone();
        studio.start_asset(asset_worker::Operation::Remove { id: "image".into() });
        assert!(studio.busy);
        let deadline = Instant::now() + Duration::from_secs(5);
        while studio.busy && Instant::now() < deadline {
            studio.poll();
            std::thread::yield_now();
        }
        assert!(!studio.busy);
        assert!(studio.asset_dirty);
        assert!(studio.project_document.as_ref().unwrap().assets.is_empty());
        assert_eq!(studio.source, original);
        assert_eq!(std::fs::read(file).unwrap(), b"retained");
        studio.undo_edit(false);
        assert_eq!(studio.project_document.as_ref().unwrap().assets.len(), 1);
        studio.asset_dirty = false;
        studio.start_asset(asset_worker::Operation::Remove { id: "image".into() });
        assert!(studio.busy);
        studio.script_path = root.join("different.kilsat").to_string_lossy().into();
        let deadline = Instant::now() + Duration::from_secs(5);
        while studio.busy && Instant::now() < deadline {
            studio.poll();
            std::thread::yield_now();
        }
        assert!(!studio.busy);
        assert_eq!(studio.project_document.as_ref().unwrap().assets.len(), 1);
        assert!(studio.history.can_redo());
        assert!(!studio.asset_dirty);
        assert!(studio.status.contains("discarded"));
        studio.project_root = root.join("missing-root").to_string_lossy().into();
        studio.start_asset(asset_worker::Operation::Import {
            source: root.join("image.dat"),
            id: "new".into(),
            kind: motion_project::AssetKind::Other,
        });
        assert!(!root.join("resources").exists());
        assert!(!studio.busy);
        assert!(studio.status.contains("configured project root"));
        std::fs::remove_dir_all(root).unwrap();
    }
    fn clip(start_frame: u64, actor: &str) -> DialogueClip {
        DialogueClip {
            actor: actor.into(),
            start_frame,
            audio: motion_audio::AudioBuffer {
                sample_rate: 24000,
                channels: 1,
                samples: vec![0.0; 24000],
            },
            estimated_alignment: None,
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
    fn pending_text_then_canvas_artwork_undo_in_one_project_order() {
        let mut studio = settings_studio();
        let original = studio.source.clone();
        studio.project_document =
            Some(motion_project::ProjectDocument::new(original.clone()).unwrap());
        let typed = format!("# typed before drawing\n{original}");
        studio.source = typed.clone();
        studio.pending_text_edit = Some(original.clone());
        let document = studio.project_document.as_mut().unwrap();
        let mut candidate = document.art.clone();
        candidate.layers.push(motion_art::Layer {
            id: "drawn".into(),
            name: "Drawn".into(),
            scene_id: studio.script.as_ref().unwrap().scenes[0].id.clone(),
            visible: true,
            plane: motion_art::Plane::Foreground,
            active: None,
            opacity: 1.0,
            attachment: None,
            clip: None,
            clip_shape: None,
            blend: motion_art::BlendMode::Normal,
            content: motion_art::Content::Rectangle {
                rect: motion_art::Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 20.0,
                    height: 10.0,
                },
                color: motion_art::Color {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 255,
                },
            },
        });
        studio.art_editor.commit(&mut document.art, candidate);
        studio.record_art_transaction();
        assert!(studio.pending_text_edit.is_none());
        assert_eq!(
            studio.project_document.as_ref().unwrap().art.layers.len(),
            1
        );
        studio.undo_edit(false);
        assert_eq!(studio.source, typed);
        assert!(studio
            .project_document
            .as_ref()
            .unwrap()
            .art
            .layers
            .is_empty());
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        assert!(studio
            .project_document
            .as_ref()
            .unwrap()
            .art
            .layers
            .is_empty());
        studio.undo_edit(true);
        studio.undo_edit(true);
        assert_eq!(studio.source, typed);
        assert_eq!(
            studio.project_document.as_ref().unwrap().art.layers.len(),
            1
        );
        let revision = studio.sequence_revision;
        let document = studio.project_document.as_mut().unwrap();
        let mut invalid = document.art.clone();
        invalid.layers[0].opacity = f64::NAN;
        studio.art_editor.commit(&mut document.art, invalid);
        studio.record_art_transaction();
        assert_eq!(studio.sequence_revision, revision);
        assert!(!studio.history.can_redo());
        studio.undo_edit(false);
        assert!(studio
            .project_document
            .as_ref()
            .unwrap()
            .art
            .layers
            .is_empty());
    }

    #[test]
    fn sequence_revisions_are_idle_stable_and_follow_document_history() {
        let mut studio = settings_studio();
        studio.sync_sequence_resources();
        let initial = studio.sequence_revision;
        let document_initial = studio.document_revision;
        for _ in 0..100 {
            studio.sync_sequence_resources();
        }
        assert_eq!(studio.sequence_revision, initial);
        studio.validate();
        assert_eq!(studio.sequence_revision, initial);
        studio.settings_draft =
            scene_settings::Draft::new(studio.script.as_ref().unwrap(), studio.scene_index);
        studio.settings_draft_source = Some(studio.source.clone());
        studio.settings_draft.as_mut().unwrap().project.title = "Revision edit".into();
        studio.apply_settings_draft();
        assert!(studio.sequence_revision > initial);
        assert!(studio.document_revision > document_initial);
        let edited = studio.sequence_revision;
        studio.undo_edit(false);
        assert!(studio.sequence_revision > edited);
        let undone = studio.sequence_revision;
        studio.undo_edit(true);
        assert!(studio.sequence_revision > undone);
        let before_invalidation = studio.sequence_revision;
        studio.invalidate_scene(); // Includes unvalidated text and imported-root changes.
        assert!(studio.sequence_revision > before_invalidation);
    }

    #[test]
    fn sequence_resources_track_immutable_replacement_and_art_reset_without_scanning() {
        let mut studio = Studio::default();
        studio.sync_sequence_resources();
        let original = studio.sequence_revision;
        // Even an identically empty replacement is a new decode/reload generation.
        studio.art_resources.images = Arc::new(Default::default());
        studio.sync_sequence_resources();
        assert_eq!(studio.sequence_revision, original + 1);
        let replacement = studio.sequence_revision;
        studio.sync_sequence_resources();
        assert_eq!(studio.sequence_revision, replacement);
        studio.art_editor.reset();
        studio.sync_sequence_resources();
        assert_eq!(studio.sequence_revision, replacement + 1);
        studio.sync_sequence_resources();
        assert_eq!(studio.sequence_revision, replacement + 1);
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
    fn kokoro_setup_respects_active_document_operations() {
        for guard in 0..3 {
            let mut studio = Studio::default();
            studio.busy = guard == 0;
            studio.diagnostics_busy = guard == 1;
            studio.export_busy = guard == 2;
            studio.setup_status = "unchanged".into();
            studio.submit_kokoro(kokoro_setup::Task::ConnectExisting {
                root: "/not-read-because-busy".into(),
            });
            assert!(!studio.setup_busy);
            assert_eq!(studio.setup_status, "unchanged");
        }
    }

    #[test]
    fn kokoro_setup_panel_renders_without_running_tasks() {
        let ctx = egui::Context::default();
        let mut studio = Studio::default();
        let source = studio.source.clone();
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| studio.runtime_ui(ui));
        });
        assert!(!output.shapes.is_empty());
        assert_eq!(studio.source, source);
        assert!(!studio.setup_busy);
        assert!(!studio.setup_downloads);
        assert!(studio.setup_profile.is_none());
    }

    #[test]
    fn connecting_kokoro_profile_preserves_document_and_dialogue() {
        let mut studio = Studio::default();
        studio.validate();
        let source = studio.source.clone();
        let jobs = studio.jobs.len();
        studio.alignment = "manual line timing".into();
        studio.scene_alignment = "manual scene timing".into();
        assert!(!studio.setup_downloads);
        let profile = kokoro_setup::Profile {
            python: "/local/python".into(),
            config: "/local/config.json".into(),
            model: "/local/model.pth".into(),
            voices: "/local/voices".into(),
            cache: "/local/cache".into(),
            revision: "immutable-test-revision".into(),
        };
        studio.apply_kokoro_profile(profile);
        assert_eq!(studio.python, "/local/python");
        assert_eq!(studio.weights, "/local/model.pth");
        assert_eq!(studio.revision, "immutable-test-revision");
        assert_eq!(studio.source, source);
        assert_eq!(studio.jobs.len(), jobs);
        assert_eq!(studio.alignment, "manual line timing");
        assert_eq!(studio.scene_alignment, "manual scene timing");
        assert!(studio.setup_profile.is_some());
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

#[cfg(test)]
mod alignment_status_tests {
    use super::*;
    #[test]
    fn editing_or_clearing_line_timing_discards_prepared_audio_and_status() {
        let mut studio = Studio::default();
        for changed in ["new bound envelope", ""] {
            let audio = motion_audio::AudioBuffer {
                sample_rate: 24000,
                channels: 1,
                samples: vec![0.0; 24],
            };
            studio.preview = Some(Preview {
                scene: "scene".into(),
                line_id: 0,
                transcript: "test".into(),
                alignment_status: AlignmentStatus::BoundCacheHit,
                clip: DialogueClip {
                    actor: "actor".into(),
                    start_frame: 0,
                    audio: audio.clone(),
                    estimated_alignment: None,
                    alignment: None,
                    gain: 1.0,
                    trim_start_frame: 0,
                },
                peaks: vec![],
                result: SpeechResult {
                    audio_path: "unused.wav".into(),
                    sample_rate: 24000,
                    sample_count: 24,
                    cache_key: "fixture".into(),
                    phonemes: String::new(),
                    cached: true,
                },
                playback_audio: Arc::new(audio),
            });
            studio.alignment = changed.into();
            studio.selected_alignment_edited();
            assert!(studio.preview.is_none());
            assert_eq!(studio.alignment, changed);
            assert!(studio.status.contains("timing changed"));
        }
    }
    #[test]
    fn timing_status_distinguishes_unbound_cache_and_rest() {
        assert!(alignment_status_text(AlignmentStatus::LegacyUnbound).contains("unbound"));
        assert!(alignment_status_text(AlignmentStatus::BoundCacheHit).contains("cache hit"));
        assert!(alignment_status_text(AlignmentStatus::BoundCacheMiss).contains("mouth at rest"));
        assert!(alignment_status_text(AlignmentStatus::None).contains("mouth at rest"));
        assert!(
            alignment_status_text(AlignmentStatus::BoundImported).contains("quality not measured")
        );
    }
}

#[cfg(test)]
mod bone_and_compositor_tests {
    use super::*;
    fn studio_with_draft() -> Studio {
        let mut studio = Studio::default();
        studio.validate();
        let script = studio.script.as_ref().unwrap();
        let actor = script.characters.keys().next().unwrap().clone();
        studio.selected_actor = Some(actor.clone());
        studio.selected_bone = "chest".into();
        let mut draft = bone_editor::Draft::for_bone(&script.scenes[0], 0, &actor, "chest");
        draft.keys.push(motion_script::ActorKey {
            rotation: 0.25,
            ..Default::default()
        });
        studio.bone_draft = Some((studio.source.clone(), draft));
        studio
    }
    fn layer(scene: &str) -> motion_art::Layer {
        motion_art::Layer {
            id: "test".into(),
            name: "Test".into(),
            scene_id: scene.into(),
            visible: true,
            plane: motion_art::Plane::Foreground,
            active: None,
            opacity: 1.0,
            attachment: None,
            clip: None,
            clip_shape: None,
            blend: motion_art::BlendMode::Normal,
            content: motion_art::Content::Rectangle {
                rect: motion_art::Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 20.0,
                    height: 10.0,
                },
                color: motion_art::Color {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 255,
                },
            },
        }
    }
    #[test]
    fn bone_apply_and_artwork_undo_share_chronology() {
        let mut studio = studio_with_draft();
        let original = studio.source.clone();
        studio.apply_bone_draft();
        let bone_source = studio.source.clone();
        assert_ne!(bone_source, original);
        let scene_id = studio.script.as_ref().unwrap().scenes[0].id.clone();
        if studio.project_document.is_none() {
            studio.project_document =
                Some(motion_project::ProjectDocument::new(bone_source.clone()).unwrap());
        }
        let document = studio.project_document.as_mut().unwrap();
        let mut candidate = document.art.clone();
        candidate.layers.push(layer(&scene_id));
        studio.art_editor.commit(&mut document.art, candidate);
        studio.record_art_transaction();
        studio.undo_edit(false);
        assert_eq!(studio.source, bone_source);
        assert!(studio
            .project_document
            .as_ref()
            .unwrap()
            .art
            .layers
            .is_empty());
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        studio.undo_edit(true);
        assert_eq!(studio.source, bone_source);
    }
    #[test]
    fn joint_draft_applies_through_history_and_rejects_stale_context() {
        let rig = "characters:\n  P:\n    asset: custom\n    rig:\n      version: 1\n      id: p\n      joints: [{id: root, parent: null}, {id: arm, parent: root, x: 5}]\n      parts: [{id: a, joint: arm, points: [[0, 0], [2, 0], [0, 2]], color: [1, 2, 3, 255]}]\nscenes:\n  - {id: shot, duration: 4, events: []}\n";
        let ready = || {
            let mut studio = Studio::default();
            studio.source = rig.into();
            studio.validate();
            studio.selected_actor = Some("P".into());
            studio.selected_joint = "arm".into();
            let script = studio.script.as_ref().unwrap();
            let mut draft = bone_editor::Draft::for_joint(&script.scenes[0], 0, "P", "arm");
            draft.keys.push(motion_script::ActorKey {
                rotation: 0.25,
                ..Default::default()
            });
            studio.joint_draft = Some((studio.source.clone(), draft));
            studio
        };
        let mut studio = ready();
        let original = studio.source.clone();
        studio.apply_joint_draft();
        assert_ne!(studio.source, original);
        let keys = &studio.script.as_ref().unwrap().scenes[0]
            .composition
            .joint_tracks["P"]["arm"];
        assert_eq!(keys[0].rotation, 0.25);
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        for context in 0..4 {
            let mut studio = ready();
            let original = studio.source.clone();
            match context {
                0 => studio.joint_draft.as_mut().unwrap().0.push(' '),
                1 => studio.scene_index += 1,
                2 => studio.selected_joint = "root".into(),
                _ => studio.joint_draft.as_mut().unwrap().1.keys[0].scale_x = 0.0,
            }
            studio.apply_joint_draft();
            assert_eq!(studio.source, original);
        }
    }
    #[test]
    fn stale_bone_context_and_invalid_offsets_preserve_source() {
        for context in 0..5 {
            let mut studio = studio_with_draft();
            let original = studio.source.clone();
            match context {
                0 => studio.bone_draft.as_mut().unwrap().0.push(' '),
                1 => studio.scene_index += 1,
                2 => studio.selected_actor = Some("another_actor".into()),
                3 => studio.selected_bone = "head".into(),
                _ => studio.bone_draft.as_mut().unwrap().1.keys[0].scale_x = 0.0,
            }
            studio.apply_bone_draft();
            assert_eq!(studio.source, original);
        }
        let mut studio = studio_with_draft();
        studio.invalidate_scene();
        assert!(studio.bone_draft.is_none());
    }
    #[test]
    fn advanced_direct_canvas_gate_obeys_scene_visibility_and_time() {
        let mut art = motion_art::Document {
            layers: vec![layer("shot")],
            ..Default::default()
        };
        assert!(!requires_composited_preview(&art, "shot", 0.0));
        art.layers[0].blend = motion_art::BlendMode::Multiply;
        assert!(requires_composited_preview(&art, "shot", 0.0));
        assert!(!requires_composited_preview(&art, "other", 0.0));
        art.layers[0].active = Some(motion_art::TimeRange {
            start: 1.0,
            end: 2.0,
        });
        assert!(!requires_composited_preview(&art, "shot", 0.0));
        assert!(requires_composited_preview(&art, "shot", 1.0));
        assert!(!requires_composited_preview(&art, "shot", 2.0));
        art.layers[0].visible = false;
        assert!(!requires_composited_preview(&art, "shot", 1.0));
        art.layers[0].visible = true;
        art.layers[0].blend = motion_art::BlendMode::Normal;
        art.layers[0].clip_shape = Some(motion_art::ClipShape::Ellipse {
            rect: motion_art::Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
        });
        assert!(requires_composited_preview(&art, "shot", 1.0));
    }
}

#[cfg(test)]
mod clip_editor_integration_tests {
    use super::*;
    fn studio_with_draft() -> Studio {
        let mut studio = Studio::default();
        studio.validate();
        let script = studio.script.as_ref().unwrap();
        let actor = script.characters.keys().next().unwrap().clone();
        let mut draft = clip_editor::Draft::new(&script.scenes[0], 0, &actor);
        draft.weights.gesture = 0.25;
        draft.weights.walk = 0.0;
        studio.selected_actor = Some(actor);
        studio.clip_draft = Some((studio.source.clone(), draft));
        studio
    }
    #[test]
    fn clip_strengths_are_one_undoable_saved_project_edit() {
        let mut studio = studio_with_draft();
        let original = studio.source.clone();
        let actor = studio.selected_actor.clone().unwrap();
        studio.apply_clip_draft();
        let edited = studio.source.clone();
        assert_ne!(edited, original);
        let weights = studio.script.as_ref().unwrap().scenes[0]
            .composition
            .clip_weights[&actor];
        assert_eq!(weights.gesture, 0.25);
        assert_eq!(weights.walk, 0.0);
        let document = motion_project::ProjectDocument::new(edited.clone()).unwrap();
        let loaded = motion_project::decode(&motion_project::encode(&document).unwrap()).unwrap();
        assert_eq!(loaded.source, edited);
        assert_eq!(
            motion_script::parse(&loaded.source).unwrap().scenes[0]
                .composition
                .clip_weights[&actor],
            weights
        );
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        studio.undo_edit(true);
        assert_eq!(studio.source, edited);
        studio.invalidate_scene();
        assert!(studio.clip_draft.is_none());
    }
    #[test]
    fn stale_invalid_or_busy_clip_drafts_preserve_source() {
        for context in 0..5 {
            let mut studio = studio_with_draft();
            let original = studio.source.clone();
            match context {
                0 => studio.clip_draft.as_mut().unwrap().0.push(' '),
                1 => studio.scene_index += 1,
                2 => studio.selected_actor = Some("another_actor".into()),
                3 => studio.clip_draft.as_mut().unwrap().1.weights.gesture = f64::NAN,
                _ => studio.busy = true,
            }
            studio.apply_clip_draft();
            assert_eq!(studio.source, original);
        }
    }
}

#[cfg(test)]
mod animated_strength_integration_tests {
    use super::*;

    fn studio_with_track() -> Studio {
        let mut studio = Studio::default();
        studio.validate();
        let script = studio.script.as_ref().unwrap();
        let actor = script.characters.keys().next().unwrap().clone();
        let mut draft = clip_editor::TrackDraft::new(&script.scenes[0], 0, &actor);
        draft.keys = vec![
            motion_script::ClipWeightKeyframe {
                time: 0.0,
                weights: motion_script::ClipWeights {
                    gesture: 0.0,
                    ..Default::default()
                },
                easing: motion_script::ActorEasing::Linear,
            },
            motion_script::ClipWeightKeyframe {
                time: 1.0,
                weights: motion_script::ClipWeights::default(),
                easing: motion_script::ActorEasing::Hold,
            },
        ];
        studio.selected_actor = Some(actor);
        studio.clip_track_draft = Some((studio.source.clone(), draft));
        studio
    }

    #[test]
    fn animated_strength_apply_persists_as_one_undoable_project_edit() {
        let mut studio = studio_with_track();
        let original = studio.source.clone();
        let actor = studio.selected_actor.clone().unwrap();
        studio.apply_clip_track_draft();
        let edited = studio.source.clone();
        assert_ne!(edited, original);
        let document = motion_project::ProjectDocument::new(edited.clone()).unwrap();
        let loaded = motion_project::decode(&motion_project::encode(&document).unwrap()).unwrap();
        let script = motion_script::parse(&loaded.source).unwrap();
        assert_eq!(
            script.scenes[0]
                .composition
                .sample_clip_weights(&actor, 0.5)
                .gesture,
            0.5
        );
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        studio.undo_edit(true);
        assert_eq!(studio.source, edited);
        studio.invalidate_scene();
        assert!(studio.clip_track_draft.is_none());
    }

    #[test]
    fn stale_invalid_or_busy_animated_drafts_never_change_source() {
        for context in 0..8 {
            let mut studio = studio_with_track();
            let original = studio.source.clone();
            match context {
                0 => studio.clip_track_draft.as_mut().unwrap().0.push(' '),
                1 => studio.scene_index += 1,
                2 => studio.selected_actor = Some("another_actor".into()),
                3 => studio.clip_track_draft.as_mut().unwrap().1.keys[0].time = f64::NAN,
                4 => studio.busy = true,
                5 => studio.diagnostics_busy = true,
                6 => studio.export_busy = true,
                _ => studio.setup_busy = true,
            }
            studio.apply_clip_track_draft();
            assert_eq!(studio.source, original);
        }
    }
}

#[cfg(test)]
mod clip_library_integration_tests {
    use super::*;
    const SOURCE: &str = "project: {width: 64, height: 64}\ncharacters: {A: {asset: a}, B: {asset: b}}\nscenes: [{id: shot, duration: 1, events: [], composition: {bone_tracks: {A: {head: [{at: 0, x: 12}]}, B: {left_hand: [{at: 0, y: 5}]}}}}]";

    fn studio() -> Studio {
        let mut studio = Studio::default();
        studio.source = SOURCE.into();
        studio.validate();
        studio.selected_actor = Some("A".into());
        studio
    }

    #[test]
    fn library_actions_are_saved_and_undoable_without_live_references() {
        let mut studio = studio();
        studio.apply_library_action(
            clip_library::Action::Capture {
                name: "nod".into(),
                replace: false,
            },
            SOURCE,
            0,
            "A",
        );
        let captured = studio.source.clone();
        assert_ne!(captured, SOURCE);
        studio.selected_actor = Some("B".into());
        studio.apply_library_action(
            clip_library::Action::Apply { name: "nod".into() },
            &captured,
            0,
            "B",
        );
        let applied = studio.source.clone();
        let script = studio.script.as_ref().unwrap();
        assert_eq!(
            script.scenes[0].composition.bone_tracks["A"],
            script.scenes[0].composition.bone_tracks["B"]
        );
        assert!(!script.scenes[0].composition.bone_tracks["B"].contains_key("left_hand"));
        let document = motion_project::ProjectDocument::new(applied.clone()).unwrap();
        let restored = motion_project::decode(&motion_project::encode(&document).unwrap()).unwrap();
        assert_eq!(restored.source, applied);
        assert!(motion_script::parse(&restored.source)
            .unwrap()
            .animation_clips
            .contains_key("nod"));
        studio.undo_edit(false);
        assert_eq!(studio.source, captured);
        studio.undo_edit(true);
        assert_eq!(studio.source, applied);
        studio.apply_library_action(
            clip_library::Action::Remove { name: "nod".into() },
            &applied,
            0,
            "B",
        );
        assert!(studio.script.as_ref().unwrap().animation_clips.is_empty());
        assert!(studio.script.as_ref().unwrap().scenes[0]
            .composition
            .bone_tracks["B"]
            .contains_key("head"));
    }

    #[test]
    fn retimed_library_apply_is_one_saved_undoable_edit() {
        let mut studio = studio();
        studio.source = SOURCE.replace(
            "[{at: 0, x: 12}]",
            "[{at: 0, x: 0}, {at: 1, x: 12, easing: smoothstep}]",
        );
        studio.validate();
        let initial = studio.source.clone();
        studio.apply_library_action(
            clip_library::Action::Capture {
                name: "nod".into(),
                replace: false,
            },
            &initial,
            0,
            "A",
        );
        let captured = studio.source.clone();
        let library = studio.script.as_ref().unwrap().animation_clips.clone();
        studio.selected_actor = Some("B".into());
        studio.apply_library_action(
            clip_library::Action::ApplyRetimed {
                name: "nod".into(),
                at: 0.25,
                duration: 0.5,
            },
            &captured,
            0,
            "B",
        );
        let edited = studio.source.clone();
        assert_ne!(edited, captured);
        let script = studio.script.as_ref().unwrap();
        assert_eq!(script.animation_clips, library);
        let keys = &script.scenes[0].composition.bone_tracks["B"]["head"];
        assert_eq!((keys[0].at, keys[1].at), (0.25, 0.75));
        assert_eq!((keys[0].x, keys[1].x), (0.0, 12.0));
        assert_eq!(keys[1].easing, motion_script::ActorEasing::Smoothstep);
        assert_eq!(studio.selected_actor.as_deref(), Some("B"));
        let document = motion_project::ProjectDocument::new(edited.clone()).unwrap();
        let restored = motion_project::decode(&motion_project::encode(&document).unwrap()).unwrap();
        assert_eq!(restored.source, edited);
        studio.undo_edit(false);
        assert_eq!(studio.source, captured);
        studio.undo_edit(true);
        assert_eq!(studio.source, edited);
    }

    #[test]
    fn library_actions_reject_stale_or_busy_context_without_source_changes() {
        for context in 0..7 {
            let mut studio = studio();
            let mut expected = SOURCE.to_string();
            match context {
                0 => expected.push(' '),
                1 => studio.scene_index = 1,
                2 => studio.selected_actor = Some("B".into()),
                3 => studio.busy = true,
                4 => studio.diagnostics_busy = true,
                5 => studio.export_busy = true,
                _ => studio.setup_busy = true,
            }
            studio.apply_library_action(
                clip_library::Action::Capture {
                    name: "nod".into(),
                    replace: false,
                },
                &expected,
                0,
                "A",
            );
            assert_eq!(studio.source, SOURCE);
        }
    }
}

#[cfg(test)]
mod live_clip_integration_tests {
    use super::*;
    const SOURCE: &str = "project: {width: 256, height: 256}\ncharacters: {A: {asset: a}, B: {asset: b}}\nanimation_clips: {tilt: {duration: 1, bone_tracks: {head: [{at: 0, x: 0}, {at: 1, x: 12}]}}}\nscenes: [{id: shot, duration: 2, events: []}]";
    fn studio() -> Studio {
        let mut studio = Studio::default();
        studio.source = SOURCE.into();
        studio.validate();
        studio.selected_actor = Some("A".into());
        studio
    }
    fn instance(id: &str) -> motion_script::ClipInstance {
        motion_script::ClipInstance {
            id: id.into(),
            clip: "tilt".into(),
            actor: "A".into(),
            at: 0.25,
            duration: 1.0,
            weight: 0.5,
            fade_in: 0.0,
            fade_out: 0.0,
            weight_keys: Vec::new(),
            looping: false,
            joint_mask: Vec::new(),
            blend_mode: motion_script::ClipBlendMode::Override,
            normalized_group: None,
        }
    }
    #[test]
    fn live_edits_persist_order_and_undo_and_select_cached_preview() {
        let mut studio = studio();
        assert!(!studio.advanced_canvas_needed());
        studio.apply_live_clip_action(
            live_clip_editor::Action::Upsert {
                instance: instance("one"),
            },
            SOURCE,
            0,
            "A",
        );
        let first = studio.source.clone();
        assert_ne!(first, SOURCE);
        assert!(studio.advanced_canvas_needed());
        assert_eq!(studio.selected_actor.as_deref(), Some("A"));
        let document = motion_project::ProjectDocument::new(first.clone()).unwrap();
        let restored = motion_project::decode(&motion_project::encode(&document).unwrap()).unwrap();
        assert_eq!(
            motion_script::parse(&restored.source).unwrap().scenes[0]
                .composition
                .clip_instances[0],
            instance("one")
        );
        studio.undo_edit(false);
        assert_eq!(studio.source, SOURCE);
        assert!(!studio.advanced_canvas_needed());
        studio.undo_edit(true);
        assert_eq!(studio.source, first);
        studio.apply_live_clip_action(
            live_clip_editor::Action::Upsert {
                instance: instance("two"),
            },
            &first,
            0,
            "A",
        );
        let added = studio.source.clone();
        studio.apply_live_clip_action(
            live_clip_editor::Action::Reorder {
                id: "two".into(),
                new_index: 0,
            },
            &added,
            0,
            "A",
        );
        let ordered = studio.source.clone();
        let rows = &studio.script.as_ref().unwrap().scenes[0]
            .composition
            .clip_instances;
        assert_eq!(
            rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            vec!["two", "one"]
        );
        studio.undo_edit(false);
        assert_eq!(studio.source, added);
        studio.undo_edit(true);
        assert_eq!(studio.source, ordered);
        studio.apply_live_clip_action(
            live_clip_editor::Action::Remove { id: "two".into() },
            &ordered,
            0,
            "A",
        );
        assert_eq!(
            studio.script.as_ref().unwrap().scenes[0]
                .composition
                .clip_instances,
            vec![instance("one")]
        );
    }
    #[test]
    fn additive_mode_uses_existing_history_persistence_and_context_guards() {
        let mut studio = studio();
        let mut row = instance("additive");
        row.blend_mode = motion_script::ClipBlendMode::Additive;
        studio.apply_live_clip_action(
            live_clip_editor::Action::Upsert {
                instance: row.clone(),
            },
            SOURCE,
            0,
            "A",
        );
        let edited = studio.source.clone();
        assert_ne!(edited, SOURCE);
        let document = motion_project::ProjectDocument::new(edited.clone()).unwrap();
        let loaded = motion_project::decode(&motion_project::encode(&document).unwrap()).unwrap();
        assert_eq!(
            motion_script::parse(&loaded.source).unwrap().scenes[0]
                .composition
                .clip_instances[0],
            row
        );
        studio.undo_edit(false);
        assert_eq!(studio.source, SOURCE);
        studio.undo_edit(true);
        assert_eq!(studio.source, edited);
        row.weight = f64::NAN;
        studio.apply_live_clip_action(
            live_clip_editor::Action::Upsert { instance: row },
            &edited,
            0,
            "A",
        );
        assert_eq!(studio.source, edited);
    }
    #[test]
    fn envelope_edits_are_saved_and_undoable_and_invalid_duration_is_atomic() {
        let mut studio = studio();
        let mut row = instance("animated");
        row.fade_in = 0.25;
        row.fade_out = 0.25;
        row.weight_keys = vec![
            motion_script::InstanceWeightKey {
                time: 0.0,
                weight: 0.0,
                easing: motion_script::ActorEasing::Linear,
            },
            motion_script::InstanceWeightKey {
                time: 1.0,
                weight: 1.0,
                easing: motion_script::ActorEasing::Hold,
            },
        ];
        studio.apply_live_clip_action(
            live_clip_editor::Action::Upsert {
                instance: row.clone(),
            },
            SOURCE,
            0,
            "A",
        );
        let edited = studio.source.clone();
        assert_ne!(edited, SOURCE);
        let document = motion_project::ProjectDocument::new(edited.clone()).unwrap();
        let restored = motion_project::decode(&motion_project::encode(&document).unwrap()).unwrap();
        let script = motion_script::parse(&restored.source).unwrap();
        assert_eq!(
            script.scenes[0].composition.clip_instances,
            vec![row.clone()]
        );
        assert_eq!(
            script.scenes[0].composition.clip_instances[0].sample_weight(0.5),
            0.125
        );
        studio.undo_edit(false);
        assert_eq!(studio.source, SOURCE);
        studio.undo_edit(true);
        assert_eq!(studio.source, edited);
        row.duration = 0.1;
        studio.apply_live_clip_action(
            live_clip_editor::Action::Upsert { instance: row },
            &edited,
            0,
            "A",
        );
        assert_eq!(studio.source, edited);
    }

    #[test]
    fn live_edits_reject_stale_invalid_busy_and_foreign_actor_actions() {
        for context in 0..9 {
            let mut studio = studio();
            let mut source = SOURCE.to_string();
            let mut row = instance("one");
            match context {
                0 => source.push(' '),
                1 => studio.scene_index = 1,
                2 => studio.selected_actor = Some("B".into()),
                3 => row.weight = f64::NAN,
                4 => studio.busy = true,
                5 => studio.diagnostics_busy = true,
                6 => studio.export_busy = true,
                7 => studio.setup_busy = true,
                _ => row.actor = "B".into(),
            }
            studio.apply_live_clip_action(
                live_clip_editor::Action::Upsert { instance: row },
                &source,
                0,
                "A",
            );
            assert_eq!(studio.source, SOURCE);
        }
        let mut studio = studio();
        let mut foreign = instance("other");
        foreign.actor = "B".into();
        studio.source = motion_script::upsert_clip_instance(SOURCE, 0, foreign).unwrap();
        studio.validate();
        studio.selected_actor = Some("A".into());
        let source = studio.source.clone();
        let mut update = instance("other");
        update.weight = 1.0;
        for action in [
            live_clip_editor::Action::Upsert { instance: update },
            live_clip_editor::Action::Remove { id: "other".into() },
            live_clip_editor::Action::Reorder {
                id: "other".into(),
                new_index: 0,
            },
        ] {
            studio.apply_live_clip_action(action, &source, 0, "A");
            assert_eq!(studio.source, source);
        }
    }
}

#[cfg(test)]
mod editable_timeline_integration_tests {
    use super::*;
    use motion_scene::editable_timeline::TimelineEdit;

    const SOURCE: &str = "characters: {}\nscenes: [{id: first, duration: 2, events: []}, {id: second, duration: 1, events: []}]\n";

    #[test]
    fn scene_reordering_keeps_selection_and_roundtrips_through_history() {
        let mut studio = Studio::default();
        studio.source = SOURCE.into();
        studio.validate();
        studio.apply_timeline_edit(TimelineEdit::MoveScene {
            scene_id: "first".into(),
            before: None,
        });
        let edited = studio.source.clone();
        assert!(edited.lines().any(|line| line == "project:"));
        assert!(serde_json::from_str::<serde_json::Value>(&edited).is_err());
        assert_eq!(studio.scene_index, 1);
        assert_eq!(studio.script.as_ref().unwrap().scenes[0].id, "second");
        let document = motion_project::ProjectDocument::new(edited.clone()).unwrap();
        assert_eq!(
            motion_project::decode(&motion_project::encode(&document).unwrap())
                .unwrap()
                .source,
            edited
        );
        studio.undo_edit(false);
        assert_eq!(studio.source, SOURCE);
        studio.undo_edit(true);
        assert_eq!(studio.source, edited);
    }

    #[test]
    fn invalid_or_busy_timeline_edits_leave_source_untouched() {
        let mut studio = Studio::default();
        studio.source = SOURCE.into();
        studio.validate();
        studio.apply_timeline_edit(TimelineEdit::MoveScene {
            scene_id: "first".into(),
            before: Some("missing".into()),
        });
        assert_eq!(studio.source, SOURCE);
        assert!(studio.status.contains("Timeline edit"));
        studio.busy = true;
        studio.apply_timeline_edit(TimelineEdit::MoveScene {
            scene_id: "first".into(),
            before: None,
        });
        assert_eq!(studio.source, SOURCE);
    }
}

#[cfg(test)]
mod native_integration_tests {
    use super::*;
    fn prepared_line(studio: &mut Studio) {
        studio.validate();
        let job = studio.jobs.first().expect("default dialogue").clone();
        let audio = motion_audio::AudioBuffer {
            sample_rate: 24000,
            channels: 1,
            samples: vec![0.0; 24],
        };
        studio.preview = Some(Preview {
            scene: job.scene,
            line_id: job.id,
            transcript: job.request.text,
            alignment_status: AlignmentStatus::None,
            clip: DialogueClip {
                actor: job.actor,
                start_frame: (job.at * 24000.0).round() as u64,
                audio: audio.clone(),
                alignment: None,
                estimated_alignment: None,
                gain: 1.0,
                trim_start_frame: 0,
            },
            peaks: vec![],
            result: SpeechResult {
                audio_path: "/not-read.wav".into(),
                sample_rate: 24000,
                sample_count: 24,
                cache_key: "fixture".into(),
                phonemes: String::new(),
                cached: true,
            },
            playback_audio: Arc::new(audio),
        });
    }
    #[test]
    fn native_alignment_eligibility_is_exact_and_idle_stable() {
        let mut studio = Studio::default();
        prepared_line(&mut studio);
        studio.sync_native_alignment();
        let identity = studio.alignment_identity.clone().unwrap();
        let audio = studio.alignment_audio.clone().unwrap();
        for _ in 0..50 {
            studio.sync_native_alignment();
        }
        assert_eq!(studio.alignment_identity.as_ref(), Some(&identity));
        assert!(Arc::ptr_eq(
            studio.alignment_audio.as_ref().unwrap(),
            &audio
        ));
        studio.preview.as_mut().unwrap().transcript.push('!');
        studio.sync_native_alignment();
        assert!(studio.alignment_identity.is_none());
        assert!(!studio.native_alignment.busy());
    }
    #[test]
    fn stale_native_alignment_cannot_replace_inputs_or_schedule_preparation() {
        let mut studio = Studio::default();
        prepared_line(&mut studio);
        studio.sync_native_alignment();
        studio.alignment = "existing selected timing".into();
        studio.scene_alignment = "existing map".into();
        let mut stale = studio.alignment_identity.clone().unwrap();
        stale.generation += 1;
        studio.apply_native_alignment(alignment_panel::Ready {
            identity: stale,
            json: "replacement".into(),
            scene_json: "replacement map".into(),
            cache: Default::default(),
        });
        assert_eq!(studio.alignment, "existing selected timing");
        assert_eq!(studio.scene_alignment, "existing map");
        assert!(!studio.busy);
        assert!(studio.worker.is_none());
        assert!(studio.preview.is_some());
    }
    #[test]
    fn advanced_native_canvas_uses_cached_snapshot_without_scene_audio() {
        let mut studio = Studio::default();
        studio.source="project: {title: Native, width: 32, height: 32, fps: 4}\ncharacters: {}\nscenes: [{id: s, duration: 1, events: []}]\n".into();
        studio.validate();
        let mut document = motion_project::ProjectDocument::new(studio.source.clone()).unwrap();
        document.art.layers.push(motion_art::Layer {
            id: "mask".into(),
            scene_id: "s".into(),
            name: "ellipse mask".into(),
            visible: true,
            plane: motion_art::Plane::Foreground,
            active: None,
            opacity: 1.0,
            attachment: None,
            clip: None,
            clip_shape: Some(motion_art::ClipShape::Ellipse {
                rect: motion_art::Rect {
                    x: 0.,
                    y: 0.,
                    width: 16.,
                    height: 16.,
                },
            }),
            blend: motion_art::BlendMode::Multiply,
            content: motion_art::Content::Rectangle {
                rect: motion_art::Rect {
                    x: 0.,
                    y: 0.,
                    width: 32.,
                    height: 32.,
                },
                color: motion_art::Color {
                    r: 128,
                    g: 64,
                    b: 32,
                    a: 255,
                },
            },
        });
        document.validate().unwrap();
        studio.project_document = Some(document);
        assert!(studio.advanced_canvas_needed());
        assert!(studio.scene_preview.is_none());
        let context = egui::Context::default();
        // The first asset sync publishes an empty pending inventory, then the
        // asynchronous decoder publishes its completed inventory (also empty
        // here). That legitimate resource revision must settle before testing
        // idle snapshot identity.
        studio
            .art_resources
            .sync(&context, &studio.project_root, &[]);
        let pending_images = studio.art_resources.images.clone();
        for _ in 0..200 {
            studio
                .art_resources
                .sync(&context, &studio.project_root, &[]);
            if !Arc::ptr_eq(&pending_images, &studio.art_resources.images) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(studio.art_resources.error.is_none());
        assert!(
            !Arc::ptr_eq(&pending_images, &studio.art_resources.images),
            "initial resource decode did not settle"
        );
        let mut snapshot = None;
        for _ in 0..200 {
            let _ = context.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| studio.native_canvas_ui(ui));
            });
            let canvas = studio.native_canvas.as_ref().unwrap();
            if let Some(old) = &snapshot {
                assert!(Arc::ptr_eq(old, &canvas.request.script));
            } else {
                snapshot = Some(canvas.request.script.clone());
            }
            if canvas.texture.is_some() {
                assert_eq!(canvas.shown, Some(0));
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(studio.native_canvas.as_ref().unwrap().shown, Some(0));
        // A paused seek to the exact scene end must render the endpoint, rather
        // than retain the last pre-EOF mouth/art state or reconstruct the script.
        studio.playhead = 1.0;
        for _ in 0..200 {
            let _ = context.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| studio.native_canvas_ui(ui));
            });
            let canvas = studio.native_canvas.as_ref().unwrap();
            assert!(Arc::ptr_eq(
                snapshot.as_ref().unwrap(),
                &canvas.request.script
            ));
            if canvas.shown == Some(24000) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(studio.native_canvas.as_ref().unwrap().shown, Some(24000));

        // Simulate a persistent snapshot validation error. Advancing the clock
        // must not keep submitting the same invalid snapshot at every sample.
        let canvas = studio.native_canvas.as_mut().unwrap();
        let mut invalid_art = (*canvas.request.art).clone();
        invalid_art.document.layers[0].opacity = f64::NAN;
        canvas.request.art = Arc::new(invalid_art);
        canvas.last_requested = None;
        studio.playhead = 0.0;
        for _ in 0..200 {
            let _ = context.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| studio.native_canvas_ui(ui));
            });
            if studio.native_canvas.as_ref().unwrap().failed.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let canvas = studio.native_canvas.as_ref().unwrap();
        assert_eq!(canvas.failed, Some(0));
        assert!(canvas.error.is_some());
        for sample in [6000, 12000, 18000, 24000] {
            studio.playhead = sample as f64 / 24000.0;
            let _ = context.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| studio.native_canvas_ui(ui));
            });
            let canvas = studio.native_canvas.as_ref().unwrap();
            assert_eq!(canvas.last_requested, Some(0));
            assert_eq!(canvas.failed, Some(0));
            assert!(Arc::ptr_eq(
                snapshot.as_ref().unwrap(),
                &canvas.request.script
            ));
        }
    }
}

#[cfg(test)]
mod custom_rig_history_integration_tests {
    use super::*;

    #[test]
    fn custom_rig_apply_preserves_context_history_and_project_persistence() {
        let mut studio = Studio::default();
        studio.validate();
        studio.selected_actor = Some("ALEX".into());
        let original = studio.source.clone();
        // Every rejected operation must leave both source and history untouched.
        for context in 0..7 {
            let script = motion_script::parse(&original).unwrap();
            let mut draft = custom_rig_editor::Draft::for_actor(&script, "ALEX");
            let mut captured = original.clone();
            match context {
                0 => captured.push(' '),
                1 => draft.actor = "EMMA".into(),
                2 => studio.busy = true,
                3 => studio.diagnostics_busy = true,
                4 => studio.export_busy = true,
                5 => studio.setup_busy = true,
                _ => draft.yaml = "version: 99".into(),
            }
            studio.custom_rig_draft = Some((captured, draft));
            studio.apply_custom_rig_draft(1);
            assert_eq!(studio.source, original);
            assert!(!studio.history.can_undo());
            assert!(!studio.history.can_redo());
            studio.busy = false;
            studio.diagnostics_busy = false;
            studio.export_busy = false;
            studio.setup_busy = false;
        }
        let script = motion_script::parse(&original).unwrap();
        studio.custom_rig_draft = Some((
            original.clone(),
            custom_rig_editor::Draft::for_actor(&script, "ALEX"),
        ));
        studio.apply_custom_rig_draft(1);
        let edited = studio.source.clone();
        assert_ne!(edited, original);
        let expected = motion_script::parse(&edited).unwrap().characters["ALEX"]
            .rig
            .clone();
        assert!(expected.is_some());
        let document = motion_project::ProjectDocument::new(edited.clone()).unwrap();
        let loaded = motion_project::decode(&motion_project::encode(&document).unwrap()).unwrap();
        assert_eq!(
            motion_script::parse(&loaded.source).unwrap().characters["ALEX"].rig,
            expected
        );
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        studio.undo_edit(true);
        assert_eq!(studio.source, edited);
        // An invalid edited definition cannot add a history entry or break Undo.
        studio.selected_actor = Some("ALEX".into());
        let script = motion_script::parse(&edited).unwrap();
        let mut invalid = custom_rig_editor::Draft::for_actor(&script, "ALEX");
        invalid.yaml = "version: 99".into();
        studio.custom_rig_draft = Some((edited.clone(), invalid));
        studio.apply_custom_rig_draft(1);
        assert_eq!(studio.source, edited);
        assert!(!studio.history.can_redo());
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        studio.undo_edit(true);
        assert_eq!(studio.source, edited);
    }
}
