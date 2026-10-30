//! Native dialogue workbench. Audio assets are prepared on bounded background workers.
mod alignment_panel;
mod alignment_worker;
mod art_editor;
mod art_resources;
mod asset_worker;
mod bone_editor;
mod camera_framing;
mod clip_editor;
mod clip_library;
mod composition_editor;
mod cue_rule_editor;
mod custom_rig_editor;
mod demo;
mod direct_manipulation;
mod direction_worker;
mod event_editor;
mod event_factory;
mod export_worker;
mod gpu_check_panel;
mod gpu_check_worker;
mod hand_props;
mod input_latency;
mod keyframe_editor;
mod kokoro_setup;
mod live_clip_editor;
mod local_speech_ui;
mod managed_license_evidence;
mod motion_blocks;
mod native_compositor;
mod playback;
mod project_history;
mod quick_start;
mod rig_clip_editor;
mod runtime_check;
mod scene_settings;
mod scene_worker;
mod screen_cues;
mod screen_video_worker;
mod sequence_panel;
mod sequence_worker;
mod stick_studio;
mod story_import;
mod studio_help;
mod timeline_edit;
mod ui_levels;
mod ui_metrics;
mod voice_editor;
mod webcam_pose;
mod xsheet_panel;

use eframe::egui;
use motion_audio::WaveformPeak;
#[cfg(test)]
use motion_lipsync::Alignment;
use motion_lipsync::{
    cache::{AlignmentCache, AlignmentStatus},
    Viseme,
};
use motion_tts::{CancellationToken, KokoroConfig, SpeechResult};
use motion_voice::{dialogue_jobs, timeline::DialogueClip, DialogueJob, VoiceUpdate, VoiceWorker};
use project_history::MAX_SOURCE_BYTES;
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
    /// Non-fatal preparation findings, e.g. low-confidence pronunciations.
    diagnostics: Vec<String>,
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
    /// Project `pronunciations:` for the text-estimated mouth fallback.
    pronunciations: std::collections::BTreeMap<String, String>,
    cancel: CancellationToken,
}
type PreviewResult = Result<Preview, String>;
struct NativeCanvas {
    revision: u64,
    request: native_compositor::Request,
    line_audio: Option<Arc<motion_audio::AudioBuffer>>,
    head_preview: Option<(String, f64)>,
    last_requested: Option<u64>,
    shown: Option<u64>,
    failed: Option<u64>,
    texture: Option<egui::TextureHandle>,
    error: Option<String>,
    /// `ScreenVideos::revision` the request's decoded proxies came from.
    screen_video_revision: u64,
}
fn same_optional_arc<T>(a: &Option<Arc<T>>, b: &Option<Arc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum InspectorTab {
    #[default]
    Edit,
    Speech,
    Export,
    Advanced,
}

struct PendingDirection {
    generation: u64,
    revision: u64,
    source: String,
    dialogue: String,
    root: String,
    piper_profile: String,
}
struct Studio {
    direction_worker: direction_worker::Worker,
    direction_generation: u64,
    direction_pending: Option<PendingDirection>,
    direction_imports: std::collections::BTreeMap<usize, String>,
    direction_imports_for: String,
    direction_report: String,
    piper_config_path: String,
    system_inventory: local_speech_ui::Inventory,
    piper_profile_editor: local_speech_ui::ProfileEditor,
    help_open: bool,
    help_topic: usize,
    inspector_tab: InspectorTab,
    character_mode: bool,
    character_drag: Option<direct_manipulation::Drag>,
    head_draft: Option<(String, direct_manipulation::HeadDraft)>,
    webcam_pose: webcam_pose::Panel,
    confirm_new_episode: bool,
    episode_title: String,
    quick_start: quick_start::Draft,
    story_text: String,
    story_issues: Vec<motion_script::story::StoryIssue>,
    story_review: Option<Vec<String>>,
    ui_metrics: ui_metrics::Metrics,
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
    cue_draft: Option<(String, cue_rule_editor::CueDraft)>,
    rule_draft: Option<(String, cue_rule_editor::RuleDraft)>,
    joint_draft: Option<(String, bone_editor::Draft)>,
    rig_clips: rig_clip_editor::State,
    hand_props: hand_props::State,
    screen_cues: screen_cues::Panel,
    /// Imports and preview decodes for screen videos (background lanes).
    screen_videos: screen_video_worker::ScreenVideos,
    /// Where a finished video import becomes a cue: actor, slot and time.
    screen_import_target: Option<screen_cues::Draft>,
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
    /// User-level caption choice layered over `project.captions` for scene preview,
    /// sequence preview and every export. Not screenplay text and not in undo history;
    /// persisted only in `.kilsat` project documents.
    caption_mode: motion_script::CaptionMode,
    /// Progressive-disclosure level. UI state only: never in the screenplay or project.
    ui_level: ui_levels::UiLevel,
    /// Project model policy (ADR-A01-09), persisted in `.kilsat` documents.
    model_policy: motion_project::ModelPolicy,
    /// Canvas drags as EventLog v1, persisted in `.kilsat` documents.
    drag_recorder: ui_levels::DragRecorder,
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
    xsheet: xsheet_panel::XSheetPanel,
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
                    let mut clip = clip;
                    let overrides =
                        motion_lipsync::english::Pronunciations::from_map(&task.pronunciations)
                            .map_err(|e| e.to_string())?;
                    // Same derivation as scene preparation; precedence keeps real alignments.
                    let _ = motion_voice::scene_audio::derive_text_mouths(
                        &mut clip, &task.job, &overrides,
                    );
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
            ui_metrics: Default::default(),
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
            cue_draft: None,
            rule_draft: None,
            joint_draft: None,
            rig_clips: rig_clip_editor::State::default(),
            hand_props: hand_props::State::default(),
            screen_cues: Default::default(),
            screen_videos: Default::default(),
            screen_import_target: None,
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
            direction_worker: direction_worker::Worker::new(),
            direction_generation: 0,
            direction_pending: None,
            direction_imports: std::collections::BTreeMap::new(),
            direction_imports_for: String::new(),
            direction_report: String::new(),
            piper_config_path: String::new(),
            system_inventory: local_speech_ui::Inventory::default(),
            piper_profile_editor: local_speech_ui::ProfileEditor::default(),
            help_open: false,
            help_topic: 0,
            inspector_tab: InspectorTab::default(),
            character_mode: true,
            character_drag: None,
            head_draft: None,
            webcam_pose: webcam_pose::Panel::default(),
            confirm_new_episode: false,
            episode_title: "Uusi jakso".into(),
            quick_start: quick_start::Draft::default(),
            story_text: String::new(),
            story_issues: Vec::new(),
            story_review: None,
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
            status: "Tarkista käsikirjoitus, jotta voit valita repliikin. Resursseja ei ladata automaattisesti.".into(),
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
            xsheet: xsheet_panel::XSheetPanel::new(),
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
            caption_mode: Default::default(),
            ui_level: Default::default(),
            model_policy: Default::default(),
            drag_recorder: Default::default(),
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
            Ok(source) if source.len() <= MAX_SOURCE_BYTES => {
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
                self.status = "Projektin rakenne päivitetty. Kumoa palauttaa edellisen käsikirjoituksen; valmistele ääni uudelleen.".into();
            }
            Ok(_) => self.status = "Muutos ylittää käsikirjoituksen 256 KiB:n kokorajan.".into(),
            Err(error) => self.status = format!("Projektin muokkaus epäonnistui: {error}"),
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
                .ok_or("Valittua kohtausta ei enää ole")?
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
                    self.status = "Kohtausten järjestys päivitetty. Siirtymät ja ajoitus tarkistettiin; Kumoa palauttaa edellisen käsikirjoituksen.".into();
                }
            }
            Err(error) => self.status = format!("Aikajanan muokkaus: {error}"),
        }
    }

    fn structure_ui(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Lisää, kopioi tai poista").show(ui, |ui| {
            ui.add_enabled_ui(self.script.is_some() && !self.busy && !self.diagnostics_busy && !self.export_busy, |ui| {
                field(ui, "Uusi pysyvä tunniste", &mut self.lifecycle_id);
                field(ui, "Uuden hahmon resurssiviite", &mut self.lifecycle_asset);
                ui.weak("Resurssiviitteet säilyvät. Esikatselu käyttää projektin hahmorakenteita.");
                let id = self.lifecycle_id.trim().to_string();
                if ui.button("Lisää kohtaus").clicked() {
                    let result = motion_script::add_scene(&self.source, motion_script::Scene { composition: Default::default(), id: id.clone(), duration: 5.0, background: None, events: vec![] });
                    self.apply_structure(result);
                }
                if ui.button("Kopioi valittu kohtaus").clicked() {
                    self.apply_structure(motion_script::duplicate_scene(&self.source, self.scene_index, &id));
                }
                let scene_ids: Vec<_> = self.script.as_ref().map(|s| s.scenes.iter()
                    .map(|scene| scene.id.clone()).collect()).unwrap_or_default();
                if let Some(selected) = scene_ids.get(self.scene_index).cloned() {
                    ui.horizontal(|ui| {
                        if ui.add_enabled(self.scene_index > 0, egui::Button::new("Siirrä kohtausta aiemmaksi")).clicked() {
                            self.apply_timeline_edit(motion_scene::editable_timeline::TimelineEdit::MoveScene {
                                scene_id: selected.clone(), before: scene_ids.get(self.scene_index - 1).cloned(),
                            });
                        }
                        if ui.add_enabled(self.scene_index + 1 < scene_ids.len(), egui::Button::new("Siirrä kohtausta myöhemmäksi")).clicked() {
                            self.apply_timeline_edit(motion_scene::editable_timeline::TimelineEdit::MoveScene {
                                scene_id: selected.clone(), before: scene_ids.get(self.scene_index + 2).cloned(),
                            });
                        }
                    });
                }
                if ui.button("Poista valittu kohtaus…").clicked() {
                    if let Some(scene) = self.script.as_ref().and_then(|s| s.scenes.get(self.scene_index)) {
                        self.lifecycle_delete = Some((true, scene.id.clone(), self.source.clone()));
                    }
                }
                if ui.button("Lisää hahmo").clicked() {
                    self.apply_structure(motion_script::add_character(&self.source, &id, motion_script::Character { asset: self.lifecycle_asset.trim().into(), default_pose: None, voice: None, rig: None, holds: None, timing: Default::default() }));
                }
                if ui.button("Kopioi valittu hahmo").clicked() {
                    if let Some(character) = self.selected_actor.as_ref().and_then(|actor| self.script.as_ref()?.characters.get(actor)).cloned() {
                        self.apply_structure(motion_script::add_character(&self.source, &id, character));
                    }
                }
                if ui.button("Poista valittu hahmo…").clicked() {
                    if let Some(actor) = &self.selected_actor {
                        self.lifecycle_delete = Some((false, actor.clone(), self.source.clone()));
                    }
                }
                if let Some((scene, id, source)) = self.lifecycle_delete.clone() {
                    ui.colored_label(egui::Color32::LIGHT_RED, format!("Poistetaanko {} {id}?", if scene {"kohtaus tapahtumineen"} else {"hahmo"}));
                    ui.weak("Käytössä olevaa hahmoa ei voi poistaa. Kumoa palauttaa tehdyt muutokset.");
                    if ui.button("Vahvista poisto").clicked() {
                        self.lifecycle_delete = None;
                        if source != self.source { self.status = "Käsikirjoitus vaihtui; valitse poistettava kohde uudelleen.".into(); }
                        else if scene {
                            if let Some(index) = self.script.as_ref().and_then(|s| s.scenes.iter().position(|s| s.id == id)) {
                                self.apply_structure(motion_script::remove_scene(&self.source, index));
                            }
                        } else { self.apply_structure(motion_script::remove_character(&self.source, &id)); }
                    }
                    if ui.button("Peru poisto").clicked() { self.lifecycle_delete = None; }
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
            self.settings_error = Some(
                "Käsikirjoitus tai kohtaus vaihtui; lataa asetusten luonnos uudelleen.".into(),
            );
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
            self.status = "Projektin ja kohtauksen asetuksia ei muutettu.".into();
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
        if source.len() > MAX_SOURCE_BYTES {
            self.settings_error =
                Some("Muokattu käsikirjoitus ylittää 256 KiB:n kokorajan.".into());
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
                    self.settings_error = Some("Valmistellun repliikin ajoitus on virheellinen; valmistele ääni uudelleen ennen keston muuttamista.".into());
                    return;
                };
                required_end = required_end.max(end);
            }
            if ((draft.duration_seconds * 24_000.0).round() as u64) < required_end {
                self.settings_error = Some(format!("Valmisteltu repliikki päättyy kohdassa {:.3} s. Siirrä tai lyhennä repliikkiä ennen kohtauksen lyhentämistä.",required_end as f64/24_000.0));
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
        self.status = "Projektin ja kohtauksen asetukset otettu käyttöön. Tapahtuma-ajat säilyvät sekunteina; valmistele ääni uudelleen ennen toistoa tai vientiä.".into();
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
        egui::CollapsingHeader::new("Projektin ja kohtauksen asetukset").show(ui, |ui| {
            ui.add_enabled_ui(!self.busy && !self.diagnostics_busy && !self.export_busy, |ui| {
                if let Some(draft) = &mut self.settings_draft { apply = draft.ui(ui); }
                reset = ui.button("Palauta asetusten luonnos").clicked();
            });
            ui.weak("Käytä tyhjentää keskeneräiset muokkaukset ja valmistellun äänen. Uusien puheäänien kestot tarkistetaan seuraavassa valmistelussa.");
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
            ui.colored_label(egui::Color32::LIGHT_RED, format!("Virhe: {error}"));
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
            self.status = "Puheäänen asetuksia ei muutettu.".into();
            return;
        }
        match motion_script::apply_voice(&self.source, &actor, draft.voice.clone()) {
            Ok(source) => {
                if source.len() > MAX_SOURCE_BYTES {
                    self.voice_edit_error =
                        Some("Muokattu käsikirjoitus ylittää 256 KiB:n kokorajan.".into());
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
                self.status = "Puheääni otettu käyttöön. Valmistele kohtauksen ääni uudelleen; muuttuneen puheen vanha kohdistus poistettiin. Välimuistitiedostot säilyvät.".into();
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
        egui::CollapsingHeader::new("Hahmon puheääni").default_open(true).show(ui, |ui| {
            ui.add_enabled_ui(!self.busy && !self.diagnostics_busy && !self.export_busy, |ui| {
                if let Some(draft) = &mut self.voice_draft { apply = draft.ui(ui); }
                reset = ui.button("Palauta puheäänen luonnos").clicked();
            });
            ui.weak("Käytä tyhjentää keskeneräiset tapahtumamuokkaukset ja valmistellun kohtausäänen. Valmistele ääni uudelleen kuullaksesi valinnan.");
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
            ui.colored_label(egui::Color32::LIGHT_RED, format!("Virhe: {error}"));
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
                    "Uusi tapahtumaluonnos: muokkaa ajoitusta ja asetuksia, sitten paina Käytä."
                        .into();
            }
            Err(error) => self.status = format!("Toiminto epäonnistui: {error}"),
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
            self.status = "Valittu tapahtuma vaihtui; poisto peruttiin.".into();
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
                if source.len() > MAX_SOURCE_BYTES {
                    self.status = "Muokattu käsikirjoitus ylittää 256 KiB:n kokorajan.".into();
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
                self.status = "Tapahtuma poistettu. Kumoa palauttaa edellisen käsikirjoituksen. Valmistele kohtauksen ääni uudelleen.".into();
            }
            Err(error) => self.status = format!("Tapahtuman poisto: {error}"),
        }
    }
    /// Turn a palette block dropped on `actor`'s track at `time` into one
    /// validated, undoable source edit.
    fn apply_block_drop(&mut self, block: &motion_blocks::Block, actor: &str, time: f64) {
        if self.busy || self.diagnostics_busy || self.export_busy {
            return;
        }
        self.flush_text_edit();
        let Some(script) = &self.script else {
            return;
        };
        let scene_index = self.scene_index;
        let Some(scene) = script.scenes.get(scene_index) else {
            return;
        };
        let first = scene.events.len();
        let result = match block {
            motion_blocks::Block::Clip(name) => {
                let at =
                    (time * f64::from(script.project.fps)).round() / f64::from(script.project.fps);
                let remaining = scene.duration - at;
                let custom = script
                    .characters
                    .get(actor)
                    .is_some_and(|c| c.rig.is_some());
                let result = if custom {
                    let length = script.rig_clips.get(name).map_or(1.0, |c| c.duration);
                    motion_script::apply_rig_clip(
                        &self.source,
                        scene_index,
                        actor,
                        name,
                        &motion_script::RigClipPlacement {
                            interval: Some((at, length.min(remaining))),
                            ..Default::default()
                        },
                    )
                } else {
                    let length = script.animation_clips.get(name).map_or(1.0, |c| c.duration);
                    motion_script::apply_animation_clip_retimed(
                        &self.source,
                        scene_index,
                        actor,
                        name,
                        at,
                        length.min(remaining),
                    )
                };
                result
                    .map(|source| (source, Vec::new()))
                    .map_err(|e| e.to_string())
            }
            _ => motion_blocks::events(script, scene_index, actor, time, block)
                .and_then(|events| motion_blocks::insert(&self.source, scene_index, first, events)),
        };
        match result {
            Ok((source, _)) if source.len() > MAX_SOURCE_BYTES => {
                self.status = "Muutos ylittää käsikirjoituksen 256 KiB:n kokorajan.".into();
            }
            Ok((source, inserted)) => {
                self.stop_playback();
                self.inserting_event = false;
                self.event_draft_source = None;
                self.pending_event_delete = None;
                self.record_source_edit(&self.source.clone(), &source);
                self.source = source;
                self.document_changed();
                self.validate();
                self.selected_actor = Some(actor.to_owned());
                self.selected_event = inserted.last().copied();
                self.event_draft = match (&self.script, self.selected_event) {
                    (Some(script), Some(event)) => {
                        event_editor::Draft::new(script, scene_index, event)
                    }
                    _ => None,
                };
                self.status = format!(
                    "{} lisätty hahmolle {actor}. Kumoa peruu lisäyksen.",
                    block.label()
                );
            }
            Err(error) => self.status = format!("Palikkaa ei voitu lisätä: {error}"),
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
        let replacement = drag.replacement(event);
        if &replacement == event {
            return;
        }
        self.inserting_event = false;
        self.event_draft_source = None;
        self.pending_event_delete = None;
        self.event_draft = Some(event_editor::Draft {
            scene_index: drag.scene_index,
            event_index: drag.event_index,
            event: replacement,
        });
        self.apply_event_draft();
        if let Some(error) = &self.event_edit_error {
            self.status = format!("Aikajanan muokkaus hylättiin: {error}");
        }
    }
    fn flush_text_edit(&mut self) {
        if let Some(before) = self.pending_text_edit.take() {
            self.record_source_edit(&before, &self.source.clone());
        }
    }
    fn record_source_edit(&mut self, before: &str, after: &str) {
        self.history
            .record(self.history_snapshot(before), self.history_snapshot(after));
    }
    fn snapshot(&self) -> project_history::Snapshot {
        self.history_snapshot(&self.source)
    }
    fn history_snapshot(&self, source: &str) -> project_history::Snapshot {
        let document = self.project_document.clone().map(|mut document| {
            let timing = motion_project::TimingDocument {
                line: self.alignment.clone(),
                scene: self.scene_alignment.clone(),
            };
            if timing.validate().is_ok() {
                document.timing = timing;
            }
            document
        });
        project_history::Snapshot::capture(source, document.as_ref())
    }
    /// Called only after the artwork editor reports an actual committed edit.
    fn record_art_transaction(&mut self) {
        let Some(before_art) = self.art_editor.take_transaction_before() else {
            return;
        };
        let Some(document) = self.project_document.as_mut() else {
            return;
        };
        let timing = motion_project::TimingDocument {
            line: self.alignment.clone(),
            scene: self.scene_alignment.clone(),
        };
        if timing.validate().is_ok() {
            document.timing = timing;
        }
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
                // Cue indices may no longer match the restored screenplay.
                self.screen_cues.draft = None;
                self.art_editor.reset();
                self.art_editor.dirty = art_dirty;
                self.asset_dirty |= inventory_changed;
                self.alignment = self
                    .project_document
                    .as_ref()
                    .map(|d| d.timing.line.clone())
                    .unwrap_or_default();
                self.scene_alignment = self
                    .project_document
                    .as_ref()
                    .map(|d| d.timing.scene.clone())
                    .unwrap_or_default();
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
                        self.status = format!(
                            "Historiasta palautettu käsikirjoitus on keskeneräinen: {error}"
                        );
                    }
                }
            }
            Ok(None) => {}
            Err(error) => self.status = format!("Historiaa ei muutettu: {error}"),
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
            self.event_edit_error =
                Some("Käsikirjoitus vaihtui; luo uusi tapahtumaluonnos.".into());
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
                if source.len() > MAX_SOURCE_BYTES {
                    self.event_edit_error = Some(
                        "Muokattu käsikirjoitus ylittää tallennuksen 256 KiB:n kokorajan.".into(),
                    );
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
                    self.status = "Tapahtuma otettu käyttöön. Valmistele kohtauksen ääni uudelleen ennen toistoa tai vientiä.".into();
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
                    self.status = "Valmistellaan synteettistä ääniesimerkkiä. Äänet ovat testiääniä ja suun ajoitus havainnollinen. Kuuntele painamalla Toista ääni.".into();
                }
            }
            Err(error) => self.status = format!("Ääniesimerkki: {error}"),
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
        self.character_drag = None;
        self.head_draft = None;
        self.webcam_pose.set_context("document_changed");
        self.document_revision = self.document_revision.wrapping_add(1);
        self.sequence_changed();
    }
    fn invalidate_scene(&mut self) {
        self.custom_rig_draft = None;
        self.bone_draft = None;
        self.cue_draft = None;
        self.rule_draft = None;
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
        self.character_drag = None;
        self.head_draft = None;
        self.webcam_pose.set_context("reset_scene");
        self.custom_rig_draft = None;
        self.bone_draft = None;
        self.cue_draft = None;
        self.rule_draft = None;
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
                if doc.source.len() > MAX_SOURCE_BYTES {
                    return Err(
                        "Projektin käsikirjoitus ylittää muokkaimen 256 KiB:n kokorajan".into(),
                    );
                }
                let root = path
                    .parent()
                    .ok_or("Projektin kansiota ei löydy")?
                    .to_string_lossy()
                    .into_owned();
                Ok((doc, root))
            })();
            match result {
                Ok((mut doc, root)) => {
                    self.source = doc.source.clone();
                    let timing = doc.timing.clone();
                    self.caption_mode = doc.caption_mode;
                    self.set_model_policy(doc.model_policy);
                    self.load_event_log(&mut doc);
                    self.document_changed();
                    self.project_document = Some(doc);
                    self.art_editor.reset();
                    self.asset_dirty = false;
                    self.project_root = root;
                    self.history.reset(&self.source);
                    self.pending_text_edit = None;
                    self.alignment = timing.line;
                    self.scene_alignment = timing.scene;
                    self.preview = None;
                    self.scene_index = 0;
                    self.validate();
                }
                Err(error) => self.status = format!("Projektin avaaminen: {error}"),
            }
            return;
        }
        self.stop_playback();
        use std::io::Read;
        let result = (|| -> Result<String, String> {
            let file = std::fs::File::open(&self.script_path)
                .map_err(|e| format!("Tiedoston {} avaaminen: {e}", self.script_path))?;
            let mut bytes = Vec::new();
            file.take((MAX_SOURCE_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|e| format!("Read screenplay: {e}"))?;
            if bytes.len() > MAX_SOURCE_BYTES {
                return Err("Käsikirjoitus ylittää muokkaimen 256 KiB:n kokorajan".into());
            }
            String::from_utf8(bytes)
                .map_err(|e| format!("Käsikirjoituksen on oltava UTF-8-muotoinen: {e}"))
        })();
        match result {
            Ok(source) => {
                self.art_editor.reset();
                self.project_document = None;
                self.asset_dirty = false;
                // A plain screenplay has no recorded input. The model policy is kept,
                // so a strict session never silently becomes model-enabled.
                self.drag_recorder.clear();
                self.source = source;
                self.document_changed();
                self.history.reset(&self.source);
                self.pending_text_edit = None;
                self.alignment.clear();
                self.scene_alignment.clear();
                self.invalidate_scene();
                self.jobs.clear();
                self.preview = None;
                self.status = "Käsikirjoitus avattu; paina Tarkista ennen äänen valmistelua. Tuodut äänet käyttävät asetettua projektikansiota.".into();
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
                if self.source.len() > MAX_SOURCE_BYTES {
                    return Err(
                        "Projektin käsikirjoitus ylittää muokkaimen 256 KiB:n kokorajan".into(),
                    );
                }
                let parent = Path::new(&self.script_path)
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                let root = std::fs::canonicalize(parent).map_err(|e| e.to_string())?;
                let configured =
                    std::fs::canonicalize(&self.project_root).map_err(|e| e.to_string())?;
                if root != configured {
                    return Err("Tallenna .kilsat asetettuun projektikansioon, jotta resurssien ja äänten suhteelliset polut säilyvät. Kopioi resurssit ennen projektin siirtämistä.".into());
                }
                let mut doc = self.project_document.clone().unwrap_or(
                    motion_project::ProjectDocument::new(self.source.clone())
                        .map_err(|e| e.to_string())?,
                );
                doc.source = self.source.clone();
                doc.timing = motion_project::TimingDocument {
                    line: self.alignment.clone(),
                    scene: self.scene_alignment.clone(),
                };
                doc.caption_mode = self.caption_mode;
                doc.model_policy = self.model_policy;
                doc.event_log = self.drag_recorder.project_log();
                motion_project::save_atomic(Path::new(&self.script_path), &doc)
                    .map_err(|e| e.to_string())?;
                Ok(doc)
            })();
            match result {
                Ok(mut doc) => {
                    // The recorder owns the live log; see `load_event_log`.
                    doc.event_log = None;
                    self.project_document = Some(doc);
                    self.asset_dirty = false;
                    self.art_editor.dirty = false;
                    self.flush_text_edit();
                    self.asset_dirty = false;
                    self.status = "Projekti ja resurssiluettelo tallennettu.".into();
                }
                Err(error) => self.status = format!("Projektin tallentaminen: {error}"),
            }
            return;
        }
        if !self.alignment.trim().is_empty()
            || !self.scene_alignment.trim().is_empty()
            || self
                .project_document
                .as_ref()
                .is_some_and(|d| !d.timing.is_empty())
        {
            self.status =
                "Tarkistettu suun ajoitus tarvitsee .kilsat-tiedoston. Vaihda tallennuspolku ennen tallentamista."
                    .into();
            return;
        }
        if self
            .project_document
            .as_ref()
            .is_some_and(|d| !d.art.layers.is_empty())
        {
            self.status =
                "Kuvitus tarvitsee .kilsat-tiedoston. Vaihda tallennuspolun päätteeksi .kilsat ennen tallentamista.".into();
            return;
        }
        self.flush_text_edit();
        if self.source.len() > MAX_SOURCE_BYTES {
            self.status = "Käsikirjoitus ylittää tallennuksen 256 KiB:n kokorajan".into();
            return;
        }
        self.status = match std::fs::write(&self.script_path, &self.source) {
            Ok(()) => format!("Käsikirjoitus tallennettu: {}", self.script_path),
            Err(e) => format!("Tiedoston {} tallentaminen: {e}", self.script_path),
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
                    "Tarkistus valmis: {} repliikkiä. Puheääni, nopeus ja kieli määräytyvät käsikirjoituksesta.",
                    self.jobs.len()
                );
            }
            Err(e) => {
                self.invalidate_scene();
                self.jobs.clear();
                self.preview = None;
                self.status = format!("Käsikirjoituksen tarkistus epäonnistui: {e}");
            }
        }
    }
    fn scene_audio_ui(&mut self, ui: &mut egui::Ui) {
        let Some(preview) = &self.scene_preview else {
            return;
        };
        ui.label(format!(
            "Kohtaus {} · {} repliikkiä · {:.3} s",
            preview.scene,
            preview.clips.len(),
            preview.audio.duration_seconds()
        ));
        let audio = Arc::clone(&preview.audio);
        let duration = audio.duration_seconds();
        waveform(ui, &preview.peaks, self.playhead / duration.max(0.001));
        ui.horizontal(|ui| {
            if ui.button("Toista ääni").clicked() {
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
            if ui.button("Tauota ääni").clicked() {
                if let Some(player) = &self.playback {
                    if let Err(e) = player.pause() {
                        self.playback_error = Some(e);
                    }
                }
            }
            if ui.button("Jatka ääntä").clicked() && self.scene_audio_active {
                self.sequence_panel.stop_transport();
                if let Some(player) = &self.playback {
                    if let Err(e) = player.resume() {
                        self.playback_error = Some(e);
                    }
                }
            }
            if ui.button("Pysäytä ääni").clicked() {
                self.stop_playback();
                self.playhead = 0.0;
            }
        });
        if ui
            .add(
                egui::Slider::new(&mut self.playhead, 0.0..=duration)
                    .text("Kohtauksen toistokohta (s)"),
            )
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
                ui.label(format!(
                    "Repliikki {id}: {}",
                    alignment_status_text(*status)
                ));
            }
            for message in &preview.diagnostics {
                ui.colored_label(egui::Color32::YELLOW, message);
            }
        }
        ui.label("Yhteinen äänikello ohjaa kaikkien hahmojen suun liikkeitä. Tekstistä tehty ajoitus on arvio. Äänen ja kuvan yhteistoiminta on tarkistettava oikealla Macilla.");
        if let Some(error) = &self.playback_error {
            ui.colored_label(egui::Color32::LIGHT_RED, format!("Virhe: {error}"));
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
        if self.model_policy.is_strict() {
            // Rhubarb uses PocketSphinx's acoustic model (ADR-A01-09): never started.
            self.native_alignment.cancel();
            ui.label("Puheen tunnistava kohdistus (Rhubarb)");
            ui.weak(policy_block_text(
                motion_project::ModelBackedStep::RecognizerAlignment,
            ));
            return;
        }
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
    fn commit_reviewed_timing(&mut self, line: String, scene: String) -> Result<(), String> {
        self.flush_text_edit();
        let mut before = match &self.project_document {
            Some(document) => document.clone(),
            None => motion_project::ProjectDocument::new(self.source.clone())
                .map_err(|e| e.to_string())?,
        };
        before.source = self.source.clone();
        let prior_timing = motion_project::TimingDocument {
            line: self.alignment.clone(),
            scene: self.scene_alignment.clone(),
        };
        if prior_timing.validate().is_ok() {
            before.timing = prior_timing;
        }
        let mut after = before.clone();
        after.timing = motion_project::TimingDocument {
            line: line.clone(),
            scene: scene.clone(),
        };
        after.validate().map_err(|e| e.to_string())?;
        self.history.record(
            project_history::Snapshot::capture(&self.source, Some(&before)),
            project_history::Snapshot::capture(&self.source, Some(&after)),
        );
        self.project_document = Some(after);
        self.alignment = line;
        self.scene_alignment = scene;
        Ok(())
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
        if let Err(error) = self.commit_reviewed_timing(ready.json, ready.scene_json) {
            self.status = format!("Ajoitusta ei muutettu: {error}");
            return;
        }
        self.stop_playback();
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
            pronunciations: self.script_pronunciations(),
            cancel: cancel.clone(),
        }) {
            Ok(()) => {
                self.busy = true;
                self.active_cancel = Some(cancel);
                self.status =
                    "Valmistellaan olemassa olevaa ääntä tarkistetulla suun ajoituksella. Puhetta ei luoda eikä tiedostoja ladata."
                        .into();
            }
            Err(e) => {
                self.status =
                    format!("Ajoitus otettu käyttöön; valmistele repliikki uudelleen: {e}")
            }
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
    fn editing_idle(&self) -> bool {
        !self.busy && !self.diagnostics_busy && !self.export_busy
    }
    fn scene_playing(&self) -> bool {
        self.scene_transport
            .as_ref()
            .is_some_and(|p| p.is_playing())
            || self.playback.as_ref().is_some_and(|p| p.snapshot().playing)
    }
    fn create_empty_scene(&mut self) {
        if !self.editing_idle() {
            return;
        }
        match direct_manipulation::blank_scene(&self.source) {
            Ok((source, index)) => {
                self.apply_structure(Ok(source.clone()));
                if self.source == source {
                    self.scene_index = index;
                    self.reset_scene();
                    self.status =
                        "Uusi 10 sekunnin kohtaus luotu. Aloita animointi vetämällä hahmoja."
                            .into();
                }
            }
            Err(error) => self.status = format!("Kohtauksen luominen epäonnistui: {error}"),
        }
    }
    fn create_new_episode(&mut self) {
        if !self.editing_idle() {
            return;
        }
        match direct_manipulation::new_episode(&self.episode_title) {
            Ok(source) => {
                self.flush_text_edit();
                let before = self.snapshot();
                self.stop_playback();
                self.source = source;
                self.project_document = None;
                self.script_path.clear();
                self.asset_dirty = false;
                self.art_editor.reset();
                self.alignment.clear();
                self.scene_alignment.clear();
                self.preview = None;
                self.scene_index = 0;
                self.selected_actor = None;
                self.reset_scene();
                self.document_changed();
                self.validate();
                self.history.record(before, self.snapshot());
                self.confirm_new_episode = false;
                self.status = "Uusi jakso valmis. Tallenna se uutena tiedostona. Kumoa palauttaa edellisen projektin sisällön.".into();
            }
            Err(error) => self.status = format!("Jakson luominen epäonnistui: {error}"),
        }
    }
    fn load_stick_studio(&mut self) {
        if !self.editing_idle() {
            return;
        }
        let mut document = match stick_studio::load() {
            Ok(document) => document,
            Err(error) => {
                self.status = format!("Tikku-ukkostudion avaaminen epäonnistui: {error}");
                return;
            }
        };
        self.flush_text_edit();
        let before = self.snapshot();
        self.stop_playback();
        self.source = document.source.clone();
        self.caption_mode = document.caption_mode;
        self.set_model_policy(document.model_policy);
        self.load_event_log(&mut document);
        self.project_document = Some(document);
        self.script_path.clear();
        self.asset_dirty = false;
        self.art_editor.reset();
        self.art_editor.dirty = true;
        self.alignment.clear();
        self.scene_alignment.clear();
        self.preview = None;
        self.scene_index = 0;
        self.selected_actor = Some("NOUTAJA".into());
        self.selected_joint = "head".into();
        self.editor_tab = false;
        self.character_mode = true;
        self.inspector_tab = InspectorTab::Edit;
        self.reset_scene();
        self.document_changed();
        self.validate();
        self.history.record(before, self.snapshot());
        self.confirm_new_episode = false;
        self.status = "Tikku-ukkostudio avattu: kuusi nivelhahmoa, taustat ja esineet ovat muokattavia. Esimerkki on äänetön. Tallenna uutena .kilsat-tiedostona. Kumoa palauttaa edellisen projektin sisällön ja kuvituksen.".into();
    }
    fn load_story(&mut self) {
        if !self.editing_idle() {
            return;
        }
        let built = match story_import::build_with(&self.story_text, None, &self.story_project()) {
            Ok(built) => built,
            Err(issues) => {
                self.status = format!(
                    "Käsikirjoitusta ei hyväksytty: {} korjattavaa kohtaa. Kopioi virheet tekoälylle ja liitä korjattu versio.",
                    issues.len()
                );
                self.story_issues = issues;
                return;
            }
        };
        self.apply_story(built);
    }
    fn apply_story(&mut self, built: story_import::Built) {
        self.story_issues.clear();
        let mut document = built.document;
        let compilation = built.compilation;
        self.story_review = Some(
            compilation
                .review
                .iter()
                .map(story_import::review_text)
                .collect(),
        );
        self.flush_text_edit();
        let before = self.snapshot();
        self.stop_playback();
        self.source = document.source.clone();
        self.caption_mode = document.caption_mode;
        self.set_model_policy(document.model_policy);
        self.load_event_log(&mut document);
        self.project_document = Some(document);
        self.script_path.clear();
        self.asset_dirty = false;
        self.art_editor.reset();
        self.art_editor.dirty = true;
        self.alignment.clear();
        self.scene_alignment.clear();
        self.preview = None;
        self.scene_index = 0;
        self.selected_actor = compilation.script.characters.keys().next().cloned();
        self.selected_joint = "head".into();
        self.editor_tab = false;
        self.character_mode = true;
        self.reset_scene();
        self.document_changed();
        self.validate();
        self.history.record(before, self.snapshot());
        self.confirm_new_episode = false;
        let timing = match compilation.timing {
            motion_script::story::StoryTiming::Estimated => "Repliikkien kestot ovat tekstistä arvioituja: valmistele puhe ja tarkista ajoitus ennen vientiä.",
            motion_script::story::StoryTiming::Supplied => "Jokainen repliikki on ajoitettu oikean puheen mitatun keston mukaan ja käyttää omaa WAV-tiedostoaan. Tallenna projekti samaan projektikansioon.",
        };
        let notes = if compilation.notes.is_empty() {
            String::new()
        } else {
            format!(" Huomiot: {}", compilation.notes.join("; "))
        };
        self.status = format!(
            "Animaatio luotu käsikirjoituksesta: {} kohtausta, {} otosta, {} repliikkiä, {:.1} s. {timing} Kumoa palauttaa edellisen projektin.{notes}",
            compilation.script.scenes.len(),
            compilation.shot_count,
            compilation.lines.len(),
            compilation.duration
        );
    }
    fn story_project(&self) -> story_import::ProjectContent {
        story_import::ProjectContent::new(
            &self.source,
            self.project_document.as_ref().map(|d| &d.art),
        )
    }
    fn start_story_speech(&mut self) {
        if !self.editing_idle() || self.setup_busy {
            return;
        }
        self.flush_text_edit();
        self.stop_playback();
        let root = std::path::PathBuf::from(self.project_root.trim());
        if !root.is_absolute() || !root.is_dir() {
            self.status = "Anna projektikansion koko polku (Projektin lisätyökalut → Projektikansio) ennen puheen valmistelua. Äänitiedostot tallennetaan sinne.".into();
            return;
        }
        if let Err(issues) = story_import::build_with(&self.story_text, None, &self.story_project())
        {
            self.status = format!(
                "Käsikirjoitusta ei hyväksytty: {} korjattavaa kohtaa.",
                issues.len()
            );
            self.story_issues = issues;
            return;
        }
        self.story_issues.clear();
        self.direction_generation = self.direction_generation.wrapping_add(1);
        let generation = self.direction_generation;
        let cancel = CancellationToken::new();
        let request = direction_worker::Request {
            generation,
            source: String::new(),
            dialogue: String::new(),
            root,
            provider: self.speech_provider(),
            imported_audio: Default::default(),
            cancel: cancel.clone(),
            story: Some(self.story_text.clone()),
            story_project: self.story_project(),
        };
        match self.direction_worker.submit(request) {
            Ok(()) => {
                self.direction_pending = Some(PendingDirection {
                    generation,
                    revision: self.document_revision,
                    source: self.source.clone(),
                    dialogue: self.story_text.clone(),
                    root: self.project_root.clone(),
                    piper_profile: self.piper_config_path.clone(),
                });
                self.active_cancel = Some(cancel);
                self.busy = true;
                self.status = "Tuotetaan hahmojen puhe ja ajoitetaan tarina sen mukaan…".into();
            }
            Err(error) => {
                self.status = format!("Puheen valmistelun käynnistys epäonnistui: {error}")
            }
        }
    }
    fn story_ui(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Luo animaatio käsikirjoituksesta (Claude / ChatGPT)")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("1. Kopioi ohje ja liitä se Claudeen tai ChatGPT:hen. Kirjoita perään tarinasi idea.");
                if ui.button("Kopioi ohje tekoälylle").clicked() {
                    match story_import::guide(&self.story_project()) {
                        Ok(guide) => {
                            ui.ctx().copy_text(guide);
                            self.status = "Ohje kopioitu leikepöydälle. Liitä se tekoälylle ja kirjoita perään tarinan idea, esimerkiksi: Sisu ja Noutaja riitelevät bussipysäkillä.".into();
                        }
                        Err(error) => self.status = format!("Ohjeen luominen epäonnistui: {error}"),
                    }
                }
                ui.label("2. Liitä tekoälyn vastaus tähän (koko vastaus käy, koodilohko poimitaan).");
                ui.add(
                    egui::TextEdit::multiline(&mut self.story_text)
                        .desired_rows(10)
                        .desired_width(f32::INFINITY)
                        .code_editor()
                        .char_limit(motion_script::story::MAX_STORY_BYTES),
                );
                ui.add_enabled_ui(self.editing_idle() && !self.story_text.trim().is_empty(), |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .button("3. Luo animaatio oikealla puheella")
                            .on_hover_text("Tuottaa jokaisen repliikin hahmon äänellä (Kokoro englanniksi, Piper suomeksi), mittaa kestot ja ajoittaa koko tarinan niiden mukaan. Äänet tallennetaan projektikansioon. Korvaa nykyisen projektin; Kumoa palauttaa sen.")
                            .clicked()
                        {
                            self.start_story_speech();
                        }
                        if ui
                            .button("Luonnos ilman puhetta")
                            .on_hover_text("Nopea esikatselu: repliikkien kestot arvioidaan tekstistä. Korvaa nykyisen projektin; Kumoa palauttaa sen.")
                            .clicked()
                        {
                            self.load_story();
                        }
                    });
                });
                if let Some(review) = &self.story_review {
                    if review.is_empty() {
                        ui.label("Leikkaustarkastus: ei automaattisesti havaittuja ongelmia. Katso ja kuuntele silti valmis animaatio.");
                    } else {
                        ui.colored_label(egui::Color32::from_rgb(220, 170, 80), format!("Leikkaustarkastus: {} huomiota", review.len()));
                        for note in review.iter().take(20) {
                            ui.label(egui::RichText::new(note).small());
                        }
                    }
                }
                if !self.story_issues.is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(220, 120, 90), "Korjattavaa:");
                    for issue in self.story_issues.iter().take(20) {
                        ui.label(egui::RichText::new(issue.to_string()).small());
                    }
                    if self.story_issues.len() > 20 {
                        ui.weak(format!("… ja {} muuta", self.story_issues.len() - 20));
                    }
                    if ui.button("Kopioi virheet tekoälylle").clicked() {
                        ui.ctx().copy_text(motion_script::story::issues_report(&self.story_issues));
                        self.status = "Virheet kopioitu. Liitä ne samaan tekoälykeskusteluun ja liitä korjattu käsikirjoitus tänne.".into();
                    }
                }
                ui.weak("Käytettävissä: Tikku-ukkostudion hahmot, lavasteet ja käsiesineet sekä avoimen projektin omat rigatut hahmot ja kuvitetut kohtaukset. Kopioi ohje uudelleen, kun lisäät omia hahmoja tai taustoja.");
            });
    }
    fn creation_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_enabled_ui(self.editing_idle(), |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("+ Uusi kohtaus").clicked() { self.create_empty_scene(); }
                if ui.button("+ Uusi jakso").clicked() { self.confirm_new_episode = true; }
                if ui.button("Tikku-ukkostudio")
                    .on_hover_text("Avaa kuusi alkuperäistä nivelhahmoa, taustat ja esineet. Tallenna nykyinen työ ensin, jos haluat erillisen tiedoston. Kumoa palauttaa sen sisällön.")
                    .clicked() { self.load_stick_studio(); }
            });
            if self.confirm_new_episode {
                ui.label("Aloitetaanko uusi jakso? Tallenna nykyinen projekti ensin, jos haluat siitä erillisen tiedoston.");
                ui.label("Uuden jakson nimi");
                ui.text_edit_singleline(&mut self.episode_title);
                ui.horizontal(|ui| {
                    if ui.button("Luo jakso").clicked() { self.create_new_episode(); }
                    if ui.button("Jatka nykyistä").clicked() { self.confirm_new_episode = false; }
                });
            }
        });
        ui.separator();
    }
    fn drag_current(
        &self,
        drag: &direct_manipulation::Drag,
        scene: &motion_scene::EvaluatedScene,
        rect: egui::Rect,
    ) -> bool {
        drag.source == self.source
            && drag.scene_index == self.scene_index
            && drag.time == self.playhead
            && drag.camera == scene.camera
            && drag.rect == rect
    }
    fn preview_character_drag(
        &self,
        scene: &mut motion_scene::EvaluatedScene,
        ctx: &egui::Context,
    ) {
        if let Some(drag) = &self.character_drag {
            if self.drag_current(drag, scene, drag.rect) {
                if let Some(delta) = ctx
                    .input(|i| i.pointer.interact_pos())
                    .and_then(|p| drag.delta(scene, p))
                {
                    if let Some(actor) = scene.actors.iter_mut().find(|a| a.id == drag.actor) {
                        actor.x += delta[0];
                        actor.y += delta[1];
                    }
                }
            }
        }
    }
    /// Start an EventLog gesture for a whole-character drag of `actor`'s root joint.
    fn begin_drag_recording(&mut self, scene: &motion_scene::EvaluatedScene, actor: &str) {
        let Some(script) = &self.script else {
            return;
        };
        let Some(scene_id) = script.scenes.get(self.scene_index).map(|s| s.id.clone()) else {
            return;
        };
        let (Some(joint), Some(pose)) = (
            ui_levels::drag_joint(script, actor),
            scene.actors.iter().find(|pose| pose.id == actor),
        ) else {
            return;
        };
        // Humanoid: the hip joint. Custom rig: its root, which the actor transform moves.
        let origin = if script
            .characters
            .get(actor)
            .is_some_and(|c| c.rig.is_some())
        {
            [pose.x, pose.y]
        } else {
            let hip = pose.local_to_world(pose.skeleton[motion_animation::HIP]);
            [hip.x, hip.y]
        };
        if let Err(error) =
            self.drag_recorder
                .begin(&scene_id, self.playhead, actor, &joint, origin)
        {
            self.status = error;
        }
    }
    fn finish_character_drag(
        &mut self,
        drag: direct_manipulation::Drag,
        scene: &motion_scene::EvaluatedScene,
        rect: egui::Rect,
        pointer: egui::Pos2,
    ) {
        if !self.editing_idle() || self.scene_playing() || !self.drag_current(&drag, scene, rect) {
            return;
        }
        match drag.apply(scene, pointer) {
            Ok(source) if source != self.source => {
                self.apply_structure(Ok(source));
                self.selected_actor = Some(drag.actor);
                self.playhead = drag.time;
            }
            Ok(_) => (),
            Err(error) => self.status = format!("Hahmon siirtäminen epäonnistui: {error}"),
        }
    }
    fn character_canvas_ui(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        scene: &motion_scene::EvaluatedScene,
        enabled: bool,
    ) {
        if !enabled
            || ui.input(|i| i.key_pressed(egui::Key::Escape))
            || self
                .character_drag
                .as_ref()
                .is_some_and(|drag| !self.drag_current(drag, scene, rect))
        {
            self.character_drag = None;
        }
        let response = ui
            .add_enabled_ui(enabled, |ui| {
                ui.interact(
                    rect,
                    ui.id().with("character-canvas"),
                    egui::Sense::click_and_drag(),
                )
            })
            .inner;
        if response.clicked() {
            self.selected_actor = response
                .interact_pointer_pos()
                .and_then(|p| direct_manipulation::pick(rect, scene, p));
        }
        if response.drag_started() {
            if let Some(pointer) = ui.input(|i| i.pointer.press_origin()) {
                self.selected_actor = direct_manipulation::pick(rect, scene, pointer);
                if let Some(actor) = self.selected_actor.clone() {
                    self.flush_text_edit();
                    self.character_drag = direct_manipulation::Drag::new(
                        &self.source,
                        self.scene_index,
                        &actor,
                        self.playhead,
                        rect,
                        scene,
                        pointer,
                    )
                    .ok();
                    if self.character_drag.is_some() {
                        self.begin_drag_recording(scene, &actor);
                    }
                }
            }
        }
        if let Some(actor) = scene
            .actors
            .iter()
            .find(|actor| Some(&actor.id) == self.selected_actor.as_ref())
        {
            ui.painter().with_clip_rect(rect).rect_stroke(
                direct_manipulation::actor_bounds(rect, scene, actor),
                3.0,
                egui::Stroke::new(2.0_f32, ACCENT),
                egui::StrokeKind::Inside,
            );
        }
        let pointer_delta = self.character_drag.as_ref().and_then(|drag| {
            ui.input(|i| i.pointer.interact_pos())
                .and_then(|p| drag.delta(scene, p))
        });
        if response.drag_stopped() {
            if let Some(drag) = self.character_drag.take() {
                self.drag_recorder.release(pointer_delta);
                if let Some(pointer) = ui.input(|i| i.pointer.interact_pos()) {
                    self.finish_character_drag(drag, scene, rect, pointer);
                }
            }
        } else if self.character_drag.is_some() && !ui.input(|i| i.pointer.primary_down()) {
            self.character_drag = None;
        } else if let Some(delta) = pointer_delta {
            self.drag_recorder.update(delta);
        }
        if self.character_drag.is_none() {
            // Escape, a lost gesture or a stale drag still ends with one release.
            self.drag_recorder.cancel();
        }
        if self.character_drag.is_some() {
            ui.ctx().request_repaint();
        }
    }
    /// One validated edit, preserving the artist's selection and scene position.
    fn apply_artist_source(&mut self, result: Result<String, String>, success: &str) {
        if !self.editing_idle() || self.setup_busy || self.scene_playing() {
            return;
        }
        let actor = self.selected_actor.clone();
        let time = self.playhead;
        match result {
            Ok(source) => {
                self.apply_structure(Ok(source.clone()));
                if self.source == source {
                    self.selected_actor = actor;
                    self.playhead = time;
                    if let Some(clock) = &mut self.scene_transport {
                        let _ = clock.seek_seconds(time);
                    }
                    self.status = success.into();
                }
            }
            Err(error) => self.status = format!("Muutos epäonnistui: {error}"),
        }
    }
    fn apply_hand_prop(&mut self, remove: bool) {
        if !self.editing_idle() || self.setup_busy || self.scene_playing() {
            return;
        }
        let result = (|| {
            let actor = self
                .selected_actor
                .as_deref()
                .ok_or("Valitse hahmo ensin.")?;
            if remove {
                hand_props::remove(&self.source, actor, self.hand_props.hand)
            } else {
                let catalog = hand_props::catalog()?;
                let item = catalog
                    .items
                    .get(self.hand_props.item_index)
                    .ok_or("Valitse esine.")?;
                hand_props::attach(
                    &self.source,
                    actor,
                    self.hand_props.hand,
                    item,
                    self.hand_props.placement,
                )
            }
        })();
        self.apply_artist_source(result, if remove {
            "Käsiesine poistettu hahmon kaikista kohtauksista. Kumoa palauttaa sen."
        } else {
            "Esine kiinnitetty käteen hahmon kaikissa kohtauksissa. Käden nivelet liikuttavat sitä. Kumoa peruu otteen."
        });
    }
    fn hand_props_ui(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.label(egui::RichText::new("Esine käteen").strong());
        let catalog = match hand_props::catalog() {
            Ok(catalog) => catalog,
            Err(error) => {
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    format!("Esineluetteloa ei voitu avata: {error}"),
                );
                return;
            }
        };
        let Some(rig) = self
            .script
            .as_ref()
            .and_then(|s| {
                self.selected_actor
                    .as_deref()
                    .and_then(|a| s.characters.get(a))
            })
            .and_then(|c| c.rig.as_ref())
        else {
            ui.weak("Valitse mukautettu nivelhahmo näyttämöltä tai vasemmalta.");
            return;
        };
        let problem = hand_props::hand_problem(rig, self.hand_props.hand);
        let has_prop = rig
            .parts
            .iter()
            .any(|p| p.id.starts_with(self.hand_props.hand.prefix()));
        let enabled = self.editing_idle() && !self.setup_busy && !self.scene_playing();
        let mut attach = false;
        let mut remove = false;
        ui.add_enabled_ui(enabled, |ui| {
            egui::ComboBox::from_id_salt("hand_prop_item")
                .selected_text(
                    catalog
                        .items
                        .get(self.hand_props.item_index)
                        .map_or("Valitse esine", |i| i.name.as_str()),
                )
                .show_ui(ui, |ui| {
                    for (i, item) in catalog.items.iter().enumerate() {
                        ui.selectable_value(&mut self.hand_props.item_index, i, &item.name);
                    }
                });
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.hand_props.hand,
                    hand_props::Hand::Left,
                    hand_props::Hand::Left.label(),
                );
                ui.selectable_value(
                    &mut self.hand_props.hand,
                    hand_props::Hand::Right,
                    hand_props::Hand::Right.label(),
                );
            });
            if let Some(problem) = &problem {
                ui.weak(problem);
            }
            egui::CollapsingHeader::new("Säädä otetta").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("X");
                    ui.add(
                        egui::DragValue::new(&mut self.hand_props.placement.x)
                            .range(-100.0..=100.0),
                    );
                    ui.label("Y");
                    ui.add(
                        egui::DragValue::new(&mut self.hand_props.placement.y)
                            .range(-100.0..=100.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Kierto °");
                    ui.add(
                        egui::DragValue::new(&mut self.hand_props.placement.rotation_degrees)
                            .range(-180.0..=180.0),
                    );
                    ui.label("Koko");
                    ui.add(
                        egui::DragValue::new(&mut self.hand_props.placement.scale)
                            .speed(0.01)
                            .range(0.25..=3.0),
                    );
                });
                if ui.button("Palauta oletusote").clicked() {
                    self.hand_props.placement = Default::default();
                }
            });
            ui.horizontal(|ui| {
                attach = ui
                    .add_enabled(
                        problem.is_none(),
                        egui::Button::new(if has_prop {
                            "Vaihda esine / ote"
                        } else {
                            "Kiinnitä käteen"
                        }),
                    )
                    .clicked();
                remove = ui
                    .add_enabled(has_prop, egui::Button::new("Poista käsiesine"))
                    .clicked();
            });
        });
        ui.small("Ote koskee tätä hahmoa kaikissa kohtauksissa. Valitse puhelimesta näyttö-, taka- tai vinonäkymä. Pysäytä toisto ennen muutosta.");
        if attach || remove {
            self.apply_hand_prop(remove);
        }
    }
    fn apply_camera_shot(&mut self, shot: camera_framing::Shot) {
        if !self.editing_idle() || self.setup_busy || self.scene_playing() {
            return;
        }
        let result = camera_framing::apply(
            &self.source,
            self.scene_index,
            self.selected_actor.as_deref(),
            self.playhead,
            shot,
        );
        self.apply_artist_source(
            result,
            "Kuvakoon avainruutu lisätty toistopään kohdalle. Kumoa peruu rajauksen.",
        );
    }
    fn screen_cues_ui(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        egui::CollapsingHeader::new("Näyttö: kuva tai video puhelimessa").show(ui, |ui| {
            let Some(script) = self.script.as_ref() else {
                ui.weak("Korjaa käsikirjoitus ensin.");
                return;
            };
            let images: Vec<String> = self
                .project_document
                .as_ref()
                .map(|d| {
                    d.assets
                        .iter()
                        .filter(|a| a.kind == motion_project::AssetKind::Image)
                        .map(|a| a.id.clone())
                        .collect()
                })
                .unwrap_or_default();
            let keys = script
                .scenes
                .get(self.scene_index)
                .map(screen_video_worker::VideoKey::of_scene)
                .unwrap_or_default();
            let status = self.screen_videos.status(&keys);
            let has_failures = self
                .screen_videos
                .entries
                .values()
                .any(|s| matches!(s, screen_video_worker::DecodeState::Failed(_)));
            let view = screen_cues::View {
                script,
                scene_index: self.scene_index,
                playhead: self.playhead,
                images: &images,
                enabled: self.editing_idle() && !self.setup_busy && !self.scene_playing(),
                importing: self.screen_videos.import.is_some(),
                decode_status: &status,
                has_failures,
            };
            if let Some(action) = screen_cues::ui(ui, &mut self.screen_cues, view) {
                self.screen_cue_action(action);
            }
            ui.small("Videon tuonti ja esikatselun purku käyttävät samaa FFmpeg-polkua kuin videon vienti (ffprobe samasta kansiosta).");
            field(ui, "Paikallinen FFmpeg-ohjelma (koko polku)", &mut self.ffmpeg_path);
        });
    }
    fn screen_cue_action(&mut self, action: screen_cues::Action) {
        use screen_cues::Action;
        match action {
            Action::Apply(draft) => {
                let result = draft.to_cue().and_then(|cue| match draft.editing {
                    Some(index) => screen_cues::replace(&self.source, self.scene_index, index, cue),
                    None => screen_cues::add(&self.source, self.scene_index, cue),
                });
                self.apply_screen_cue_source(result, "Näyttömuutos tallennettu. Kumoa peruu sen.");
            }
            Action::Remove(index) => {
                let result = screen_cues::remove(&self.source, self.scene_index, index);
                if self
                    .apply_screen_cue_source(result, "Näyttömuutos poistettu. Kumoa palauttaa sen.")
                {
                    self.screen_cues.draft = None;
                }
            }
            Action::ImportVideo(path) => {
                let target = self.screen_cues.draft.clone().or_else(|| {
                    let slots = screen_cues::slots(self.script.as_ref()?);
                    let (actor, slot) = slots.get(self.screen_cues.slot_index)?;
                    Some(screen_cues::Draft::new(actor, slot, self.playhead))
                });
                match self.screen_videos.start_import(
                    Path::new(self.project_root.trim()),
                    Path::new(&path),
                    &self.ffmpeg_path,
                ) {
                    Ok(_) => {
                        self.screen_import_target = target;
                        self.screen_cues.error = None;
                        self.status = "Tuodaan videota taustalla…".into();
                    }
                    Err(error) => self.screen_cues.error = Some(error),
                }
            }
            Action::CancelImport => {
                self.screen_videos.cancel_import();
                self.screen_import_target = None;
                self.status = "Videon tuonti peruttiin.".into();
            }
            Action::RetryDecode => self.screen_videos.retry_failed(),
        }
    }
    /// Apply a screen-cue edit on the undoable screenplay path. Errors stay
    /// inline in the panel. Returns whether the source changed.
    fn apply_screen_cue_source(&mut self, result: Result<String, String>, success: &str) -> bool {
        match result {
            Ok(source) => {
                let before = self.source.clone();
                self.apply_artist_source(Ok(source.clone()), success);
                let applied = self.source == source && self.source != before;
                if applied {
                    self.screen_cues.error = None;
                    let last = self
                        .script
                        .as_ref()
                        .and_then(|s| s.scenes.get(self.scene_index))
                        .map(|s| s.composition.screens.len().saturating_sub(1));
                    if let Some(draft) = &mut self.screen_cues.draft {
                        // A new cue is appended last; keep editing it.
                        if draft.editing.is_none() {
                            draft.editing = last;
                        }
                    }
                } else if self.source != source {
                    self.screen_cues.error = Some(self.status.clone());
                }
                applied
            }
            Err(error) => {
                self.screen_cues.error = Some(error);
                false
            }
        }
    }
    fn poll_screen_videos(&mut self) {
        let Some(result) = self.screen_videos.poll() else {
            return;
        };
        match result {
            Ok(imported) => {
                let mut draft = self
                    .screen_import_target
                    .take()
                    .unwrap_or_else(|| screen_cues::Draft::new("", "", self.playhead));
                draft.editing = None;
                draft.set_video(&imported.asset, imported.fps, imported.duration);
                let message = format!(
                    "Video tuotu: {} ({} fps, {:.3} s).",
                    imported.asset, imported.fps, imported.duration
                );
                self.screen_cues.draft = Some(draft.clone());
                if !self.editing_idle() || self.setup_busy || self.scene_playing() {
                    self.screen_cues.error = Some(format!(
                        "{message} Pysäytä toisto ja lisää näyttömuutos painikkeella."
                    ));
                    return;
                }
                let result = draft
                    .to_cue()
                    .and_then(|cue| screen_cues::add(&self.source, self.scene_index, cue));
                self.apply_screen_cue_source(
                    result,
                    &format!("{message} Videomuutos lisätty; Kumoa peruu sen."),
                );
            }
            Err(error) => {
                self.screen_import_target = None;
                self.screen_cues.error = Some(format!("Videon tuonti epäonnistui: {error}"));
            }
        }
    }
    fn camera_framing_ui(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.label(egui::RichText::new("Kamera ja kuvakoko").strong());
        let enabled = self.editing_idle() && !self.setup_busy && !self.scene_playing();
        let mut selected = None;
        ui.add_enabled_ui(enabled, |ui| {
            ui.horizontal_wrapped(|ui| {
                for shot in camera_framing::Shot::ALL {
                    if ui.button(shot.label()).clicked() {
                        selected = Some(shot);
                    }
                }
            });
        });
        ui.small("Rajaa 2D-näkymää valitun hahmon tai käsiesineen ympärille. Avainruutu tulee nykyiseen aikaan; aiemmat avainruudut säilyvät.");
        if let Some(shot) = selected {
            self.apply_camera_shot(shot);
        }
    }

    fn head_controls_ui(&mut self, ui: &mut egui::Ui) {
        let Some((script, actor)) = self.script.as_ref().zip(self.selected_actor.as_deref()) else {
            self.head_draft = None;
            self.webcam_pose.set_context("no selection");
            return;
        };
        let Ok(at) = direct_manipulation::snapped_time(script, self.scene_index, self.playhead)
        else {
            self.head_draft = None;
            self.webcam_pose.set_context("invalid playhead");
            return;
        };
        if !self.head_draft.as_ref().is_some_and(|(source, draft)| {
            source == &self.source
                && draft.scene_index == self.scene_index
                && draft.actor == actor
                && draft.at == at
        }) {
            self.head_draft =
                direct_manipulation::HeadDraft::new(script, self.scene_index, actor, self.playhead)
                    .ok()
                    .map(|draft| (self.source.clone(), draft));
        }
        let enabled = self.editing_idle() && !self.scene_playing();
        let strict = self.model_policy.is_strict();
        let context = format!(
            "{}:{}:{}:{}:{}:{}",
            self.document_revision, self.scene_index, actor, at, enabled, strict
        );
        self.webcam_pose.set_context(&context);
        let Some((source, draft)) = &mut self.head_draft else {
            self.webcam_pose.set_context("no head joint");
            ui.weak("Pään ohjaus tarvitsee mukautettuun hahmoon head-nimisen nivelen.");
            return;
        };
        ui.separator();
        ui.label("Pään asento · avainruutu toistopään kohdalle");
        let mut apply = false;
        ui.add_enabled_ui(enabled, |ui| {
            ui.horizontal(|ui| { ui.label("X"); ui.add(egui::DragValue::new(&mut draft.x)); ui.label("Y"); ui.add(egui::DragValue::new(&mut draft.y)); });
            if draft.supports_tilt {
                ui.horizontal(|ui| { ui.label("Kallistus °"); ui.add(egui::DragValue::new(&mut draft.tilt_degrees)); });
                if strict {
                    // Apple Vision face tracking is model-backed (ADR-A01-06): not started.
                    ui.weak(policy_block_text(motion_project::ModelBackedStep::CameraFaceTracking));
                } else {
                    self.webcam_pose.ui(ui);
                }
                if let Some(degrees) = self.webcam_pose.take_roll_degrees() { draft.tilt_degrees = degrees; }
            } else { ui.weak("Oletushahmon päätä voi siirtää X/Y-suunnassa. Kamerakallistus tarvitsee mukautetun hahmon head-nivelen."); }
            apply = ui.button("Lisää pään avainruutu").clicked();
        });
        if let Some(samples) = self.webcam_pose.take_recording() {
            let actor = draft.actor.clone();
            let expected = source.clone();
            let time = self.playhead;
            if enabled && expected == self.source {
                match direct_manipulation::apply_head_recording(
                    &self.source,
                    self.scene_index,
                    &actor,
                    time,
                    &samples,
                ) {
                    Ok(source) => {
                        self.apply_structure(Ok(source));
                        self.selected_actor = Some(actor);
                        self.playhead = time;
                        self.status = format!(
                            "Kameranauhoitus lisätty: {} mittausta. Kumoa peruu koko nauhoituksen.",
                            samples.len()
                        );
                    }
                    Err(error) => self.status = format!("Pään muokkaus epäonnistui: {error}"),
                }
            }
            return;
        }
        if apply && enabled && source == &self.source {
            let actor = draft.actor.clone();
            let time = self.playhead;
            match draft.apply(&self.source) {
                Ok(source) => {
                    self.apply_structure(Ok(source));
                    self.selected_actor = Some(actor);
                    self.playhead = time;
                }
                Err(error) => self.status = format!("Pään muokkaus epäonnistui: {error}"),
            }
        }
    }

    fn native_canvas_ui(&mut self, ui: &mut egui::Ui) {
        if let Some(document) = &self.project_document {
            self.art_resources
                .sync(ui.ctx(), &self.project_root, &document.assets);
        }
        self.sync_sequence_resources();
        let head_preview = self.live_head_preview();
        let Some(script) = self.script.as_ref() else {
            return;
        };
        let Some(scene) = script.scenes.get(self.scene_index) else {
            return;
        };
        let total = (scene.duration * 24000.0).round() as u64;
        if total == 0 || !self.playhead.is_finite() {
            ui.label("Esikatselun aikakello on virheellinen.");
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
        let screen_keys = screen_video_worker::VideoKey::of_scene(scene);
        self.screen_videos.sync(
            &screen_keys,
            Path::new(self.project_root.trim()),
            &self.ffmpeg_path,
        );
        let screen_video_revision = self.screen_videos.revision;
        let changed = self.native_canvas.as_ref().is_none_or(|canvas| {
            canvas.revision != self.sequence_revision
                || canvas.screen_video_revision != screen_video_revision
                || (canvas.head_preview != head_preview
                    && (canvas.last_requested == canvas.shown || canvas.failed.is_some()))
                || canvas.request.scene_index != self.scene_index
                || canvas.request.captions != self.caption_mode
                || !same_optional_arc(&canvas.request.manifest, &manifest)
                || !same_optional_arc(&canvas.line_audio, &line_audio)
        });
        if changed {
            let previous_texture = self
                .native_canvas
                .as_ref()
                .filter(|c| {
                    c.revision == self.sequence_revision
                        && c.request.scene_index == self.scene_index
                })
                .and_then(|c| c.texture.clone());
            let mut preview_script = script.clone();
            if let Some((actor, tilt)) = &head_preview {
                let _ = direct_manipulation::stage_head_preview(
                    &mut preview_script,
                    self.scene_index,
                    actor,
                    self.playhead,
                    *tilt,
                );
            }
            self.native_canvas_generation = self.native_canvas_generation.wrapping_add(1);
            self.native_canvas = Some(NativeCanvas {
                revision: self.sequence_revision,
                request: native_compositor::Request {
                    generation: self.native_canvas_generation,
                    scene_index: self.scene_index,
                    sample_frame: sample,
                    script: Arc::new(preview_script),
                    captions: self.caption_mode,
                    art: Arc::new(motion_export::ArtContext {
                        document: self
                            .project_document
                            .as_ref()
                            .map(|p| p.art.clone())
                            .unwrap_or_default(),
                        images: (*self.art_resources.images).clone(),
                        // Proxies decoded off the UI thread; frames load per instant.
                        screen_videos: self.screen_videos.decoded(&screen_keys),
                    }),
                    manifest,
                    mouths: None,
                },
                line_audio,
                head_preview,
                last_requested: None,
                shown: None,
                failed: None,
                texture: previous_texture,
                error: None,
                screen_video_revision,
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
                    // Unrenderable captions are reported, never silently dropped.
                    if e.contains("caption") {
                        self.status = format!("Esikatselu: {e}");
                    }
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
        ui.small("Kohtauksen esikatselu käyttää projektin maskeja, tasoja, kameraa ja suun ajoitusta. Kuvituksen työkalut löytyvät Muokkaa-välilehdeltä.");
        let canvas_rect = if let Some(texture) = &canvas.texture {
            let scale = (ui.available_width() / texture.size_vec2().x).min(1.0);
            Some(ui.image((texture.id(), texture.size_vec2() * scale)).rect)
        } else {
            ui.spinner();
            None
        };
        for line in self.screen_videos.status(&screen_keys) {
            ui.colored_label(egui::Color32::from_rgb(220, 180, 90), line);
        }
        if let Some(error) = &canvas.error {
            ui.colored_label(egui::Color32::LIGHT_RED, format!("Virhe: {error}"));
            if ui.button("Yritä esikatselua uudelleen").clicked() {
                canvas.failed = None;
                canvas.last_requested = None;
                ui.ctx().request_repaint();
            }
        }
        if canvas.shown != Some(sample) && canvas.failed.is_none() {
            ui.ctx().request_repaint_after(Duration::from_millis(25));
        }
        let interaction = canvas_rect.and_then(|rect| {
            motion_scene::evaluate(script, self.scene_index, self.playhead)
                .ok()
                .map(|evaluated| {
                    (
                        rect,
                        evaluated,
                        scene.id.clone(),
                        !self.busy
                            && !self.diagnostics_busy
                            && !self.export_busy
                            && !playing
                            && canvas.shown == Some(sample),
                    )
                })
        });
        if let Some((rect, mut evaluated, scene_id, enabled)) = interaction {
            if self.character_mode {
                self.preview_character_drag(&mut evaluated, ui.ctx());
                self.character_canvas_ui(ui, rect, &evaluated, enabled);
            } else if let Some(document) = &mut self.project_document {
                let response = ui
                    .add_enabled_ui(enabled, |ui| {
                        ui.interact(rect, ui.id().with("art-canvas"), egui::Sense::drag())
                    })
                    .inner;
                self.art_editor
                    .gesture(&response, rect, &evaluated, &scene_id, &mut document.art);
            }
        }
        self.record_art_transaction();
    }

    fn scene_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Näyttämö");
            if let Some(script) = &self.script {
                if let Some(scene) = script.scenes.get(self.scene_index) {
                    ui.weak(format!(
                        "{}  /  {} × {}  /  {} fps",
                        scene.id, script.project.width, script.project.height, script.project.fps
                    ));
                }
            }
        });
        ui.horizontal(|ui| {
            let before = self.character_mode;
            ui.selectable_value(&mut self.character_mode, true, "Hahmot");
            ui.selectable_value(&mut self.character_mode, false, "Kuvitus");
            if before != self.character_mode {
                self.character_drag = None;
                self.art_editor.reset();
            }
        });
        ui.weak(if self.character_mode {
            "Vedä hahmoa lisätäksesi avainruudun nykyiseen aikaan. Siirrä toistopäätä ja vedä uudelleen. Esc peruu vedon."
        } else {
            "Kuvituksen työkalut löytyvät oikealta Muokkaa-välilehdeltä."
        });
        ui.add_space(12.0);
        if self.advanced_canvas_needed() {
            self.native_canvas_ui(ui);
            return;
        }
        if let Some(mut script) = self.script.clone() {
            self.stage_live_head(&mut script);
            match motion_scene::evaluate(&script, self.scene_index, self.playhead) {
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
                    let owned_mouths: Vec<_> = mouths
                        .into_iter()
                        .map(|(id, mouth)| (id.to_owned(), mouth))
                        .collect();
                    let mouths: Vec<_> = owned_mouths
                        .iter()
                        .map(|(id, mouth)| (id.as_str(), *mouth))
                        .collect();
                    motion_scene::apply_speech(&mut scene, &mouths);
                    let scene_id = &script.scenes[self.scene_index].id;
                    self.preview_character_drag(&mut scene, ui.ctx());
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
                    if self.character_mode {
                        let enabled = self.editing_idle() && !self.scene_playing();
                        self.character_canvas_ui(ui, rect, &scene, enabled);
                    } else if let Some(document) = &mut self.project_document {
                        let enabled = !self.busy && !self.diagnostics_busy && !self.export_busy;
                        let response = ui
                            .add_enabled_ui(enabled, |ui| {
                                ui.interact(rect, ui.id().with("art-canvas"), egui::Sense::drag())
                            })
                            .inner;
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
            ui.label("Paina Tarkista, jotta voit esikatsella kohtausta.");
        }
        self.record_art_transaction();
        ui.add_space(8.0);
        ui.weak("Kohtauksen esikatselu · suun liike seuraa äänikelloa");
    }
    fn piper_profile(&self) -> Option<std::path::PathBuf> {
        let p = self.piper_config_path.trim();
        (!p.is_empty()).then(|| p.into())
    }
    fn set_piper_profile(&mut self, path: String) {
        if path == self.piper_config_path {
            return;
        }
        self.piper_config_path = path;
        self.stop_playback();
        // Retain the cancelled token and busy state until the owning worker
        // acknowledges it. A completed result already in its channel must also
        // be rejected before a new preparation can replace that token.
        if let Some(cancel) = &self.active_cancel {
            cancel.cancel();
        }
        self.sequence_changed();
        self.scene_preview = None;
        self.scene_audio_active = false;
        self.preview = None;
        self.native_alignment.invalidate();
        self.alignment_identity = None;
        self.alignment_audio = None;
        self.status =
            "Piper-profiili vaihtui. Valmistele ääni uudelleen ennen toistoa tai vientiä.".into();
    }
    fn local_provider(&self) -> motion_tts::LocalSpeechProvider {
        let provider = motion_tts::LocalSpeechProvider::new(self.kokoro_config());
        match self.piper_profile() {
            Some(path) => provider.with_piper_config_path(path),
            None => provider,
        }
    }
    fn start_measured_direction(&mut self) {
        if !self.editing_idle() || self.setup_busy {
            return;
        }
        self.flush_text_edit();
        self.stop_playback();
        let root = std::path::PathBuf::from(self.project_root.trim());
        if !root.is_absolute() || !root.is_dir() {
            self.status =
                "Anna olemassa olevan projektikansion koko polku ennen puheohjausta.".into();
            return;
        }
        self.direction_generation = self.direction_generation.wrapping_add(1);
        let generation = self.direction_generation;
        let cancel = CancellationToken::new();
        let imported_audio = self
            .direction_imports
            .iter()
            .filter(|(_, p)| !p.trim().is_empty())
            .map(|(line, p)| (*line, p.trim().into()))
            .collect();
        let request = direction_worker::Request {
            generation,
            source: self.source.clone(),
            dialogue: self.quick_start.dialogue.clone(),
            root,
            provider: self.speech_provider(),
            imported_audio,
            cancel: cancel.clone(),
            story: None,
            story_project: Default::default(),
        };
        match self.direction_worker.submit(request) {
            Ok(()) => {
                self.direction_pending = Some(PendingDirection {
                    generation,
                    revision: self.document_revision,
                    source: self.source.clone(),
                    dialogue: self.quick_start.dialogue.clone(),
                    root: self.project_root.clone(),
                    piper_profile: self.piper_config_path.clone(),
                });
                self.active_cancel = Some(cancel);
                self.busy = true;
                self.status = "Valmistellaan oikea puhe ja mitataan repliikkien kestot…".into();
            }
            Err(error) => self.status = format!("Puheohjauksen käynnistys epäonnistui: {error}"),
        }
    }
    fn poll_direction(&mut self) {
        while let Some(update) = self.direction_worker.poll() {
            match update {
                direction_worker::Update::Progress {
                    generation,
                    completed,
                    total,
                    message,
                } => {
                    if self
                        .direction_pending
                        .as_ref()
                        .is_some_and(|p| p.generation == generation)
                    {
                        self.status = format!("Puheohjaus {completed}/{total}: {message}");
                    }
                }
                direction_worker::Update::StoryFinished { generation, result } => {
                    if !self
                        .direction_pending
                        .as_ref()
                        .is_some_and(|p| p.generation == generation)
                    {
                        continue;
                    }
                    let pending = self.direction_pending.take().expect("matched pending");
                    let cancelled = self
                        .active_cancel
                        .as_ref()
                        .is_some_and(|t| t.is_cancelled());
                    self.active_cancel = None;
                    self.busy = false;
                    if cancelled {
                        self.status = "Puheen valmistelu peruttu. Projektia ei muutettu.".into();
                        continue;
                    }
                    if pending.revision != self.document_revision
                        || pending.source != self.source
                        || pending.dialogue != self.story_text
                        || pending.root != self.project_root
                        || pending.piper_profile != self.piper_config_path
                    {
                        self.status =
                            "Projekti tai käsikirjoitus vaihtui kesken; tulos hylättiin.".into();
                        continue;
                    }
                    match result {
                        Ok(ready) => {
                            let elapsed = ready.elapsed.as_secs_f64();
                            self.apply_story(ready.built);
                            self.status =
                                format!("{} Puheen valmistelu {elapsed:.1} s.", self.status);
                        }
                        Err(error) => self.status = format!("Tarinan puhe: {error}"),
                    }
                }
                direction_worker::Update::Finished { generation, result } => {
                    if !self
                        .direction_pending
                        .as_ref()
                        .is_some_and(|p| p.generation == generation)
                    {
                        continue;
                    }
                    let pending = self.direction_pending.take().expect("matched pending");
                    let cancelled = self
                        .active_cancel
                        .as_ref()
                        .is_some_and(|t| t.is_cancelled());
                    self.active_cancel = None;
                    self.busy = false;
                    if cancelled {
                        self.status = "Puheohjaus peruttu. Projektia ei muutettu.".into();
                        continue;
                    }
                    if pending.revision != self.document_revision
                        || pending.source != self.source
                        || pending.dialogue != self.quick_start.dialogue
                        || pending.root != self.project_root
                        || pending.piper_profile != self.piper_config_path
                    {
                        self.status =
                            "Projekti tai vuorosanat vaihtuivat; ohjaustulos hylättiin.".into();
                        continue;
                    }
                    match result {
                        Ok(ready) if ready.original_source == self.source => {
                            let expected = ready.result.source.clone();
                            let first = ready.result.first_scene;
                            let shots = ready.result.shot_count;
                            let elapsed = ready.elapsed.as_secs_f64();
                            self.direction_report = ready
                                .audio
                                .iter()
                                .map(|audio| {
                                    format!(
                                        "Rivi {} · {} · {} · {} näytettä · SHA-256 {}",
                                        audio.source_line,
                                        audio.actor,
                                        audio.relative_path,
                                        audio.sample_count,
                                        audio.wav_sha256
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            self.apply_structure(Ok(expected.clone()));
                            if self.source == expected {
                                self.scene_index = first;
                                self.reset_scene();
                                self.inspector_tab = InspectorTab::Export;
                                self.status=format!("Puheeseen ajoitettu ohjaus valmis: {shots} otosta, {} varmennettua WAVia, {:.1} s valmistelu. Valmistele Koko projekti / kohtauskooste esikatselua ja vientiä varten. Kumoa peruu lähdemuutoksen; WAVit säilyvät projektikansiossa.",ready.audio.len(),elapsed);
                            }
                        }
                        Ok(_) => {
                            self.status =
                                "Ohjaustulos oli vanhasta lähteestä; tulos hylättiin.".into()
                        }
                        Err(error) => self.status = format!("Puheohjaus: {error}"),
                    }
                }
            }
        }
    }
    fn live_head_preview(&self) -> Option<(String, f64)> {
        if !self.character_mode
            || self.inspector_tab != InspectorTab::Edit
            || !self.editing_idle()
            || self.scene_playing()
        {
            return None;
        }
        self.selected_actor
            .clone()
            .zip(self.webcam_pose.live_roll_degrees().map(|v| v.round()))
    }
    fn stage_live_head(&self, script: &mut motion_script::Script) {
        if let Some((actor, tilt)) = self.live_head_preview() {
            let _ = direct_manipulation::stage_head_preview(
                script,
                self.scene_index,
                &actor,
                self.playhead,
                tilt,
            );
        }
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
            piper_config: self.piper_profile(),
            alignments: self.scene_alignment.clone(),
            policy: self.model_policy,
        };
        match self.scene_worker.submit(request) {
            Ok(()) => {
                self.active_cancel = Some(cancel);
                self.busy = true;
                self.status = "Valmistellaan kohtauksen ääntä taustalla…".into();
            }
            Err(error) => self.status = format!("Kohtauksen äänen valmistelu epäonnistui: {error}"),
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
        if job.imported_audio.is_none()
            && !self.permit_model(motion_project::ModelBackedStep::SpeechSynthesis)
        {
            return;
        }
        let provider = self.speech_provider();
        self.scene_preview = None;
        self.scene_audio_active = false;
        let worker = VoiceWorker::new(provider, 1);
        let cancel = CancellationToken::new();
        match worker.submit_cancellable(job.clone(), cancel.clone()) {
            Ok(()) => {
                self.active_cancel = Some(cancel);
                self.pending_job = Some(job);
                self.worker = Some(worker);
                self.busy = true;
                self.preview = None;
                self.status =
                    "Valmistelu jonossa. Käsikirjoitukseen asetettu WAV-äänite ohittaa puhesynteesin.".into();
            }
            Err(e) => self.status = format!("Repliikin valmistelu epäonnistui: {e}"),
        }
    }
    fn kokoro_diagnostic_presets(&self) -> std::collections::BTreeSet<String> {
        let script = motion_script::parse(&self.source);
        let mut presets = std::collections::BTreeSet::new();
        if let Ok(script) = script {
            for character in script.characters.values() {
                if let Some(voice) = character.voice.as_ref().filter(|v| v.provider == "kokoro") {
                    presets.insert(voice.preset.clone());
                }
            }
        }
        if presets.is_empty() {
            presets.insert("af_heart".into());
        }
        presets
    }
    fn check_runtime(&mut self) {
        let presets = self.kokoro_diagnostic_presets();
        if presets
            .iter()
            .any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        {
            self.diagnostics_report = Some(Err(
                "Käsikirjoituksen puheäänen tunnus on virheellinen. Tarkista se ennen paikallisten resurssien tarkistusta."
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
        self.poll_screen_videos();
        self.poll_direction();
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
                        self.status = "Resurssiluettelo päivitetty; tallenna .kilsat säilyttääksesi muutokset. Viitteen poistaminen ei poista tiedostoa levyltä.".into();
                    }
                    Ok(_) => self.status = "Projekti vaihtui resurssityön aikana; luettelon muutos hylättiin. Tuodut tiedostot säilyvät levyllä.".into(),
                    Err(error) => self.status = format!("Resurssityö: {error}"),
                }
            }
        }

        while let Some(update) = self.exporter.poll() {
            match update {
                export_worker::Update::Progress(progress) => {
                    self.export_progress = Some(progress);
                    self.status = format!(
                        "Vienti: {}/{} kuvaruutua",
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
                            "Viety {:?} ({} kuvaruutua), scene.wav ja ajoitustiedot kansioon {}",
                            self.export_format,
                            report.frame_count,
                            report.output_directory.display()
                        ),
                        Err(error) => format!("Kohtauksen vienti: {error}"),
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
                        "Kohtauksen ääni: {}/{} · repliikki {} · {}",
                        p.completed, p.total, p.dialogue_id, p.actor
                    )
                }
                scene_worker::Update::Finished(result) => {
                    self.busy = false;
                    if self.active_cancel.take().is_some_and(|c| c.is_cancelled()) {
                        self.status =
                            "Kohtauksen valmistelu peruttiin. Voit yrittää uudelleen.".into();
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
                                    "Vanhentuneen kohtauksen valmistelutulos hylättiin.".into();
                                continue;
                            }
                            let diagnostics: Vec<String> =
                                prepared.diagnostics.iter().map(|d| d.message()).collect();
                            let audio = prepared.rendered.audio;
                            self.status = format!(
                                "Kohtaus valmis: {} repliikkiä · {:.3} s",
                                prepared.clips.len(),
                                audio.duration_seconds()
                            );
                            self.scene_preview = Some(ScenePreview {
                                scene: prepared.scene_id,
                                alignment_statuses: ready.alignment_statuses,
                                diagnostics,
                                clips: prepared.clips,
                                audio: Arc::new(audio),
                                manifest: Arc::new(prepared.rendered.manifest),
                                peaks: ready.peaks,
                            });
                            self.playhead = 0.0;
                        }
                        Err(error) => self.status = format!("Kohtauksen ääni: {error}"),
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
                self.status = "Luodaan tai tuodaan ääntä taustalla…".into()
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
                                pronunciations: self.script_pronunciations(),
                                cancel: self.active_cancel.clone().unwrap_or_default(),
                            }) {
                                Ok(()) => {
                                    self.status =
                                        "Valmistellaan aaltomuotoa ja suun ajoitusta…".into()
                                }
                                Err(e) => {
                                    self.status = format!("Äänen valmistelu epäonnistui: {e}");
                                    self.busy = false;
                                    self.active_cancel = None;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        self.status = format!("Äänen valmistelu epäonnistui: {e}");
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
                self.status = "Repliikin valmistelu peruttiin. Voit yrittää uudelleen.".into();
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
                        self.status = "Repliikki ylittää valitun kohtauksen keston tai kuuluu vanhentuneeseen kohtaukseen. Pidennä kohtausta tai siirrä repliikkiä, paina Tarkista ja valmistele ääni uudelleen.".into();
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
                        "Valmis: {:.3} s. {}",
                        preview.result.duration_seconds(),
                        if preview.result.cached {
                            "Ladattu sisältövälimuistista."
                        } else {
                            "Äänitiedosto valmisteltu."
                        }
                    );
                    self.preview = Some(preview);
                }
                Err(e) => self.status = format!("Äänen ja ajoituksen esikatselu: {e}"),
            }
        }
    }
}
fn alignment_status_text(status: AlignmentStatus) -> &'static str {
    match status {
        AlignmentStatus::EstimatedImported => {
            "Rhubarb-arvio tuotu · erillinen äännevälimuistista · laatu mittaamatta"
        }
        AlignmentStatus::None => "Ei tuotua puhekohdistusta",
        AlignmentStatus::LegacyUnbound => {
            "Vanha ajoitus · ei sidottu ääneen ja tekstiin · laatu mittaamatta"
        }
        AlignmentStatus::BoundImported => {
            "Ääneen sidottu ajoitus tuotu ja tallennettu välimuistiin · laatu mittaamatta"
        }
        AlignmentStatus::BoundCacheHit => {
            "Ääneen sidottu ajoitus löytyi välimuistista · laatu mittaamatta"
        }
        AlignmentStatus::BoundCacheMiss => "Ääneen sidottua ajoitusta ei löytynyt välimuistista",
    }
}
fn mouth_source_text(source: motion_voice::timeline::MouthSource) -> &'static str {
    use motion_voice::timeline::MouthSource;
    match source {
        MouthSource::Manual => "Manuaalinen suun ajoitus · laatu arvioimatta",
        MouthSource::Forced => "Ulkoisella työkalulla kohdistettu · laatu arvioimatta",
        MouthSource::RhubarbEstimated => "Työkalulla arvioitu suun liike · laatu mittaamatta",
        MouthSource::TextEstimated => "Tekstistä arvioitu suun liike · ei mitattua puhekohdistusta",
        MouthSource::None => "Ei suun ajoitusta · suu lepoasennossa",
    }
}
fn viseme_label(viseme: Viseme) -> &'static str {
    match viseme {
        Viseme::Rest => "lepo",
        Viseme::Closed => "kiinni",
        Viseme::TeethLip => "alahuuli ja hampaat",
        Viseme::TongueTeeth => "kieli ja hampaat",
        Viseme::TongueUp => "kieli ylhäällä",
        Viseme::Sibilant => "S-äänne",
        Viseme::Sh => "Š-äänne",
        Viseme::R => "R-äänne",
        Viseme::A => "A",
        Viseme::E => "E",
        Viseme::I => "I",
        Viseme::O => "O",
        Viseme::U => "U",
    }
}
fn asset_kind_label(kind: &motion_project::AssetKind) -> &'static str {
    match kind {
        motion_project::AssetKind::Image => "kuva",
        motion_project::AssetKind::Audio => "ääni",
        motion_project::AssetKind::Other => "muu tiedosto",
    }
}
fn default_kokoro_root() -> String {
    kokoro_setup::default_root().to_string_lossy().into_owned()
}
impl Studio {
    fn script_pronunciations(&self) -> std::collections::BTreeMap<String, String> {
        self.script
            .as_ref()
            .map(|s| s.pronunciations.clone())
            .unwrap_or_default()
    }
    fn submit_kokoro(&mut self, task: kokoro_setup::Task) {
        if self.busy || self.diagnostics_busy || self.export_busy {
            return;
        }
        // Setup, connect and test all install or run Kokoro; strict blocks them
        // regardless of the separate download consent.
        if !self.permit_model(motion_project::ModelBackedStep::ModelDownload) {
            self.setup_status = self.status.clone();
            return;
        }
        match self.kokoro_setup.submit(task) {
            Ok(()) => {
                self.stop_playback();
                self.setup_busy = true;
                self.diagnostics_busy = true;
                self.setup_status = "Paikallinen työ käynnissä…".into();
            }
            Err(error) => self.setup_status = format!("Kokoro-toiminto epäonnistui: {error}"),
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
                kokoro_setup::Update::Progress(message) => {
                    self.setup_status = format!("Kokoro: {message}")
                }
                kokoro_setup::Update::Finished(result) => {
                    self.setup_busy = false;
                    self.diagnostics_busy = false;
                    match result {
                        Ok(kokoro_setup::Outcome::Connected(profile)) => {
                            self.apply_kokoro_profile(profile);
                            self.setup_status = "Paikallinen Kokoro yhdistetty. Aja englanninkielinen puhetesti ja kuuntele tulos.".into();
                        }
                        Ok(kokoro_setup::Outcome::Tested {
                            wav,
                            report,
                            metrics,
                            audio,
                        }) => {
                            self.test_audio = Some(audio);
                            self.setup_status = format!("Puhetesti valmis. {metrics}\nWAV: {}\nRaportti: {}\nArvioi puheen laatu kuuntelemalla. Testi ei vahvista huulisynkkaa.", wav.display(), report.display());
                        }
                        Err(error) => {
                            self.setup_status = format!("Kokoro-toiminto epäonnistui: {error}")
                        }
                    }
                }
            }
        }
        if let Some(player) = &self.test_player {
            let state = player.snapshot();
            if let Some(error) = state.error {
                self.setup_status = format!("Testipuheen toisto: {error}");
                self.test_player = None;
            } else if state.finished {
                self.test_player = None;
            }
        }
    }
    fn kokoro_setup_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Kokoro · englanninkielisen puheen asennus");
        ui.label("Asenna ja testaa englanninkielinen puhe. Python 3.10–3.12 on oltava asennettuna. Malli toimii paikallisessa taustaprosessissa.");
        let idle = !self.busy && !self.diagnostics_busy && !self.export_busy;
        ui.add_enabled_ui(idle, |ui| {
            field(ui, "Asennuskansio", &mut self.setup_root);
            field(ui, "Python 3.10–3.12 -ohjelma (tyhjä: etsi automaattisesti)", &mut self.setup_python);
            ui.checkbox(&mut self.setup_downloads, "Sallin Python-riippuvuuksien, Kokoro-mallin, äänten ja englannin kieliresurssien lataamisen tätä asennusta varten.");
            ui.weak("Asennus tarvitsee verkkoyhteyden ja levytilaa. Asennettu puhe toimii paikallisesti. Voit yhdistää olemassa olevat resurssit ilman latauksia.");
            let root = std::path::PathBuf::from(self.setup_root.trim());
            let (installer, manifest) = kokoro_setup::helper_paths(Path::new(&self.runtime));
            if ui.add_enabled(self.setup_downloads && !self.setup_root.trim().is_empty(), egui::Button::new("Asenna paikallinen Kokoro")).clicked() {
                let python = (!self.setup_python.trim().is_empty()).then(|| self.setup_python.trim().into());
                self.submit_kokoro(kokoro_setup::Task::Setup { python, root: root.clone(), installer, manifest, allow_downloads: self.setup_downloads });
            }
            if ui.add_enabled(!self.setup_root.trim().is_empty(), egui::Button::new("Yhdistä olemassa oleva asennus")).clicked() {
                self.submit_kokoro(kokoro_setup::Task::ConnectExisting { root: root.clone() });
            }
            let previous_voice = self.setup_voice.clone();
            ui.horizontal(|ui| {
                ui.label("Englanninkielinen testiääni");
                egui::ComboBox::from_id_salt("kokoro-test-voice").selected_text(&self.setup_voice).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.setup_voice, "af_heart".into(), "af_heart");
                    ui.selectable_value(&mut self.setup_voice, "am_michael".into(), "am_michael");
                });
            });
            if self.setup_voice != previous_voice { self.test_audio = None; self.stop_playback(); }
            if ui.add_enabled(self.setup_profile.is_some(), egui::Button::new("Testaa englanninkielinen puhe")).clicked() {
                if let Some(profile) = self.setup_profile.clone() {
                    self.test_audio = None;
                    let output_root = profile.cache.parent().unwrap_or(&profile.cache).join("speech-tests");
                    self.submit_kokoro(kokoro_setup::Task::Test { profile, smoke_script: Path::new(&self.runtime).with_file_name("smoke_macos.py"), voice_preset: self.setup_voice.clone(), output_root });
                }
            }
            if ui.add_enabled(self.test_audio.is_some(), egui::Button::new("Toista testipuhe")).clicked() {
                self.stop_playback();
                if let Some(audio) = self.test_audio.clone() {
                    match playback::Playback::new().and_then(|player| { player.play(audio, 0)?; Ok(player) }) {
                        Ok(player) => self.test_player = Some(player),
                        Err(error) => self.setup_status = format!("Kokoro-toiminto epäonnistui: {error}"),
                    }
                }
            }
        });
        if self.setup_busy {
            ui.spinner();
            if ui.button("Peru Kokoro-toiminto").clicked() {
                self.kokoro_setup.cancel();
                self.setup_status = "Perutaan…".into();
            }
        }
        if self.test_player.is_some() && ui.button("Pysäytä testipuhe").clicked() {
            self.stop_playback();
        }
        if !self.setup_status.is_empty() {
            ui.label(&self.setup_status);
        }
        ui.separator();
    }
}

fn caption_mode_label(mode: motion_script::CaptionMode) -> &'static str {
    match mode {
        motion_script::CaptionMode::Follow => "Käsikirjoituksen mukaan",
        motion_script::CaptionMode::Show => "Näytä",
        motion_script::CaptionMode::Hide => "Piilota",
    }
}
/// The user-level caption choice. Returns true when the user changed it.
fn caption_mode_ui(ui: &mut egui::Ui, mode: &mut motion_script::CaptionMode) -> bool {
    use motion_script::CaptionMode::{Follow, Hide, Show};
    let before = *mode;
    ui.horizontal(|ui| {
        ui.label("Tekstitys");
        egui::ComboBox::from_id_salt("caption-mode")
            .selected_text(caption_mode_label(*mode))
            .show_ui(ui, |ui| {
                for choice in [Follow, Show, Hide] {
                    ui.selectable_value(mode, choice, caption_mode_label(choice));
                }
            })
            .response
            .on_hover_text(
                "Käsikirjoituksen mukaan käyttää project.captions-asetusta. Näytä ja Piilota ohittavat sen esikatseluissa ja vienneissä muuttamatta käsikirjoitusta. Näytä tarvitsee project.captions-tyylin.",
            );
    });
    *mode != before
}
fn field(ui: &mut egui::Ui, label: &str, value: &mut String) -> bool {
    ui.label(label);
    ui.text_edit_singleline(value).changed()
}
impl Studio {
    fn selected_alignment_edited(&mut self) {
        self.stop_playback();
        self.preview = None;
        self.status = "Valitun repliikin ajoitus muuttui; valmistele repliikki uudelleen.".into();
    }
    fn runtime_ui(&mut self, ui: &mut egui::Ui) {
        self.kokoro_setup_ui(ui);
        ui.add_enabled_ui(!self.busy && !self.diagnostics_busy && !self.export_busy, |ui| {
                    let mut runtime_changed = false;
                    ui.heading("Paikallisen puhemoottorin lisäasetukset");
                    ui.label("Anna asennettujen paikallisten resurssien polut tai käytä yllä olevaa asennustoimintoa.");
                    runtime_changed |= field(ui, "Python-ohjelma", &mut self.python);
                    runtime_changed |= field(ui, "Puhemoottorin taustaskripti", &mut self.runtime);
                    runtime_changed |= field(ui, "Mallin JSON-asetukset", &mut self.config);
                    runtime_changed |= field(ui, "Mallitiedosto", &mut self.weights);
                    runtime_changed |= field(ui, "Äänten .pt-kansio", &mut self.voices);
                    runtime_changed |= field(ui, "Välimuistikansio", &mut self.cache);
                    runtime_changed |= field(ui, "Mallin ja riippuvuuksien tarkka versio", &mut self.revision);
                    if ui.button("Tarkista paikallinen Kokoro-asennus").clicked() { self.check_runtime(); }
                    ui.label("Esitarkistus lukee paikalliset paketit ja tiedostot. Se ei luo puhetta eikä vahvista yhteensopivuutta.");
                    if runtime_changed { self.sequence_changed(); self.stop_playback(); self.scene_preview = None; self.scene_audio_active = false; self.preview = None; self.setup_profile = None; self.test_audio = None; }
                    if field(ui, "Projektikansio (tuotujen WAVien polut)", &mut self.project_root) {
                        self.pending_text_edit = None; self.history.reset(&self.source);
                        self.stop_playback(); self.invalidate_scene(); self.jobs.clear(); self.preview = None;
                        self.status = "Projektikansio vaihtui; paina Tarkista, jotta tuotujen resurssien polut ratkaistaan uudelleen.".into();
                    }
                    ui.separator();
                    ui.heading("Kohtauksen suun ajoitukset (JSON)");
                    ui.label("Kohdista repliikkitunnisteet ääneen sidottuihin ajoituksiin tai vanhoihin Alignment-olioihin, esimerkiksi {\"0\": {...}}. Vanha ajoitus on sitomaton. Tyhjä kenttä sallii tekstiarvion; valmistele ääni uudelleen muokkauksen jälkeen.");
                    if ui.add(egui::TextEdit::multiline(&mut self.scene_alignment).desired_rows(5).char_limit(100_000).code_editor()).changed() { self.sequence_changed(); self.stop_playback(); self.scene_preview = None; self.scene_audio_active = false; }
                    for job in self.jobs.iter().filter(|job| self.script.as_ref().and_then(|s| s.scenes.get(self.scene_index)).is_some_and(|s| s.id == job.scene)) { ui.label(format!("ID {} · {} · {}", job.id, job.actor, job.request.text)); }
                    ui.heading("Valitun repliikin suun ajoitus (JSON)");
                    ui.label("Liitä ääneen sidottu ajoitus (schema_version, key, valinnainen alignment) tai vanha sitomaton Alignment-olio. Tyhjä kenttä sallii tekstiarvion. Alkuperän tarkistus ei mittaa ajoituksen laatua.");
                    if ui.add(egui::TextEdit::multiline(&mut self.alignment).desired_rows(7).char_limit(100_000).code_editor()).changed() { self.selected_alignment_edited(); }
        });
    }
    fn dialogue_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_enabled_ui(!self.busy && !self.diagnostics_busy, |ui| {
                    egui::ComboBox::from_label("Repliikki").selected_text(self.jobs.get(self.selected)
                        .map(|j| format!("{} / {} @ {:.2}s", j.scene, j.actor, j.at)).unwrap_or_else(|| "Paina ensin Tarkista".into()))
                        .show_ui(ui, |ui| {
                            for (index, job) in self.jobs.iter().enumerate() {
                                if ui.selectable_value(&mut self.selected, index, format!("{} / {}: {}", job.scene, job.actor, job.request.text)).changed() { if let Some(p) = &self.playback { let _ = p.stop(); } self.preview = None; }
                            }
                        });
                    if let Some(job) = self.jobs.get_mut(self.selected) {
                        ui.add(egui::DragValue::new(&mut job.at).range(0.0..=86_400.0).speed(0.01).prefix("Repliikin aloitusaika (s) "));
                        ui.label("Aloitusaika vaikuttaa seuraavaan äänen valmisteluun. Muuta myös käsikirjoitusta tallentaaksesi ajan.");
                    }
                    if ui.add_enabled(!self.jobs.is_empty(), egui::Button::new("Valmistele valitun repliikin ääni")).clicked() { self.generate(); }
        });
    }
    fn line_preview_ui(&mut self, ui: &mut egui::Ui) {
        if let Some(preview) = &self.preview {
            ui.separator();
            ui.label(format!(
                "Äänitiedosto: {}",
                preview.result.audio_path.display()
            ));
            ui.label(format!(
                "Kesto {:.3} s · {} Hz · {} ääninäytettä",
                preview.result.duration_seconds(),
                preview.result.sample_rate,
                preview.result.sample_count
            ));
            let start = preview.clip.start_frame as f64 / 24000.0;
            let end = start + preview.clip.audio.duration_seconds();
            if let Some(player) = &self.playback {
                let state = player.snapshot();
                ui.label(if state.finished {
                    "Toisto päättynyt"
                } else if state.playing {
                    "Toistetaan"
                } else {
                    "Tauolla tai pysäytetty"
                });
            }
            ui.horizontal(|ui| {
                if ui.button("Toista toistopäästä").clicked() {
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
                if ui.button("Tauko").clicked() {
                    if let Some(p) = &self.playback {
                        if let Err(e) = p.pause() {
                            self.playback_error = Some(e);
                        }
                    }
                }
                if ui.button("Jatka").clicked() {
                    self.sequence_panel.stop_transport();
                    if let Some(p) = &self.playback {
                        if let Err(e) = p.resume() {
                            self.playback_error = Some(e);
                        }
                    }
                }
                if ui.button("Pysäytä").clicked() {
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
                        .text("Kohtauksen toistokohta (s)"),
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
            ui.label(format!(
                "Ääninäyte {scene_frame} · suun muoto {}",
                viseme_label(mouth)
            ));
            waveform(ui, &preview.peaks, (self.playhead - start) / (end - start));
            mouth_preview(ui, mouth);
            ui.label("macOS-toisto käyttää äänen toistokohtaa suun esikatselussa. Viive ja havaittu huulisynkka on tarkistettava Macilla. MP4-vienti tarvitsee paikallisen FFmpeg-ohjelman.");
            if let Some(error) = &self.playback_error {
                ui.colored_label(egui::Color32::LIGHT_RED, format!("Virhe: {error}"));
            }
            ui.label(alignment_status_text(preview.alignment_status));
            ui.label(mouth_source_text(preview.clip.mouth_source()));
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
            .min_height(48.0)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(25, 29, 36))
                    .inner_margin(10.0),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("K").strong().size(20.0).color(ACCENT));
                    ui.label(egui::RichText::new("KILSAT STUDIO").strong().size(14.0));
                    ui.separator();
                    ui.add_enabled_ui(!self.busy && !self.diagnostics_busy, |ui| {
                        if ui
                            .button("Avaa")
                            .on_hover_text("Avaa projekti vasemman paneelin tiedostopolusta. Polku löytyy myös MotionScript-välilehdeltä.")
                            .clicked()
                        {
                            self.open_script();
                        }
                        if ui
                            .button("Tallenna")
                            .on_hover_text("Tallenna projekti annettuun polkuun. Korvaa samannimisen tiedoston.")
                            .clicked()
                        {
                            self.save_script();
                        }
                        if ui.button("Tarkista").clicked() {
                            self.validate();
                        }
                        let pending_change = self
                            .pending_text_edit
                            .as_ref()
                            .is_some_and(|before| before != &self.source);
                        if ui
                            .add_enabled(
                                pending_change || self.history.can_undo(),
                                egui::Button::new("Kumoa"),
                            )
                            .on_hover_text("Kumoa projektin muokkaus · Cmd/Ctrl+Z")
                            .clicked()
                        {
                            self.undo_edit(false);
                        }
                        if ui
                            .add_enabled(
                                !pending_change && self.history.can_redo(),
                                egui::Button::new("Tee uudelleen"),
                            )
                            .on_hover_text("Palauta kumottu muokkaus · Cmd/Ctrl+Shift+Z")
                            .clicked()
                        {
                            self.undo_edit(true);
                        }
                        if ui
                            .button("Ääniesimerkki")
                            .on_hover_text(
                                "Testiääniä ilman puhemallin lataamista. Ei oikeaa puhetta.",
                            )
                            .clicked()
                        {
                            self.load_sound_check();
                        }
                    });
                    if ui.button("Ohjeet").on_hover_text("Suomenkielinen käyttöopas: mistä tehdään mitäkin").clicked() { self.help_open = true; }
                    ui.separator();
                    self.level_and_policy_ui(ui);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.weak("OFFLINE / CPU");
                        if self.busy || self.diagnostics_busy {
                            ui.spinner();
                        }
                        if self.busy {
                            if let Some(cancel) = &self.active_cancel {
                                if cancel.is_cancelled() {
                                    ui.label("Pysäytetään…");
                                } else if ui
                                    .button(if self.export_busy {
                                        "Peru vienti"
                                    } else {
                                        "Peru valmistelu"
                                    })
                                    .clicked()
                                {
                                    cancel.cancel();
                                    self.status = "Peruutusta pyydetty…".into();
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
            .min_height(110.0)
            .max_height(720.0)
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
        studio_help::show(ctx, &mut self.help_open, &mut self.help_topic);
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
            self.status =
                "Tallenna projekti .kilsat-muotoon ennen paikallisten resurssien hallintaa.".into();
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
            self.status = "Resurssien hallinta tarvitsee .kilsat-tiedoston asetettuun projektikansioon. Avaa tai tallenna haluttu projekti ensin.".into();
            return;
        }
        self.flush_text_edit();
        let mut document = self.project_document.clone().unwrap();
        document.source = self.source.clone();
        if let Err(error) = document.validate() {
            self.status = format!("Resurssityö epäonnistui: {error}");
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
                self.status = "Päivitetään paikallisia resursseja taustalla…".into();
            }
            Err(error) => self.status = format!("Resurssityö epäonnistui: {error}"),
        }
    }
    fn assets_ui(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Resurssit · kuvat ja äänet").show(ui, |ui| {
            if let Some(document) = &self.project_document {
                for asset in &document.assets { ui.label(format!("{} · {} · {}", asset.id, asset_kind_label(&asset.kind), asset.path)); }
            }
            if self.asset_dirty { ui.label("Tallentamattomia resurssimuutoksia"); }
            ui.weak("Tuo kopioi paikallisen tiedoston projektiin. Vaihda viite käyttää projektikansioon suhteutettua polkua. Poista viite säilyttää tiedoston. Lisää tuodut PNG/JPEG-kuvat näyttämölle Kuvitus-tilan tasotyökaluilla.");
            ui.add_enabled_ui(!self.busy && !self.export_busy && !self.diagnostics_busy, |ui| {
                field(ui, "Resurssin tunniste", &mut self.asset_id);
                field(ui, "Tuotavan tiedoston koko polku / vaihdettavan viitteen suhteellinen polku", &mut self.asset_path);
                ui.horizontal(|ui| { ui.selectable_value(&mut self.asset_kind, 0, "Kuva"); ui.selectable_value(&mut self.asset_kind, 1, "Ääni"); ui.selectable_value(&mut self.asset_kind, 2, "Muu"); });
                if ui.button("Tuo paikallinen tiedosto").clicked() {
                    let kind = match self.asset_kind { 0 => motion_project::AssetKind::Image, 1 => motion_project::AssetKind::Audio, _ => motion_project::AssetKind::Other };
                    self.start_asset(asset_worker::Operation::Import { source: self.asset_path.clone().into(), id: self.asset_id.clone(), kind });
                }
                if ui.button("Vaihda resurssiviite").clicked() { self.start_asset(asset_worker::Operation::Relink { id: self.asset_id.clone(), path: self.asset_path.clone() }); }
                if ui.button("Poista viite (säilytä tiedosto)").clicked() { self.start_asset(asset_worker::Operation::Remove { id: self.asset_id.clone() }); }
                ui.weak("Yläpalkin Kumoa ja Tee uudelleen palauttavat myös resurssiviitteet. Tuodut tiedostot säilyvät levyllä.");
            });
        });
    }
    fn create_dialogue_scene(&mut self) {
        if self.busy || self.diagnostics_busy || self.export_busy || self.setup_busy {
            return;
        }
        self.flush_text_edit();
        match quick_start::append_scene(&self.source, &self.quick_start) {
            Ok((source, index)) => {
                self.apply_structure(Ok(source.clone()));
                if self.source == source {
                    self.scene_index = index;
                    self.reset_scene();
                    self.status = "Kohtaus luotu. Esikatsele liikettä, valitse puheääni ja valmistele ääni ennen vientiä. Tallenna työ .kilsat-tiedostona.".into();
                }
            }
            Err(error) => self.status = format!("Kohtauksen luominen epäonnistui: {error}"),
        }
    }

    fn auto_direct_dialogue(&mut self) {
        if self.busy || self.diagnostics_busy || self.export_busy || self.setup_busy {
            return;
        }
        self.flush_text_edit();
        match motion_script::auto_direction::direct(&self.source, &self.quick_start.dialogue, None)
        {
            Ok(plan) => {
                self.apply_structure(Ok(plan.source.clone()));
                if self.source == plan.source {
                    self.scene_index = plan.first_scene;
                    self.reset_scene();
                    self.status = format!("Ohjausluonnos: {} otosta, {:.2} s. {} Esikatsele ja vie kaikki otokset kohtauskoosteena. Nykyiset kohtaukset säilyvät; Kumoa palauttaa projektin.",
                        plan.shot_count, plan.duration_samples as f64 / 24_000.0,
                        plan.diagnostics.join(" "));
                }
            }
            Err(error) => self.status = format!("Automaattiohjaus: {error}"),
        }
    }

    fn quick_start_ui(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Luo animaatio vuorosanoista")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("1. Kirjoita yksi vuorosana riville");
                let actors = self.script.as_ref().map(|script| {
                    script.characters.keys().cloned().collect::<Vec<_>>().join(", ")
                }).unwrap_or_default();
                ui.weak(format!("HAHMO: vuorosana · {actors}"));
                let idle = !self.busy && !self.diagnostics_busy && !self.export_busy && !self.setup_busy;
                ui.add_enabled_ui(idle, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut self.quick_start.dialogue)
                        .desired_rows(6).desired_width(f32::INFINITY).char_limit(16_000));
                    ui.horizontal(|ui| {
                        ui.label("Sekuntia per vuorosana");
                        ui.add(egui::DragValue::new(&mut self.quick_start.seconds_per_line)
                            .range(1.0..=30.0).speed(0.5));
                    });
                    ui.weak("Ajoitus on luonnos. Pitkä repliikki tarvitsee lisää aikaa. Nykyiset kohtaukset säilyvät; Kumoa poistaa uuden kohtauksen.");
                    if ui.button("Luo kohtaus vuorosanoista").clicked() {
                        self.create_dialogue_scene();
                    }
                    ui.weak("Automaattiohjaus: enintään 2 puhujaa, 16 vuorosanaa ja 60 sekuntia. Tunne voidaan merkitä: EMMA[happy]: Hei! Tarkista arvioitu ajoitus puheesta.");
                    if ui.button("Automaattiohjaa luonnos (arvioitu puhe)").clicked() {self.auto_direct_dialogue();}
                    ui.weak("Puheohjaus käyttää oikeita WAV-kestoja, lisää reaktio-otoksia ja säilyttää muokattavan lähteen. Valitse hahmojen äänet oikealta Puhe-välilehdeltä.");
                    if self.direction_imports_for!=self.quick_start.dialogue {self.direction_imports.clear();self.direction_imports_for=self.quick_start.dialogue.clone();}
                    egui::CollapsingHeader::new("Omat WAV-äänet repliikeille (valinnainen)").show(ui,|ui|{
                        if let Some(script)=&self.script {match motion_script::auto_direction::parse_dialogue(&self.quick_start.dialogue,script){
                            Ok(lines)=>{for line in lines{ui.label(format!("Rivi {} · {}: {}",line.source_line,line.actor,line.text));ui.add(egui::TextEdit::singleline(self.direction_imports.entry(line.source_line).or_default()).hint_text("WAV-polku tai tyhjä: käytä hahmon puheääntä"));}},
                            Err(error)=>{ui.weak(error);}
                        }}
                        ui.weak("Tuodun WAVin on oltava 24 kHz mono PCM16. Vuorosanojen muuttaminen tyhjentää nämä sidonnat.");
                    });
                    if ui.button("Ohjaa oikean puheen mukaan").clicked(){self.start_measured_direction();}
                });
                ui.separator();
                ui.label("2. Valmistele puhe");
            ui.weak("Avaa oikealta Puhe: valitse Piper suomeksi, Kokoro englanniksi tai oma WAV-äänite. Piper tarvitsee erikseen asennetun mallin ja profiilin.");
                ui.add_enabled_ui(idle && self.script.is_some(), |ui| {
                    if ui.button("Valmistele kohtauksen ääni").clicked() {
                        self.prepare_scene_audio();
                    }
                });
                if self.scene_preview.is_some() {
                    ui.label("Ääni valmis. Kuuntele alhaalta Toista ääni -painikkeella.");
                }
                ui.label("3. Tallenna projekti");
                field(ui, "Projektikansio (koko polku)", &mut self.project_root);
                field(ui, "Projektitiedosto (.kilsat, projektikansion sisällä). Tallennus korvaa tiedoston.", &mut self.script_path);
                ui.add_enabled_ui(idle, |ui| {
                    if ui.button("Tallenna projekti").clicked() {
                        self.save_script();
                    }
                });
                if ui.button("Avaa videon vienti").clicked() {
                    self.inspector_tab = InspectorTab::Export;
                    self.export_format = export_worker::Format::Mp4;
                }
            });
    }

    fn project_ui(&mut self, ui: &mut egui::Ui) {
        let previous_tab = self.editor_tab;
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.editor_tab, false, "Projekti");
            ui.selectable_value(&mut self.editor_tab, true, "MotionScript");
        });
        if previous_tab != self.editor_tab {
            self.flush_text_edit();
        }
        ui.separator();
        egui::ScrollArea::vertical().id_salt("project_scroll").show(ui, |ui| {
            if self.editor_tab {
                ui.weak("MotionScript: englanninkielinen YAML. Paina Tarkista muokkauksen jälkeen.");
                ui.add_enabled_ui(!self.busy && !self.diagnostics_busy, |ui| {
                    field(ui, "YAML- tai .kilsat-tiedoston koko polku. Tallennus korvaa tiedoston.", &mut self.script_path);
                    ui.add_space(6.0);
                    let before = self.source.clone();
                    let response = ui.add(egui::TextEdit::multiline(&mut self.source)
                        .id(egui::Id::new("screenplay_editor"))
                        .desired_rows(32).desired_width(f32::INFINITY).char_limit(MAX_SOURCE_BYTES)
                        .font(egui::TextStyle::Monospace).code_editor());
                    if response.changed()
                    {
                        if self.pending_text_edit.is_none() { self.pending_text_edit = Some(before); }
                        self.alignment.clear(); self.scene_alignment.clear();
                        self.stop_playback(); self.invalidate_scene(); self.jobs.clear(); self.preview = None;
                        self.status = "Käsikirjoitus muuttui; paina Tarkista ennen äänen valmistelua.".into();
                    }
                    if response.lost_focus() { self.flush_text_edit(); }
                });
                return;
            }
            ui.add_space(6.0);
            self.creation_ui(ui);
            if let Some(script) = &self.script { ui.label(egui::RichText::new(&script.project.title).strong().size(16.0)); }
            ui.add_space(12.0);
            ui.weak("Valitse kohtaus, vedä hahmoa ja toista alhaalta.");
            section_label(ui, "KOHTAUKSET");
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
            section_label(ui, "HAHMOT");
            let characters: Vec<_> = self.script.as_ref().map(|s| s.characters.iter()
                .map(|(id, c)| (id.clone(), c.asset.clone())).collect()).unwrap_or_default();
            for (id, asset) in characters {
                if ui.selectable_label(self.selected_actor.as_deref() == Some(id.as_str()), &id).clicked() {
                    self.selected_actor = Some(id); self.selected_event = None;
                }
                ui.label(egui::RichText::new(asset).small().color(egui::Color32::from_gray(140)));
                ui.add_space(5.0);
            }
            ui.separator();
            self.story_ui(ui);
            self.quick_start_ui(ui);
            egui::CollapsingHeader::new("Projektin lisätyökalut").show(ui, |ui| {
                self.settings_ui(ui);
                self.structure_ui(ui);
                self.assets_ui(ui);
            });
            if self.script.is_none() {
                ui.label("Paina Tarkista, jotta projekti ja kohtaukset latautuvat.");
            }
            ui.add_space(16.0);
            ui.weak("Valitse hahmo listasta tai näyttämöltä. Ohjeet-painike neuvoo animoinnin.");
            if self.demo_assets.as_ref().is_some_and(|assets| {
                self.jobs.iter().any(|job| {
                    job.imported_audio.as_ref().is_some_and(|path| path.starts_with(&assets.root))
                })
            }) {
                ui.add_space(8.0);
                ui.colored_label(egui::Color32::from_rgb(238, 185, 107), "SYNTEETTINEN ÄÄNIESIMERKKI");
                ui.weak("Esimerkki käyttää testiääniä ja havainnollista suun ajoitusta. Kopioi väliaikaiset resurssit ennen pysyvän projektin tallentamista.");
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
            if ui.button("Lataa kuvat uudelleen levyltä").clicked() {
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
                ui.colored_label(egui::Color32::LIGHT_RED, format!("Virhe: {error}"));
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
                    egui::CollapsingHeader::new("Hahmon sijainnin ja koon avainruudut").show(
                        ui,
                        |ui| {
                            apply = draft.ui(ui);
                        },
                    );
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
    fn apply_cue_draft(&mut self) {
        let Some((source, draft)) = &self.cue_draft else {
            return;
        };
        let current_scene = self
            .script
            .as_ref()
            .and_then(|script| script.scenes.get(self.scene_index))
            .map(|scene| scene.id.as_str());
        if source != &self.source
            || draft.scene_index != self.scene_index
            || current_scene != Some(draft.scene_id.as_str())
        {
            self.status = "Äänivihjeen luonnos on vanhasta käsikirjoituksesta tai kohtauksesta; lataa se uudelleen ennen käyttöä.".into();
            self.cue_draft = None;
            return;
        }
        let result = draft.apply(&self.source);
        self.apply_structure(result);
    }

    fn apply_rule_draft(&mut self) {
        let Some((source, draft)) = &self.rule_draft else {
            return;
        };
        if source != &self.source {
            self.status =
                "Säännön luonnos on vanhasta käsikirjoituksesta; lataa se uudelleen ennen käyttöä."
                    .into();
            self.rule_draft = None;
            return;
        }
        let result = draft.apply(&self.source);
        self.apply_structure(result);
    }

    fn cue_rule_inspector_ui(&mut self, ui: &mut egui::Ui) {
        let Some(script) = &self.script else {
            return;
        };
        let editable = !self.busy && !self.diagnostics_busy && !self.export_busy;
        let mut apply_cues = false;
        let mut apply_rules = false;
        if let Some(scene) = script.scenes.get(self.scene_index) {
            if !self.cue_draft.as_ref().is_some_and(|(source, draft)| {
                source == &self.source
                    && draft.scene_index == self.scene_index
                    && draft.scene_id == scene.id
            }) {
                self.cue_draft = Some((
                    self.source.clone(),
                    cue_rule_editor::CueDraft::new(scene, self.scene_index),
                ));
            }
            ui.add_enabled_ui(editable, |ui| {
                egui::CollapsingHeader::new("Äänivihjeet").show(ui, |ui| {
                    ui.small("Kohtauksen tai käsikirjoituksen vaihtaminen lataa tallennetut äänivihjeet ja hylkää käyttämättömät muutokset.");
                    if let Some((_, draft)) = &mut self.cue_draft {
                        apply_cues = draft.ui(ui);
                    }
                });
            });
        }
        if !self
            .rule_draft
            .as_ref()
            .is_some_and(|(source, _)| source == &self.source)
        {
            self.rule_draft = Some((
                self.source.clone(),
                cue_rule_editor::RuleDraft::new(&script.rules),
            ));
        }
        ui.add_enabled_ui(editable, |ui| {
            egui::CollapsingHeader::new("Animaatiosäännöt").show(ui, |ui| {
                ui.small("Käsikirjoituksen vaihtaminen lataa tallennetut säännöt ja hylkää käyttämättömät muutokset.");
                if let Some((_, draft)) = &mut self.rule_draft {
                    apply_rules = draft.ui(ui);
                }
            });
        });
        if apply_cues {
            self.apply_cue_draft();
        }
        if apply_rules {
            self.apply_rule_draft();
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
            self.status = "Luun luonnos on aiemmasta käsikirjoituksesta, kohtauksesta, hahmosta tai luusta; lataa se uudelleen ennen käyttöä.".into();
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
            self.status = "Nivelen luonnos on aiemmasta käsikirjoituksesta, kohtauksesta, hahmosta tai nivelestä; lataa se uudelleen ennen käyttöä.".into();
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
        let actor_owned = actor.to_owned();
        let mut apply = false;
        let mut clip_action = None;
        ui.add_enabled_ui(
            !self.busy && !self.diagnostics_busy && !self.export_busy && !self.setup_busy,
            |ui| {
                egui::CollapsingHeader::new("Hahmorakenteen nivelten avainruudut").show(ui, |ui| {
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
                    ui.small("Kohtauksen, hahmon tai nivelen vaihtaminen lataa tallennetut avainruudut ja hylkää käyttämättömät muutokset.");
                    if let Some((_, draft)) = &mut self.joint_draft {
                        apply = draft.ui(ui);
                    }
                    ui.separator();
                    clip_action = self.rig_clips.ui(ui, script);
                });
            },
        );
        if apply {
            self.apply_joint_draft();
        }
        if let Some(action) = clip_action {
            let actor = actor_owned;
            let result = match action {
                rig_clip_editor::Action::Capture { name, replace } => {
                    motion_script::capture_rig_clip(
                        &self.source,
                        self.scene_index,
                        &actor,
                        &name,
                        replace,
                    )
                }
                rig_clip_editor::Action::Apply { name, placement } => {
                    motion_script::apply_rig_clip(
                        &self.source,
                        self.scene_index,
                        &actor,
                        &name,
                        &placement,
                    )
                }
                rig_clip_editor::Action::Remove { name } => {
                    motion_script::remove_rig_clip(&self.source, &name)
                }
            };
            self.apply_structure(result);
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
                "Liikekatkelman luonnos on aiemmasta käsikirjoituksesta, kohtauksesta tai hahmosta; lataa se uudelleen ennen käyttöä."
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
                "Vahvuuksien luonnos on aiemmasta käsikirjoituksesta, kohtauksesta tai hahmosta; lataa se uudelleen ennen käyttöä."
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
            egui::CollapsingHeader::new("Animaatiotasojen vahvuudet").show(ui, |ui| {
                ui.small("Kohtauksen tai hahmon vaihtaminen lataa tallennetut vahvuudet ja hylkää käyttämättömät muutokset.");
                if let Some((_, draft)) = &mut self.clip_draft { apply = draft.ui(ui); }
            });
            egui::CollapsingHeader::new("Animoidut tasovahvuudet").show(ui, |ui| {
                ui.small("Kohtauksen tai hahmon vaihtaminen hylkää käyttämättömät avainruudut.");
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
            self.status = "Liikekirjasto on aiemmasta käsikirjoituksesta, kohtauksesta tai hahmosta; lataa se uudelleen ennen käyttöä.".into();
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
                egui::CollapsingHeader::new("Liikekatkelmien kirjasto").show(ui, |ui| {
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
            self.status = "Liikekatkelman viitteen luonnos on aiemmasta käsikirjoituksesta, kohtauksesta tai hahmosta; lataa se uudelleen ennen käyttöä.".into();
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
            self.status = "Valitse viitteen hahmo ennen liikekatkelman muokkaamista.".into();
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
                egui::CollapsingHeader::new("Liikekatkelmien viitteet").show(ui, |ui| {
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
        let images: Vec<String> = self
            .project_document
            .iter()
            .flat_map(|d| d.assets.iter())
            .filter(|a| a.kind == motion_project::AssetKind::Image)
            .map(|a| a.id.clone())
            .collect();
        ui.add_enabled_ui(
            !self.busy && !self.diagnostics_busy && !self.export_busy && !self.setup_busy,
            |ui| {
                egui::CollapsingHeader::new("Hahmon rakenne").show(ui, |ui| {
                    if let Some((_, draft)) = &mut self.custom_rig_draft {
                        action = draft.ui(ui, &images);
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
            self.status =
                "Hahmo tai käsikirjoitus vaihtui; avaa hahmorakenteen luonnos uudelleen.".into();
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
            egui::CollapsingHeader::new("Luiden avainruudut").show(ui, |ui| {
                let old_bone = self.selected_bone.clone();
                egui::ComboBox::from_id_salt("authored_bone")
                    .selected_text(&self.selected_bone).show_ui(ui, |ui| {
                        for bone in motion_script::HUMANOID_BONES {
                            ui.selectable_value(&mut self.selected_bone, bone.into(), bone);
                        }
                    });
                if self.selected_bone != old_bone {
                    ui.small("Luu vaihtui: tallennetut avainruudut ladattiin ja käyttämättömät muutokset hylättiin.");
                }
                if !self.bone_draft.as_ref().is_some_and(|(source, draft)| {
                    source == &self.source && draft.scene_index == self.scene_index
                        && draft.actor == actor && draft.bone == self.selected_bone
                }) {
                    self.bone_draft = Some((self.source.clone(), bone_editor::Draft::for_bone(
                        scene, self.scene_index, actor, &self.selected_bone,
                    )));
                }
                ui.small("Kohtauksen, hahmon tai luun vaihtaminen lataa tallennetut avainruudut ja hylkää käyttämättömät muutokset.");
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
                    piper_config: self.piper_profile(),
                    alignments: self.scene_alignment.clone(),
                    art: motion_export::ArtContext {
                        document: self
                            .project_document
                            .as_ref()
                            .map(|d| d.art.clone())
                            .unwrap_or_default(),
                        images: (*self.art_resources.images).clone(),
                        // Not the 540 px preview proxies: this context also feeds
                        // sequence export, whose preflight must reject undecoded
                        // video rather than silently export proxies.
                        screen_videos: Default::default(),
                    },
                    cancel: CancellationToken::new(),
                    policy: self.model_policy,
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
        use ui_levels::Tool;
        let level = self.ui_level;
        ui.heading("Työkalut");
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.inspector_tab, InspectorTab::Edit, "Muokkaa");
            ui.selectable_value(&mut self.inspector_tab, InspectorTab::Speech, "Puhe");
            ui.selectable_value(&mut self.inspector_tab, InspectorTab::Export, "Vie video");
            if level.shows(Tool::AdvancedTab) {
                ui.selectable_value(
                    &mut self.inspector_tab,
                    InspectorTab::Advanced,
                    "Lisäasetukset",
                );
            }
        });
        if self.inspector_tab == InspectorTab::Advanced && !level.shows(Tool::AdvancedTab) {
            self.inspector_tab = InspectorTab::Edit;
        }
        if self.inspector_tab != InspectorTab::Edit
            || !self.character_mode
            || !level.shows(Tool::PoseControls)
        {
            self.webcam_pose.set_context("head controls hidden");
            self.head_draft = None;
        }
        ui.separator();
        // Keep the caption override available on every tab and synchronized to sequence export.
        if caption_mode_ui(ui, &mut self.caption_mode) {
            self.status = format!(
                "Tekstitys: {}. Valinta vaikuttaa esikatseluun ja vientiin; käsikirjoitus säilyy ennallaan.",
                caption_mode_label(self.caption_mode)
            );
        }
        self.sequence_panel.set_caption_mode(self.caption_mode);
        ui.separator();
        match self.inspector_tab {
            InspectorTab::Edit => {
                ui.weak("Valitse hahmo näyttämöltä tai tapahtuma aikajanalta. Muokkaus toimii, kun toisto on pysäytetty.");
                if let Some(actor) = &self.selected_actor {
                    ui.label(egui::RichText::new(actor).strong().size(18.0));
                }
                if self.character_mode {
                    if level.shows(Tool::PoseControls) {
                        self.hand_props_ui(ui);
                        self.screen_cues_ui(ui);
                        self.camera_framing_ui(ui);
                        self.head_controls_ui(ui);
                        self.keyframe_inspector_ui(ui);
                    }
                    if level.shows(Tool::EventInspector) {
                        self.event_inspector_ui(ui);
                    }
                    if level.shows(Tool::VisemeCheck) {
                        self.viseme_check_ui(ui);
                        self.xsheet_ui(ui);
                    }
                    if level.shows(Tool::PhysicsProfile) {
                        ui.separator();
                        egui::CollapsingHeader::new("Fysiikkaprofiilit (A06)").show(ui, |ui| {
                            ui_levels::spring_profiles_ui(ui, level.shows(Tool::SpringParameters))
                        });
                    }
                    if level.shows(Tool::RuleList) {
                        ui.separator();
                        self.rule_list_ui(ui);
                    }
                    if level.shows(Tool::RigTools) {
                        egui::CollapsingHeader::new("Rig-nivelet, säännöt ja klipit").show(
                            ui,
                            |ui| {
                                self.custom_rig_ui(ui);
                                self.bone_inspector_ui(ui);
                                self.cue_rule_inspector_ui(ui);
                                self.joint_inspector_ui(ui);
                                self.clip_inspector_ui(ui);
                                self.clip_library_ui(ui);
                                self.live_clip_ui(ui);
                            },
                        );
                        ui.weak("Kahden luun IK (A05) on olemassa, mutta IK-kohteen aikajanaraita ja ratkaisu kohtausarvioinnissa odottavat A04:ää (KS-003d).");
                    }
                    ui.separator();
                    ui.label(format!(
                        "Tapahtumaloki: {} tapahtumaa (raahaukset ja vapautukset, 24 kHz)",
                        self.drag_recorder.log().events.len()
                    ));
                    if ui
                        .add_enabled(
                            !self.drag_recorder.log().events.is_empty()
                                && !self.drag_recorder.is_dragging(),
                            egui::Button::new("Tyhjennä tapahtumaloki"),
                        )
                        .clicked()
                    {
                        self.drag_recorder.clear();
                        self.status =
                            "Tapahtumaloki tyhjennetty; tallenna .kilsat säilyttääksesi muutoksen."
                                .into();
                    }
                } else {
                    ui.weak("Kuvitus-tila: piirrä tai muokkaa tasoja. Vaihda näyttämöltä Hahmot, kun haluat siirtää hahmoa.");
                    self.art_inspector_ui(ui);
                }
            }
            InspectorTab::Speech => {
                ui.heading("Puhe ja äänet");
                ui.weak("1. Valitse Piper suomeksi, Kokoro englanniksi tai oma WAV-äänite. 2. Valmistele kohtauksen ääni. 3. Kuuntele aikajanan Toista ääni -painikkeella.");
                let mut piper_path = self.piper_config_path.clone();
                field(
                    ui,
                    "Piper-asetustiedosto (JSON, koko polku)",
                    &mut piper_path,
                );
                self.set_piper_profile(piper_path);
                ui.weak("Suomi: valitse hahmolle Piper / fi-fi. Malli ja ohjelma asennetaan erikseen; asetus ei lataa niitä. Suun automaattinen liike on nimetty arvio, ei mitattu huulisynkka.");
                let profile_idle = self.editing_idle() && !self.setup_busy;
                ui.add_enabled_ui(profile_idle, |ui| {
                    if let Some(path) = self.piper_profile_editor.ui(ui) {
                        self.set_piper_profile(path.to_string_lossy().into_owned());
                    }
                });
                if let Some(preset) = self.system_inventory.ui(ui) {
                    if let Some(draft) = &mut self.voice_draft {
                        draft.voice = Some(motion_script::VoiceConfig {
                            provider: "system".into(),
                            preset,
                            language: "fi-fi".into(),
                            speed: 1.0,
                        });
                    }
                }
                if !self.direction_report.is_empty() {
                    egui::CollapsingHeader::new("Puheohjauksen ääniraportti").show(ui, |ui| {
                        ui.label(&self.direction_report);
                        if ui.button("Kopioi ääniraportti").clicked() {
                            ui.ctx().copy_text(self.direction_report.clone());
                        }
                    });
                }
                self.voice_inspector_ui(ui);
                self.dialogue_ui(ui);
                ui.add_enabled_ui(self.editing_idle() && !self.setup_busy, |ui| {
                    if ui
                        .add_enabled(
                            self.script.is_some(),
                            egui::Button::new("Valmistele kohtauksen ääni"),
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
                            "Ääni valmis · {} repliikkiä · {:.2} s",
                            preview.clips.len(),
                            preview.audio.duration_seconds()
                        ),
                    );
                }
                egui::CollapsingHeader::new("Kokoron asennus · englanninkielinen puhe")
                    .show(ui, |ui| self.kokoro_setup_ui(ui));
                egui::CollapsingHeader::new("Valitun repliikin ääni ja aaltomuoto").show(
                    ui,
                    |ui| {
                        ui.add_enabled_ui(self.editing_idle(), |ui| self.line_preview_ui(ui));
                    },
                );
            }
            InspectorTab::Export => {
                ui.heading("Vie video");
                ui.weak("Valittu kohtaus: valitse muoto ja uusi tuloskansio. Kaikki kohtaukset: avaa alempana Koko projekti / kohtauskooste.");
                self.export_ui(ui);
                ui.separator();
                self.sequence_ui(ui);
            }
            InspectorTab::Advanced => {
                ui.weak("Tekniset tarkistukset ja puheen kohdistus. Ensimmäisen hahmoliikkeen tekeminen ei tarvitse näitä asetuksia.");
                self.native_alignment_ui(ui);
                egui::CollapsingHeader::new("Metal-laitteen tarkistus")
                    .show(ui, |ui| self.gpu_check.ui(ui));
                egui::CollapsingHeader::new("Suorituskyvyn havainnot")
                    .show(ui, |ui| self.ui_metrics.ui(ui));
                egui::CollapsingHeader::new("Puheen ajonaikaiset lisäasetukset")
                    .show(ui, |ui| self.runtime_ui(ui));
                if self.diagnostics_busy {
                    ui.spinner();
                    ui.label("Tarkistetaan paikallista asennusta…");
                }
                if let Some(report) = &self.diagnostics_report {
                    egui::CollapsingHeader::new("Paikallisen asennuksen raportti")
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
        }
    }

    fn export_ui(&mut self, ui: &mut egui::Ui) {
        ui.weak(
            "Vie valittu kohtaus. Valmistele ensin ääni Puhe-välilehdellä. MP4:n mukana säilyvät myös PNG-kuvat ja WAV-ääni.",
        );
        ui.add_enabled_ui(!self.busy && !self.diagnostics_busy, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.export_format, export_worker::Format::Svg, "SVG");
                ui.selectable_value(&mut self.export_format, export_worker::Format::Png, "PNG");
                ui.selectable_value(&mut self.export_format, export_worker::Format::Mp4, "MP4");
                ui.selectable_value(&mut self.export_format, export_worker::Format::StreamingMp4, "MP4 Full HD · suoratoisto");
            });
            if matches!(self.export_format,export_worker::Format::Mp4|export_worker::Format::StreamingMp4) {
                field(ui, "Paikallinen FFmpeg-ohjelma (koko polku)", &mut self.ffmpeg_path);
                ui.weak("Käyttää tätä paikallista ohjelmaa; ei lataa kooderia. MP4:n kuvamittojen on oltava parilliset.");
            }
            if self.export_format==export_worker::Format::StreamingMp4 {ui.weak("Suoratoisto: enintään 60 s / 1920×1080 / 24 fps. Ei PNG-välitiedostoja. Viennin määräaika 300 s; koko tuotantoputken nopeutta ei luvata tällä asetuksella.");}
            field(
                ui,
                "Uusi tuloskansio (koko polku)",
                &mut self.export_path,
            );
            if ui
                .add_enabled(
                    self.scene_preview.is_some() && !self.export_path.trim().is_empty(),
                    egui::Button::new("Vie kohtaus"),
                )
                .clicked()
            {
                self.start_export();
            }
        });
        if self.scene_preview.is_none() {
            ui.weak(
                "Valmistele kohtauksen ääni Puhe-välilehdellä tai lataa yläpalkista Ääniesimerkki.",
            );
        }
        if let Some(progress) = self.export_progress {
            ui.add(
                egui::ProgressBar::new(
                    progress.completed_frames as f32 / progress.total_frames.max(1) as f32,
                )
                .show_percentage(),
            );
            if progress.completed_frames >= progress.total_frames {
                ui.label("Kuvaruudut valmiit; viimeistellään ja pakataan…");
            }
            ui.label(format!(
                "{} / {} kuvaruutua",
                progress.completed_frames, progress.total_frames
            ));
        }
        ui.weak("Anna uusi tuloskansio. Vienti käyttää valmisteltua suun ajoitusta; tekstistä tehty liike on arvio. Jos ajoitusta ei synny, suu pysyy lepoasennossa.");
    }

    fn event_inspector_ui(&mut self, ui: &mut egui::Ui) {
        let mut create = None;
        let mut duplicate = false;
        let mut delete = false;
        ui.add_enabled_ui(
            self.script.is_some() && !self.busy && !self.diagnostics_busy && !self.export_busy,
            |ui| {
                ui.horizontal(|ui| {
                    ui.menu_button("+ Tapahtuma", |ui| {
                        for (label, kind) in [
                            ("Liike", event_factory::Kind::Move),
                            ("Ilme", event_factory::Kind::Emotion),
                            ("Ele", event_factory::Kind::Gesture),
                            ("Suunta", event_factory::Kind::Face),
                            ("Repliikki", event_factory::Kind::Speak),
                        ] {
                            if ui.button(label).clicked() {
                                create = Some(kind);
                                ui.close_menu();
                            }
                        }
                    });
                    let existing = self.selected_event.is_some() && !self.inserting_event;
                    duplicate = ui
                        .add_enabled(existing, egui::Button::new("Kopioi"))
                        .clicked();
                    delete = ui
                        .add_enabled(existing, egui::Button::new("Poista tapahtuma"))
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
            ui.label("Poistetaanko tämä tapahtuma? Kumoa palauttaa sen.");
            ui.add_enabled_ui(
                !self.busy && !self.diagnostics_busy && !self.export_busy,
                |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Vahvista poisto").clicked() {
                            self.confirm_event_delete();
                        }
                        if ui.button("Peru poisto").clicked() {
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
                "VALITTU TAPAHTUMA"
            },
        );
        ui.add_enabled_ui(
            !self.busy && !self.diagnostics_busy && !self.export_busy,
            |ui| {
                if self.inserting_event {
                    cancel = ui.button("Peru luonnos").clicked();
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
            self.status = "Tapahtumaluonnos peruttiin; käsikirjoitus säilyi ennallaan.".into();
        } else if apply {
            self.apply_event_draft();
        }
        if let Some(error) = &self.event_edit_error {
            ui.colored_label(egui::Color32::LIGHT_RED, format!("Virhe: {error}"));
        }
    }

    fn start_export(&mut self) {
        if self.busy || self.diagnostics_busy {
            return;
        }
        let destination = std::path::PathBuf::from(self.export_path.trim());
        if !destination.is_absolute() {
            self.status = "Anna vientiä varten uuden tuloskansion koko polku.".into();
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
            self.status = "Valmistele valitun kohtauksen ääni ennen vientiä.".into();
            return;
        }
        let cancel = CancellationToken::new();
        if self.export_format == export_worker::Format::Mp4
            && !Path::new(self.ffmpeg_path.trim()).is_absolute()
        {
            self.status = "Anna paikallisen FFmpeg-ohjelman koko polku.".into();
            return;
        }
        if let Some(document) = &self.project_document {
            let mut document = document.clone();
            document.source = self.source.clone();
            if let Err(error) = document.validate() {
                self.status = format!("Videon vienti epäonnistui: {error}");
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
                screen_videos: Default::default(),
            },
            format: self.export_format,
            ffmpeg: self.ffmpeg_path.trim().into(),
            script: script.clone(),
            scene_index: self.scene_index,
            audio: Arc::clone(&preview.audio),
            manifest: Arc::clone(&preview.manifest),
            destination,
            captions: self.caption_mode,
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
                    "Viedään {:?} ja valmisteltua kohtausääntä…",
                    self.export_format
                );
            }
            Err(error) => self.status = format!("Videon vienti epäonnistui: {error}"),
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
            self.status = "Aikajanan muokkaus peruttiin.".into();
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("AIKAJANA").strong());
            ui.add_enabled_ui(
                self.script.is_some() && !self.busy && !self.diagnostics_busy,
                |ui| {
                    if ui.button("Toista liike").clicked() {
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
                            egui::Button::new("Toista ääni"),
                        )
                        .clicked()
                    {
                        self.play_scene_audio();
                    }
                    if ui.button("Tauko").clicked() {
                        if let Some(clock) = &mut self.scene_transport {
                            clock.pause();
                        }
                        if let Some(player) = &self.playback {
                            if let Err(error) = player.pause() {
                                self.playback_error = Some(error);
                            }
                        }
                    }
                    if ui.button("Jatka").clicked() {
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
                    if ui.button("Alkuun").clicked() {
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
                    ("Puhe", SPEAK),
                    ("Suunta", FACE),
                    ("Ele", GESTURE),
                    ("Ilme", EMOTION),
                    ("Liike", MOVE),
                ] {
                    ui.colored_label(color, name);
                }
            });
        });
        if let Some(error) = &self.playback_error {
            ui.colored_label(egui::Color32::LIGHT_RED, format!("Virhe: {error}"));
        }
        let Some(script) = &self.script else {
            ui.weak("Paina Tarkista, jotta hahmojen raidat näkyvät aikajanalla.");
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
                motion_script::Event::Move { duration, gait, .. } => (
                    0usize,
                    event_editor::gait_label(gait.as_deref().unwrap_or("walk")).to_owned(),
                    MOVE,
                    *duration,
                ),
                motion_script::Event::Face { value, .. } => {
                    (1, event_editor::facing_label(value).to_owned(), FACE, 0.0)
                }
                motion_script::Event::Pose { value, .. } => {
                    (1, event_editor::pose_label(value).to_owned(), FACE, 0.0)
                }
                motion_script::Event::Hold { item, .. } => (
                    2,
                    item.as_deref().unwrap_or("laskee esineen").to_owned(),
                    GESTURE,
                    0.0,
                ),
                motion_script::Event::Emotion { value, .. } => (
                    1,
                    event_editor::expression_label(value).to_owned(),
                    EMOTION,
                    0.0,
                ),
                motion_script::Event::Gesture {
                    value, duration, ..
                } => (
                    2,
                    event_editor::gesture_label(value).to_owned(),
                    GESTURE,
                    *duration,
                ),
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
        let mut drop = None;
        let editable = !self.busy && !self.diagnostics_busy && !self.export_busy;
        let saved_blocks = motion_blocks::saved(script, self.selected_actor.as_deref());
        egui::SidePanel::left("motion_block_palette")
            .resizable(true)
            .default_width(170.0)
            .width_range(140.0..=320.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("motion_block_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("PALIKAT").strong());
                        ui.weak("Vedä palikka hahmon raidalle.");
                        for (heading, blocks) in motion_blocks::builtin() {
                            ui.add_space(4.0);
                            ui.small(heading);
                            ui.horizontal_wrapped(|ui| {
                                for block in blocks {
                                    let _ = block_chip(ui, block, editable);
                                }
                            });
                        }
                        ui.add_space(4.0);
                        ui.small("Omat liikkeet");
                        if saved_blocks.is_empty() {
                            ui.weak("Tallenna liike Muokkaa-välilehden liikekirjastoon, niin se näkyy tässä.");
                        }
                        ui.horizontal_wrapped(|ui| {
                            for block in saved_blocks {
                                let _ = block_chip(ui, block, editable);
                            }
                        });
                    });
            });
        ui.add_space(4.0);
        // Reserve the audio section's height measured on the previous frame. Content
        // taller than the panel would otherwise grow the resizable panel every frame.
        let audio_height_id = ui.id().with("timeline_audio_height");
        let audio_height = ui
            .data(|data| data.get_temp::<f32>(audio_height_id))
            .unwrap_or(26.0);
        egui::ScrollArea::vertical().id_salt("actor_tracks").max_height((ui.available_height() - audio_height - 4.0).max(48.0)).show(ui, |ui| {
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
                painter.text(egui::pos2(rect.left()+10.0, y+8.0), egui::Align2::LEFT_TOP, actor, egui::FontId::proportional(13.0), if self.selected_actor.as_ref() == Some(actor) { ACCENT } else { egui::Color32::from_gray(210) });
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
            // Palette drops: the pointer picks the character track and the time.
            let drop_target = |position: egui::Pos2| -> Option<(String, f64, f32)> {
                if position.x < track_left || position.y < rect.top() + 26.0 {
                    return None;
                }
                let track = ((position.y - rect.top() - 26.0) / track_height) as usize;
                let actor = actor_ids.get(track)?;
                let time = timeline_seek(position.x, track_left, track_width, duration);
                Some((actor.clone(), time, rect.top() + 26.0 + track as f32 * track_height))
            };
            if let (Some(block), Some(position)) = (
                response.dnd_hover_payload::<motion_blocks::Block>(),
                ui.input(|input| input.pointer.latest_pos()),
            ) {
                if let Some((actor, time, top)) = drop_target(position) {
                    let row = egui::Rect::from_min_max(egui::pos2(track_left, top), egui::pos2(track_left + track_width, top + track_height));
                    painter.rect_filled(row, 0.0, ACCENT.gamma_multiply(0.08));
                    let x = x_at(time);
                    painter.line_segment([egui::pos2(x, top), egui::pos2(x, top + track_height)], egui::Stroke::new(2.0_f32, ACCENT));
                    painter.text(egui::pos2(x + 4.0, top + 2.0), egui::Align2::LEFT_TOP, format!("{} → {actor} · {time:.2} s", block.label()), egui::FontId::proportional(11.0), ACCENT);
                }
            }
            if let (Some(block), Some(position)) = (
                response.dnd_release_payload::<motion_blocks::Block>(),
                ui.input(|input| input.pointer.latest_pos()),
            ) {
                match drop_target(position) {
                    Some((actor, time, _)) => drop = Some(((*block).clone(), actor, time)),
                    None => self.status = "Pudota palikka jonkin hahmon raidalle.".into(),
                }
            }
            // The last few pixels of a bar resize it; the rest moves it.
            let edge_at = |position: egui::Pos2| {
                hit_boxes.iter().rev().find(|(_, bar)| {
                    (position.x - bar.right()).abs() <= 5.0 && position.y >= bar.top() && position.y <= bar.bottom() && bar.width() > 10.0
                }).map(|(index, _)| *index)
            };
            if !self.busy && !self.diagnostics_busy {
                if response.drag_started_by(egui::PointerButton::Primary) {
                    if let Some(origin) = ui.input(|input| input.pointer.press_origin()) {
                        let resize = edge_at(origin).and_then(|index| {
                            timeline_edit::Drag::resize(self.scene_index, index, &scene.events[index], origin.x).map(|drag| (index, drag))
                        });
                        if let Some((index, drag)) = resize {
                            self.timeline_drag = Some(drag);
                            self.timeline_drag_source = Some(self.source.clone());
                            self.timeline_seek_drag = false;
                            selection = Some(index);
                            pause_drag = true;
                        } else if let Some(index) = timeline_event_at(origin, &hit_boxes) {
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
                                self.status = format!("Aikajanan muokkaus: {error}");
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
                        let length = if drag.resize { drag.proposed_duration } else { *length };
                        let start=x_at(drag.proposed_at);
                        let end=x_at(drag.proposed_at+length).max(start+8.0).min(track_left+track_width);
                        let ghost=egui::Rect::from_min_max(egui::pos2(start.min(end-2.0),y),egui::pos2(end,y+15.0));
                        painter.rect_filled(ghost,3.0,ACCENT.gamma_multiply(0.25));
                        painter.rect_stroke(ghost,3.0,egui::Stroke::new(2.0_f32,ACCENT),egui::StrokeKind::Inside);
                        let text = if drag.resize { format!("kesto {length:.3} s · käytä vapauttamalla hiiri") } else { format!("{:.3} s · käytä vapauttamalla hiiri",drag.proposed_at) };
                        painter.text(egui::pos2(start,rect.top()+2.0),egui::Align2::LEFT_TOP,text,egui::FontId::monospace(11.0),ACCENT);
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
                if self.timeline_drag.as_ref().is_some_and(|d| d.resize) || edge_at(position).is_some_and(|index| matches!(scene.events.get(index), Some(motion_script::Event::Move { .. } | motion_script::Event::Gesture { .. }))) {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }
                if let Some((index,_)) = hit_boxes.iter().rev().find(|(_,bar)|bar.contains(position)) {
                    if let Some((_,_,actor,at,length,_,label,_)) = events.iter().find(|(i,..)|i==index) {
                        response.on_hover_text(format!("{actor} · {label}\n{at:.3} s{}\nMuokkaa napsauttamalla. Siirrä aikaa vetämällä, muuta kestoa oikeasta reunasta; Esc peruu.", if *length > 0.0 { format!(" · kesto {length:.3} s") } else { " · pistemerkki / kesto valmistelematta".into() }));
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
        if let Some((block, actor, time)) = drop {
            self.apply_block_drop(&block, &actor, time);
        }
        let audio_top = ui.cursor().top();
        let audio_max = (ui.available_height() - 24.0).max(40.0);
        let audio = egui::CollapsingHeader::new("Kohtauksen ääni ja yhdistetty aaltomuoto").show(
            ui,
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("timeline_audio_scroll")
                    .max_height(audio_max)
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!self.busy && !self.diagnostics_busy, |ui| {
                            self.scene_audio_ui(ui);
                        });
                    });
            },
        );
        // Measure the section itself: the palette beside it spans the full height.
        let bottom = audio
            .body_response
            .map_or(audio.header_response.rect.bottom(), |body| {
                body.rect.bottom()
            });
        let used = (bottom - audio_top).max(0.0);
        ui.data_mut(|data| data.insert_temp(audio_height_id, used));
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
const FACE: egui::Color32 = egui::Color32::from_rgb(222, 112, 150);

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
        .insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
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
/// A draggable palette block. Disabled blocks still show what is available.
fn block_chip(ui: &mut egui::Ui, block: motion_blocks::Block, enabled: bool) -> egui::Response {
    let color = match block {
        motion_blocks::Block::Gait(_) => MOVE,
        motion_blocks::Block::Gesture(_) => GESTURE,
        motion_blocks::Block::Face(_) => FACE,
        motion_blocks::Block::Clip(_) => ACCENT,
    };
    let full = block.label();
    let hint = block.hint();
    let padding = ui.spacing().button_padding.x * 2.0 + 2.0;
    let label = chip_label(ui, &full, ui.max_rect().width() - padding);
    // A sized button (never wrapping its text) so `horizontal_wrapped` moves a
    // chip that does not fit to the next row instead of squeezing its letters.
    let chip = |label: &str| {
        egui::Button::new(label)
            .wrap_mode(egui::TextWrapMode::Extend)
            .fill(color.gamma_multiply(0.3))
            .stroke(egui::Stroke::new(1.0_f32, color))
            .corner_radius(4.0)
    };
    if !enabled {
        let response = ui.add_enabled(false, chip(&label));
        return if label != full {
            response.on_disabled_hover_text(full)
        } else {
            response
        };
    }
    let response = ui.add(chip(&label).sense(egui::Sense::drag()));
    response.dnd_set_drag_payload(block);
    if response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        if let Some(pointer) = ui.ctx().pointer_interact_pos() {
            egui::Area::new(response.id.with("ghost"))
                .order(egui::Order::Tooltip)
                .interactable(false)
                .fixed_pos(pointer + egui::vec2(10.0, 6.0))
                .show(ui.ctx(), |ui| {
                    ui.add(chip(&label));
                });
        }
    }
    let hover = if label == full {
        hint.to_owned()
    } else {
        format!("{full}\n\n{hint}")
    };
    response.on_hover_text(hover)
}

/// The chip text, shortened with an ellipsis when it is wider than the palette
/// (`max_width`); the hover text keeps the full name.
fn chip_label(ui: &egui::Ui, full: &str, max_width: f32) -> String {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let width = |text: &str| {
        ui.fonts(|fonts| {
            fonts
                .layout_no_wrap(text.to_owned(), font.clone(), egui::Color32::WHITE)
                .size()
                .x
        })
    };
    if width(full) <= max_width {
        return full.to_owned();
    }
    let mut chars: Vec<char> = full.chars().collect();
    while chars.len() > 1 {
        chars.pop();
        let short: String = chars.iter().collect::<String>() + "…";
        if width(&short) <= max_width {
            return short;
        }
    }
    "…".to_owned()
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

/// Finnish explanation shown when the strict policy blocks a step.
fn policy_block_text(step: motion_project::ModelBackedStep) -> &'static str {
    use motion_project::ModelBackedStep::*;
    match step {
        SpeechSynthesis => "No AI -tila: puhesynteesi on estetty. Tuo jokaiselle repliikille 24 kHz mono -WAV.",
        RecognizerAlignment => "No AI -tila: puheentunnistukseen perustuva kohdistus on estetty. Anna suun ajoitus käsin tai käytä sääntöpohjaista arviota.",
        CameraFaceTracking => "No AI -tila: kameran kasvonseuranta on estetty. Animoi pää käsin.",
        ModelDownload => "No AI -tila: mallien asennus, lataus ja testaus on estetty.",
    }
}

impl Studio {
    fn set_model_policy(&mut self, policy: motion_project::ModelPolicy) {
        self.model_policy = policy;
        if policy.is_strict() {
            self.webcam_pose.set_context("strict model-free policy");
            self.native_alignment.cancel();
        }
    }
    /// The recorder becomes the only owner of a loaded log, so later screenplay
    /// edits are checked against it only when the project is saved.
    fn load_event_log(&mut self, document: &mut motion_project::ProjectDocument) {
        match ui_levels::DragRecorder::from_project(document.event_log.take()) {
            Ok(recorder) => self.drag_recorder = recorder,
            Err(error) => {
                self.drag_recorder = Default::default();
                self.status = format!("Tapahtumalokia ei ladattu: {error}");
            }
        }
    }
    /// Desktop check point before a model-backed step (ADR-A01-09).
    fn permit_model(&mut self, step: motion_project::ModelBackedStep) -> bool {
        match self.model_policy.permit(step) {
            Ok(()) => true,
            Err(_) => {
                self.status = policy_block_text(step).into();
                false
            }
        }
    }
    /// Strict projects get a provider whose every call fails, so no synthesis can
    /// run even if a check point were missed.
    fn speech_provider(&self) -> Arc<dyn motion_tts::SpeechProvider> {
        if self.model_policy.is_strict() {
            Arc::new(motion_tts::DisabledSpeechProvider)
        } else {
            Arc::new(self.local_provider())
        }
    }
    fn level_and_policy_ui(&mut self, ui: &mut egui::Ui) {
        if ui_levels::level_picker(ui, &mut self.ui_level) {
            if !self.ui_level.shows(ui_levels::Tool::AdvancedTab)
                && self.inspector_tab == InspectorTab::Advanced
            {
                self.inspector_tab = InspectorTab::Edit;
            }
            self.status = format!(
                "Käyttöliittymätaso {}: {} Taso on näkymäasetus eikä tallennu projektiin.",
                self.ui_level.number(),
                self.ui_level.summary()
            );
        }
        ui.separator();
        let mut strict = self.model_policy.is_strict();
        let idle = self.editing_idle() && !self.setup_busy;
        if ui
            .add_enabled(idle, egui::Checkbox::new(&mut strict, "No AI"))
            .on_hover_text("Projektikohtainen tiukka mallivapaa tila: ei puhesynteesiä, puheentunnistusta, kameran kasvonseurantaa eikä mallien latausta. Repliikit tarvitsevat tuodun WAV-äänen. Tallennetaan .kilsat-projektiin.")
            .changed()
        {
            self.set_model_policy(if strict {
                motion_project::ModelPolicy::Strict
            } else {
                motion_project::ModelPolicy::LocalPreprocessing
            });
            self.status = if strict {
                "No AI -tila päällä: mallit eivät käynnisty. Tallenna .kilsat säilyttääksesi valinnan.".into()
            } else {
                "No AI -tila pois: paikalliset puhemallit sallittu. Tallenna .kilsat säilyttääksesi valinnan.".into()
            };
        }
    }
    /// Level 2: mouth shape at the playhead from prepared scene audio.
    fn viseme_check_ui(&self, ui: &mut egui::Ui) {
        ui.separator();
        ui.label("Viseemitarkistus");
        let Some(preview) = &self.scene_preview else {
            ui.weak("Valmistele kohtauksen ääni Puhe-välilehdellä nähdäksesi suun muodot toistopään kohdalla.");
            return;
        };
        let Ok(sample) = motion_audio::clock::seconds_to_sample(self.playhead) else {
            return;
        };
        let mouths = scene_mouths(&preview.clips, sample);
        if mouths.is_empty() {
            ui.weak("Toistopään kohdalla ei ole repliikkiä.");
        }
        for (actor, viseme) in mouths {
            ui.label(format!("{actor}: {}", viseme_label(viseme)));
        }
    }
    /// Level 3: read-only rule list from the validated screenplay.
    fn rule_list_ui(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Sääntöluettelo").show(ui, |ui| {
            let Some(script) = &self.script else {
                ui.weak("Tarkista käsikirjoitus nähdäksesi säännöt.");
                return;
            };
            if script.rules.is_empty() {
                ui.weak("Käsikirjoituksessa ei ole sääntöjä (rules:).");
            }
            for (name, rule) in &script.rules {
                let effect = serde_json::to_value(&rule.effect)
                    .ok()
                    .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(str::to_owned))
                    .unwrap_or_else(|| "?".into());
                let value = rule.value.as_deref().unwrap_or("*");
                ui.label(format!("{name}: kun {:?} ({value}) → {effect}", rule.on));
            }
            ui.weak("Muokkaa sääntöjä käsikirjoituksessa tai vihjesääntöeditorilla.");
        });
    }
}

impl eframe::App for Studio {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let measurement_start = self.ui_metrics.enabled.then(Instant::now);
        let has_input = self.ui_metrics.latency.enabled
            && ctx.input(|i| input_latency::has_discrete_input(&i.events));
        if self
            .ui_metrics
            .latency
            .begin_frame(Instant::now(), has_input)
        {
            ctx.request_repaint();
        }
        let background_work =
            self.busy || self.export_busy || self.diagnostics_busy || self.sequence_panel.busy();
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
            || self.screen_videos.busy()
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
        if self.character_drag.is_none() {
            self.drag_recorder.cancel();
        }
        self.sync_sequence_resources();
        self.ui_metrics.latency.end_frame(Instant::now());
        if let Some(start) = measurement_start {
            self.ui_metrics.record(start.elapsed(), background_work);
        }
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
    ui.label("Suun muodon havainnollinen näkymä. Kohtauksen esikatselu käyttää muotoa oikealla hahmolla.");
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
        || art.lights.iter().any(|light| light.scene_id == scene_id)
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
        // The back view shows no face; the side view turns it toward local +x.
        let Some((k, shift)) = motion_scene::procedural_face(actor.view) else {
            continue;
        };
        let (k, shift) = (k as f32, shift as f32);
        let face = |dx: f32, dy: f32| face(dx * k + shift, dy);
        let ellipse = |cx: f32, cy: f32, rx: f32, ry: f32, color: egui::Color32| {
            ellipse(cx * k + shift, cy, rx, ry, color)
        };
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
            format!(
                "{} · {}",
                actor.id,
                event_editor::expression_label(&actor.emotion)
            ),
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
        assert!(studio.status.contains("keskeneräinen"));
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
        assert!(studio.status.contains("Kuvitus tarvitsee .kilsat"));
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
        assert!(studio.status.contains("hylättiin"));
        studio.project_root = root.join("missing-root").to_string_lossy().into();
        studio.start_asset(asset_worker::Operation::Import {
            source: root.join("image.dat"),
            id: "new".into(),
            kind: motion_project::AssetKind::Other,
        });
        assert!(!root.join("resources").exists());
        assert!(!studio.busy);
        assert!(studio.status.contains("asetettuun projektikansioon"));
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
            text_estimated: None,
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
        assert!(studio.status.contains("suhteelliset polut"));
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
        assert!(studio.status.contains("Aikajanan muokkaus hylättiin"));
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
    fn automatic_direction_appends_shots_with_project_undo() {
        let mut studio = settings_studio();
        let original = studio.source.clone();
        studio.auto_direct_dialogue();
        assert_eq!(studio.scene_index, 1);
        assert_eq!(studio.script.as_ref().unwrap().scenes.len(), 3);
        assert!(studio.status.contains("Ohjausluonnos"));
        let directed = studio.source.clone();
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        studio.undo_edit(true);
        assert_eq!(studio.source, directed);
        studio.busy = true;
        studio.auto_direct_dialogue();
        assert_eq!(studio.source, directed);
    }

    #[test]
    fn dialogue_scene_creation_is_selected_and_undoable() {
        let mut studio = settings_studio();
        let original = studio.source.clone();
        studio.create_dialogue_scene();
        assert_eq!(studio.scene_index, 1);
        assert_eq!(studio.script.as_ref().unwrap().scenes.len(), 2);
        let created = studio.source.clone();
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        studio.undo_edit(true);
        assert_eq!(studio.source, created);
        studio.busy = true;
        studio.create_dialogue_scene();
        assert_eq!(studio.source, created);
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
        assert!(studio.settings_error.as_ref().unwrap().contains("vaihtui"));
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
                    text_estimated: None,
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
            assert!(studio.status.contains("ajoitus muuttui"));
        }
    }
    #[test]
    fn timing_status_distinguishes_bindings_without_claiming_a_mouth_source() {
        assert!(alignment_status_text(AlignmentStatus::LegacyUnbound).contains("ei sidottu"));
        assert!(alignment_status_text(AlignmentStatus::BoundCacheHit).contains("löytyi"));
        assert!(alignment_status_text(AlignmentStatus::BoundCacheMiss).contains("ei löytynyt"));
        assert!(alignment_status_text(AlignmentStatus::None).contains("Ei tuotua"));
        assert!(!alignment_status_text(AlignmentStatus::None).contains("lepoasennossa"));
        assert!(alignment_status_text(AlignmentStatus::BoundImported).contains("laatu mittaamatta"));
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
    fn cue_and_rule_drafts_apply_undo_and_reject_stale_context() {
        let mut studio = Studio::default();
        studio.validate();
        let original = studio.source.clone();
        let script = studio.script.as_ref().unwrap();
        let scene = &script.scenes[0];
        let mut cues = cue_rule_editor::CueDraft::new(scene, 0);
        cues.cues.push(motion_script::AudioCue {
            id: "door".into(),
            asset: "audio/door.wav".into(),
            at: 0.5,
            gain: 1.0,
            trim_start: 0.0,
            duration: None,
        });
        studio.cue_draft = Some((studio.source.clone(), cues));
        studio.apply_cue_draft();
        let with_cue = studio.source.clone();
        assert_ne!(with_cue, original);
        assert_eq!(
            studio.script.as_ref().unwrap().scenes[0]
                .composition
                .audio_cues[0]
                .id,
            "door"
        );
        let mut rules = cue_rule_editor::RuleDraft::new(&studio.script.as_ref().unwrap().rules);
        rules.add_rule();
        studio.rule_draft = Some((studio.source.clone(), rules));
        studio.apply_rule_draft();
        assert!(!studio.script.as_ref().unwrap().rules.is_empty());
        studio.undo_edit(false);
        assert_eq!(studio.source, with_cue);
        studio.undo_edit(false);
        assert_eq!(studio.source, original);

        for context in 0..4 {
            let mut studio = Studio::default();
            studio.validate();
            let original = studio.source.clone();
            let scene = &studio.script.as_ref().unwrap().scenes[0];
            let mut cues = cue_rule_editor::CueDraft::new(scene, 0);
            cues.cues.push(motion_script::AudioCue {
                id: "door".into(),
                asset: "audio/door.wav".into(),
                at: 0.5,
                gain: 1.0,
                trim_start: 0.0,
                duration: None,
            });
            studio.cue_draft = Some((studio.source.clone(), cues));
            match context {
                0 => studio.cue_draft.as_mut().unwrap().0.push(' '),
                1 => studio.scene_index += 1,
                2 => studio.cue_draft.as_mut().unwrap().1.scene_id = "other".into(),
                _ => studio.cue_draft.as_mut().unwrap().1.cues[0].gain = 9.0,
            }
            studio.apply_cue_draft();
            assert_eq!(studio.source, original);
            assert_eq!(studio.cue_draft.is_none(), context != 3);
        }
        let mut studio = Studio::default();
        studio.validate();
        let original = studio.source.clone();
        let mut rules = cue_rule_editor::RuleDraft::new(&Default::default());
        rules.rows.clear();
        studio.rule_draft = Some((format!("{original} "), rules));
        studio.apply_rule_draft();
        assert_eq!(studio.source, original);
        assert!(studio.rule_draft.is_none());
        studio.cue_draft = Some((
            original.clone(),
            cue_rule_editor::CueDraft::new(&studio.script.as_ref().unwrap().scenes[0], 0),
        ));
        studio.invalidate_scene();
        assert!(studio.cue_draft.is_none() && studio.rule_draft.is_none());
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
        assert!(studio.status.contains("Aikajanan muokkaus"));
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
                text_estimated: None,
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

#[cfg(test)]
mod custom_rig_texture_skin_apply_tests {
    use super::*;

    fn studio_with_rig_draft(
        asset_kind: Option<motion_project::AssetKind>,
    ) -> (Studio, String, custom_rig_editor::Draft) {
        let mut studio = Studio::default();
        studio.validate();
        studio.selected_actor = Some("ALEX".into());
        let mut document = motion_project::ProjectDocument::new(studio.source.clone()).unwrap();
        if let Some(kind) = asset_kind {
            document.assets.push(motion_project::AssetReference {
                id: "skin".into(),
                path: "skin.png".into(),
                kind,
            });
        }
        studio.project_document = Some(document);
        let original = studio.source.clone();
        let script = motion_script::parse(&original).unwrap();
        let mut draft = custom_rig_editor::Draft::for_actor(&script, "ALEX");
        draft.yaml = draft.yaml.replace(
            "    color: [70, 160, 220, 255]\n",
            "    color: [70, 160, 220, 255]\n    texture: {asset: skin, uv: [0, 0, 0.5, 1]}\n    skin:\n      - [{joint: root, weight: 0.5}, {joint: head, weight: 0.5}]\n      - [{joint: root, weight: 1}]\n      - [{joint: root, weight: 1}]\n      - [{joint: root, weight: 1}]\n",
        );
        (studio, original, draft)
    }

    #[test]
    fn texture_and_skin_apply_requires_image_asset_and_valid_weights_and_is_undoable() {
        // Missing inventory entry, or a non-image asset: project is untouched.
        for kind in [None, Some(motion_project::AssetKind::Audio)] {
            let (mut studio, original, draft) = studio_with_rig_draft(kind);
            studio.custom_rig_draft = Some((original.clone(), draft));
            studio.apply_custom_rig_draft(1);
            assert_eq!(studio.source, original);
            assert!(!studio.history.can_undo());
        }
        // Invalid weights are rejected even with a valid image asset.
        let (mut studio, original, mut draft) =
            studio_with_rig_draft(Some(motion_project::AssetKind::Image));
        let valid_yaml = draft.yaml.clone();
        draft.yaml = valid_yaml.replace(
            "weight: 0.5}, {joint: head, weight: 0.5",
            "weight: 0.9}, {joint: head, weight: 0.5",
        );
        studio.custom_rig_draft = Some((original.clone(), draft));
        studio.apply_custom_rig_draft(1);
        assert_eq!(studio.source, original);
        assert!(!studio.history.can_undo());
        // The valid draft applies atomically, persists and undoes.
        let (mut studio, original, draft) =
            studio_with_rig_draft(Some(motion_project::AssetKind::Image));
        studio.custom_rig_draft = Some((original.clone(), draft));
        studio.apply_custom_rig_draft(1);
        assert_ne!(studio.source, original);
        let rig = motion_script::parse(&studio.source).unwrap().characters["ALEX"]
            .rig
            .clone()
            .unwrap();
        assert_eq!(rig.parts[0].texture.as_ref().unwrap().asset, "skin");
        assert_eq!(rig.parts[0].skin.len(), 4);
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
    }

    #[test]
    fn stale_texture_skin_draft_is_rejected() {
        let (mut studio, original, draft) =
            studio_with_rig_draft(Some(motion_project::AssetKind::Image));
        let mut stale = original.clone();
        stale.push('\n');
        studio.custom_rig_draft = Some((stale, draft));
        studio.apply_custom_rig_draft(1);
        assert_eq!(studio.source, original);
        assert!(studio.status.contains("vaihtui"));
        let (mut studio, original, mut draft) =
            studio_with_rig_draft(Some(motion_project::AssetKind::Image));
        draft.actor = "EMMA".into();
        studio.custom_rig_draft = Some((original.clone(), draft));
        studio.apply_custom_rig_draft(1);
        assert_eq!(studio.source, original);
        assert!(!studio.history.can_undo());
    }
}

#[cfg(test)]
mod timing_persistence_tests {
    use super::*;
    #[test]
    fn timing_invalidations_survive_source_undo_redo_and_yaml_cannot_drop_timing() {
        let mut studio = Studio::default();
        studio.source = "project: {}\ncharacters: {}\nscenes:\n  - id: timing\n    duration: 1\n    events: []\n".into();
        studio.validate();
        studio
            .commit_reviewed_timing("{}".into(), "{}".into())
            .unwrap();
        let before = studio.source.clone();
        let after = before.replace("project: {}", "project: {title: Changed}");
        studio.record_source_edit(&before, &after);
        studio.source = after.clone();
        studio.alignment.clear();
        studio.scene_alignment.clear();
        studio.undo_edit(false);
        assert_eq!(studio.source, before);
        assert_eq!(studio.alignment, "{}");
        studio.undo_edit(true);
        assert_eq!(studio.source, after);
        assert!(studio.alignment.is_empty());
        assert!(studio.scene_alignment.is_empty());
        studio
            .commit_reviewed_timing("{}".into(), "{}".into())
            .unwrap();
        let path = std::env::temp_dir().join(format!(
            "kilsat-unsaved-timing-{}-{}.yaml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        studio.script_path = path.to_string_lossy().into_owned();
        studio.save_script();
        assert!(!path.exists());
        assert!(studio.status.contains("tarvitsee .kilsat"));
        assert_eq!(studio.alignment, "{}");
    }
    #[test]
    fn reviewed_timing_is_one_project_history_transaction_and_roundtrips() {
        let mut studio = Studio::default();
        studio.source = "project: {}\ncharacters: {}\nscenes:\n  - id: timing\n    duration: 1\n    events: []\n".into();
        studio.validate();
        studio
            .commit_reviewed_timing("{}".into(), "{\"0\":{}}".into())
            .unwrap();
        let saved = motion_project::encode(studio.project_document.as_ref().unwrap()).unwrap();
        let reopened = motion_project::decode(&saved).unwrap();
        assert_eq!(reopened.timing.line, "{}");
        assert_eq!(reopened.timing.scene, "{\"0\":{}}");
        studio.undo_edit(false);
        assert!(studio.alignment.is_empty());
        assert!(studio.scene_alignment.is_empty());
        studio.undo_edit(true);
        assert_eq!(studio.alignment, "{}");
        assert_eq!(studio.scene_alignment, "{\"0\":{}}");
        let before = studio.snapshot();
        assert!(studio
            .commit_reviewed_timing("invalid".into(), "{}".into())
            .is_err());
        assert_eq!(studio.snapshot(), before);
    }

    fn drawn_text(output: &egui::FullOutput) -> String {
        let mut text = String::new();
        for clipped in &output.shapes {
            if let egui::epaint::Shape::Text(shape) = &clipped.shape {
                text.push_str(shape.galley.text());
                text.push('\n');
            }
        }
        text
    }

    #[test]
    fn caption_control_draws_headless_and_the_override_never_touches_screenplay_or_history() {
        let ctx = egui::Context::default();
        studio_theme(&ctx);
        let mut studio = Studio::default();
        studio.validate();
        let source = studio.source.clone();
        assert_eq!(studio.caption_mode, motion_script::CaptionMode::Follow);
        let mut changed = true;
        for (mode, label) in [
            (
                motion_script::CaptionMode::Follow,
                "Käsikirjoituksen mukaan",
            ),
            (motion_script::CaptionMode::Show, "Näytä"),
            (motion_script::CaptionMode::Hide, "Piilota"),
        ] {
            let mut shown = mode;
            let output = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    changed = caption_mode_ui(ui, &mut shown);
                });
            });
            let text = drawn_text(&output);
            assert!(text.contains("Tekstitys") && text.contains(label), "{text}");
            assert!(!changed && shown == mode, "drawing alone never changes it");
        }
        // The full inspector hosts the control and keeps the sequence panel in step.
        studio.caption_mode = motion_script::CaptionMode::Hide;
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| studio.inspector_ui(ui));
        });
        assert!(drawn_text(&output).contains("Tekstitys"));
        // A view/export option: no screenplay edit, no undo entry.
        assert_eq!(studio.source, source);
        assert!(!studio.history.can_undo() && !studio.history.can_redo());
        assert_eq!(studio.caption_mode, motion_script::CaptionMode::Hide);
    }
}

#[cfg(test)]
mod direct_manipulation_history_tests {
    use super::*;
    #[test]
    fn drag_is_one_undo_and_rejects_stale_source() {
        let mut studio = Studio::default();
        studio.validate();
        let original = studio.source.clone();
        let scene = motion_scene::evaluate(studio.script.as_ref().unwrap(), 0, 0.0).unwrap();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(960.0, 540.0));
        let drag =
            direct_manipulation::Drag::new(&original, 0, "EMMA", 0.0, rect, &scene, rect.center())
                .unwrap();
        studio.finish_character_drag(drag, &scene, rect, rect.center() + egui::vec2(30.0, 0.0));
        assert_ne!(studio.source, original);
        assert!(studio.history.can_undo());
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        assert!(!studio.history.can_undo());
        let drag =
            direct_manipulation::Drag::new(&original, 0, "EMMA", 0.0, rect, &scene, rect.center())
                .unwrap();
        studio.source.push('\n');
        let stale = studio.source.clone();
        studio.finish_character_drag(drag, &scene, rect, rect.center() + egui::vec2(30.0, 0.0));
        assert_eq!(studio.source, stale);
    }
    #[test]
    fn creation_and_episode_undo_restore_document() {
        let mut studio = Studio::default();
        studio.validate();
        studio.create_empty_scene();
        assert_eq!(studio.scene_index, 1);
        assert_eq!(studio.script.as_ref().unwrap().scenes.len(), 2);
        let original = studio.source.clone();
        let mut document = motion_project::ProjectDocument::new(original.clone()).unwrap();
        document.art.layers.push(motion_art::Layer {
            id: "kept".into(),
            name: "Kept artwork".into(),
            scene_id: "intro".into(),
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
        studio.project_document = Some(document);
        studio.script_path = "existing.kilsat".into();
        studio.episode_title = "Fresh".into();
        studio.create_new_episode();
        assert_eq!(studio.script.as_ref().unwrap().project.title, "Fresh");
        assert!(studio.script_path.is_empty());
        assert!(studio.project_document.is_none());
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        assert_eq!(
            studio.project_document.as_ref().unwrap().art.layers[0].id,
            "kept"
        );
    }
}

#[cfg(test)]
mod stick_studio_tests {
    use super::*;

    fn assert_starter(studio: &Studio) {
        let script = studio.script.as_ref().expect(&studio.status);
        assert_eq!(script.characters.len(), 6, "{}", studio.status);
        assert_eq!(script.scenes.len(), 6);
        for actor in ["NOUTAJA", "VEIKKO", "PASMA", "KIPINA", "KIIRE", "SISU"] {
            let rig = script.characters[actor].rig.as_ref().unwrap();
            assert_eq!(rig.joints.len(), 15);
            assert!(rig.joints.iter().any(|joint| joint.id == "head"));
        }
        assert!(studio.jobs.is_empty());
        let document = studio.project_document.as_ref().unwrap();
        assert!(document.assets.is_empty());
        assert!(document.timing.is_empty());
        assert!(!document.art.layers.is_empty());
        let mut current = document.clone();
        current.source = studio.source.clone();
        current.validate().unwrap();
    }

    #[test]
    fn embedded_stick_project_loads_and_undo_restores_source_and_artwork() {
        let mut studio = Studio::default();
        studio.validate();
        let mut document = motion_project::ProjectDocument::new(studio.source.clone()).unwrap();
        document.art.layers.push(motion_art::Layer {
            id: "kept".into(),
            name: "Kept artwork".into(),
            scene_id: "intro".into(),
            visible: true,
            plane: motion_art::Plane::Background,
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
        studio.project_document = Some(document);
        studio.script_path = "existing.kilsat".into();
        let original = studio.snapshot();

        studio.load_stick_studio();
        assert_starter(&studio);
        assert!(studio.script_path.is_empty());
        assert_eq!(studio.selected_actor.as_deref(), Some("NOUTAJA"));
        assert_eq!(studio.selected_joint, "head");
        assert!(studio.inspector_tab == InspectorTab::Edit);
        assert!(studio.character_mode && !studio.editor_tab);
        assert!(studio.history.can_undo());
        let starter = studio.snapshot();

        studio.undo_edit(false);
        assert_eq!(studio.snapshot(), original);
        assert_eq!(
            studio.project_document.as_ref().unwrap().art.layers[0].id,
            "kept"
        );
        assert!(studio.script_path.is_empty());
        assert!(!studio.history.can_undo());
        studio.undo_edit(true);
        assert_eq!(studio.snapshot(), starter);
        assert_starter(&studio);
    }

    #[test]
    fn actual_head_edit_over_100kb_roundtrips_through_history_and_saved_project() {
        let mut studio = Studio::default();
        studio.validate();
        studio.load_stick_studio();
        assert_starter(&studio);
        let before = studio.snapshot();
        let mut draft =
            direct_manipulation::HeadDraft::new(studio.script.as_ref().unwrap(), 0, "NOUTAJA", 1.0)
                .unwrap();
        assert!(draft.supports_tilt);
        draft.tilt_degrees = 12.0;
        let edited = draft.apply(&studio.source).unwrap();
        // Normal editor serialization expands compact rig YAML beyond the old cap.
        assert!(edited.len() > 100_000, "normalized bytes: {}", edited.len());
        assert!(edited.len() <= MAX_SOURCE_BYTES);
        studio.apply_structure(Ok(edited.clone()));
        assert_eq!(studio.source, edited, "{}", studio.status);
        assert_starter(&studio);
        let tracks = &studio.script.as_ref().unwrap().scenes[0]
            .composition
            .joint_tracks;
        let key = tracks["NOUTAJA"]["head"]
            .iter()
            .find(|key| key.at == 1.0)
            .unwrap();
        assert!((key.rotation - 12.0_f64.to_radians()).abs() < 1e-12);
        let after = studio.snapshot();

        studio.undo_edit(false);
        assert_eq!(studio.snapshot(), before);
        studio.undo_edit(true);
        assert_eq!(studio.snapshot(), after);
        assert_starter(&studio);

        let root = std::env::temp_dir().join(format!(
            "kilsat-stick-edit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("edited.kilsat");
        studio.project_root = root.to_string_lossy().into_owned();
        studio.script_path = path.to_string_lossy().into_owned();
        studio.save_script();
        let saved = motion_project::load(&path).expect(&studio.status);
        saved.validate().unwrap();
        assert_eq!(saved.source, edited);
        assert_eq!(saved.art, after.document().unwrap().art);

        let mut reopened = Studio::default();
        reopened.script_path = path.to_string_lossy().into_owned();
        reopened.open_script();
        assert_eq!(reopened.source, edited, "{}", reopened.status);
        assert_eq!(reopened.snapshot(), after);
        assert_starter(&reopened);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn all_directors_accept_normalized_stick_project_and_preserve_authored_scenes() {
        use motion_script::auto_direction::{self, TimingSource, SAMPLE_RATE};
        const DIALOGUE: &str = "NOUTAJA: Onko vuoroni jo?\nVEIKKO: Kahvitauon jälkeen.";
        let document = stick_studio::load().unwrap();
        let original = motion_script::parse(&document.source).unwrap();
        assert_eq!(original.characters.len(), 6);
        assert_eq!(original.scenes.len(), 6);
        let normalized = motion_script::to_source(&original).unwrap();
        assert!(normalized.len() > 100_000);
        assert!(normalized.len() <= MAX_SOURCE_BYTES);
        assert_eq!(MAX_SOURCE_BYTES, auto_direction::MAX_SOURCE_BYTES);
        let paths = [
            "assets/director/noutaja.wav".into(),
            "assets/director/veikko.wav".into(),
        ];
        let samples = [2 * SAMPLE_RATE, 2 * SAMPLE_RATE];
        // These pure directors do not open the explicitly supplied WAV paths.
        let results = [
            (
                "direct",
                auto_direction::direct(&normalized, DIALOGUE, None),
            ),
            (
                "conversational",
                auto_direction::direct_conversational(&normalized, DIALOGUE, None),
            ),
            (
                "audio",
                auto_direction::direct_with_audio(&normalized, DIALOGUE, &samples, &paths),
            ),
        ];
        for (profile, result) in results {
            let result = result.unwrap_or_else(|error| panic!("{profile}: {error}"));
            assert!(result.source.len() > 100_000, "{profile}");
            assert!(result.source.len() <= MAX_SOURCE_BYTES, "{profile}");
            assert_eq!(result.first_scene, 6, "{profile}");
            assert!(result.shot_count > 0 && result.shot_count <= auto_direction::MAX_LINES);
            assert!(result.duration_samples <= auto_direction::MAX_DURATION_SAMPLES);
            let directed = motion_script::parse(&result.source).expect(profile);
            directed.validate().expect(profile);
            assert_eq!(directed.characters.len(), 6, "{profile}");
            assert_eq!(directed.scenes.len(), 6 + result.shot_count, "{profile}");
            assert_eq!(
                serde_yaml::to_value(&directed.characters).unwrap(),
                serde_yaml::to_value(&original.characters).unwrap(),
                "{profile} changed the cast"
            );
            assert_eq!(
                serde_yaml::to_value(&directed.scenes[..6]).unwrap(),
                serde_yaml::to_value(&original.scenes).unwrap(),
                "{profile} changed authored scenes"
            );
            let spoken: Vec<_> = directed.scenes[6..]
                .iter()
                .flat_map(|scene| &scene.events)
                .filter_map(|event| match event {
                    motion_script::Event::Speak {
                        actor, text, audio, ..
                    } => Some((actor.as_str(), text.as_str(), audio.as_deref())),
                    _ => None,
                })
                .collect();
            assert_eq!(spoken.len(), 2, "{profile}");
            assert_eq!((spoken[0].0, spoken[0].1), ("NOUTAJA", "Onko vuoroni jo?"));
            assert_eq!(
                (spoken[1].0, spoken[1].1),
                ("VEIKKO", "Kahvitauon jälkeen.")
            );
            if profile == "audio" {
                assert_eq!(result.timing_source, TimingSource::SuppliedSamples);
                assert_eq!(spoken[0].2, Some(paths[0].as_str()));
                assert_eq!(spoken[1].2, Some(paths[1].as_str()));
            } else {
                assert_eq!(result.timing_source, TimingSource::Estimated);
                assert!(spoken.iter().all(|line| line.2.is_none()));
            }
        }
    }

    #[test]
    fn all_directors_reject_one_byte_over_source_budget_before_parsing() {
        use motion_script::auto_direction;
        // Invalid YAML ensures the reported error comes from the byte guard.
        let oversized = "[".repeat(MAX_SOURCE_BYTES + 1);
        let paths = [
            "assets/director/noutaja.wav".into(),
            "assets/director/veikko.wav".into(),
        ];
        let dialogue = "NOUTAJA: Hei!\nVEIKKO: Terve!";
        let results = [
            ("direct", auto_direction::direct(&oversized, dialogue, None)),
            (
                "conversational",
                auto_direction::direct_conversational(&oversized, dialogue, None),
            ),
            (
                "audio",
                auto_direction::direct_with_audio(&oversized, dialogue, &[24_000, 24_000], &paths),
            ),
        ];
        for (profile, result) in results {
            let error = result.expect_err(profile);
            assert!(error.contains("256 KiB"), "{profile}: {error}");
        }
    }

    #[test]
    fn whole_source_guard_rejects_one_byte_over_budget_without_changing_project() {
        let mut studio = Studio::default();
        studio.validate();
        let before = studio.snapshot();
        let oversized = format!(
            "#{}\n{}",
            "x".repeat(MAX_SOURCE_BYTES + 1 - studio.source.len() - 2),
            studio.source
        );
        assert_eq!(oversized.len(), MAX_SOURCE_BYTES + 1);
        motion_script::parse(&oversized).unwrap();
        studio.apply_structure(Ok(oversized));
        assert_eq!(studio.snapshot(), before);
        assert!(studio.status.contains("256 KiB"));
        assert!(!studio.history.can_undo());
    }

    #[test]
    fn embedded_stick_project_cannot_replace_work_during_a_background_job() {
        for gate in 0..3 {
            let mut studio = Studio::default();
            studio.validate();
            studio.script_path = "existing.kilsat".into();
            let before = studio.snapshot();
            studio.busy = gate == 0;
            studio.diagnostics_busy = gate == 1;
            studio.export_busy = gate == 2;
            studio.load_stick_studio();
            assert_eq!(studio.snapshot(), before);
            assert_eq!(studio.script_path, "existing.kilsat");
            assert!(!studio.history.can_undo());
        }
    }
}

#[cfg(test)]
mod finnish_workspace_tests {
    use super::*;

    fn text(output: &egui::FullOutput) -> String {
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) => Some(text.galley.text()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn prepared_text_mouths_are_not_reported_as_rest_when_imported_alignment_is_absent() {
        let mut studio = Studio::default();
        let wait = |studio: &mut Studio| {
            let deadline = Instant::now() + Duration::from_secs(5);
            while studio.busy {
                studio.poll();
                assert!(Instant::now() < deadline, "{}", studio.status);
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        studio.load_sound_check();
        wait(&mut studio);
        studio.alignment.clear();
        studio.generate();
        wait(&mut studio);
        let preview = studio.preview.as_ref().expect("imported test WAV prepared");
        assert_eq!(preview.alignment_status, AlignmentStatus::None);
        assert_eq!(
            preview.clip.mouth_source(),
            motion_voice::timeline::MouthSource::TextEstimated
        );
        let original = studio.snapshot();
        let ctx = egui::Context::default();
        let draw = |studio: &mut Studio| {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 1200.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| studio.line_preview_ui(ui));
                },
            );
            text(&output)
        };
        let shown = draw(&mut studio);
        assert!(shown.contains("Tekstistä arvioitu"), "{shown}");
        assert!(!shown.contains("suu lepoasennossa"), "{shown}");
        studio.preview.as_mut().unwrap().clip.text_estimated = None;
        let shown = draw(&mut studio);
        assert!(shown.contains("Ei suun ajoitusta"), "{shown}");
        assert!(!shown.contains("Tekstistä arvioitu"), "{shown}");
        assert_eq!(studio.snapshot(), original);
        assert!(!studio.history.can_undo());
    }

    #[test]
    fn focused_inspector_tabs_and_workspace_leave_document_unchanged() {
        let ctx = egui::Context::default();
        studio_theme(&ctx);
        let mut studio = Studio::default();
        studio.validate();
        let before = studio.snapshot();
        // Level 1 hides the advanced tab and falls back to editing.
        studio.inspector_tab = InspectorTab::Advanced;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| studio.inspector_ui(ui));
        });
        assert!(studio.inspector_tab == InspectorTab::Edit);
        studio.ui_level = ui_levels::UiLevel::Deep;
        for (tab, label) in [
            (InspectorTab::Edit, "Valitse hahmo"),
            (InspectorTab::Speech, "Puhe ja äänet"),
            (InspectorTab::Export, "Valittu kohtaus"),
            (InspectorTab::Advanced, "Tekniset tarkistukset"),
        ] {
            studio.inspector_tab = tab;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(360.0, 1000.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| studio.inspector_ui(ui));
                },
            );
            assert!(
                text(&output).contains(label),
                "missing tab content: {label}"
            );
            assert_eq!(studio.snapshot(), before);
        }
        studio.inspector_tab = InspectorTab::Edit;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440.0, 960.0),
                )),
                ..Default::default()
            },
            |ctx| studio.workspace_ui(ctx),
        );
        let shown = text(&output);
        for label in ["Ohjeet", "Projekti", "Näyttämö", "AIKAJANA", "Muokkaa"] {
            assert!(shown.contains(label), "missing workspace control: {label}");
        }
        assert_eq!(studio.snapshot(), before);
        assert!(!studio.history.can_undo());
    }
}

#[cfg(test)]
mod measured_direction_integration_tests {
    use super::*;
    struct Folder(std::path::PathBuf);
    impl Folder {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "kilsat-direction-ui-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            let mut wav = hound::WavWriter::create(
                path.join("line.wav"),
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 24000,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for _ in 0..24000 {
                wav.write_sample(1000i16).unwrap();
            }
            wav.finalize().unwrap();
            Self(path)
        }
    }
    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn start(root: &Folder) -> Studio {
        let mut studio = Studio::default();
        studio.validate();
        studio.project_root = root.0.to_string_lossy().into_owned();
        studio.quick_start.dialogue = "EMMA: Hei!".into();
        studio.direction_imports.insert(1, "line.wav".into());
        studio.start_measured_direction();
        assert!(studio.busy);
        studio
    }
    fn finish(studio: &mut Studio) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while studio.direction_pending.is_some() {
            studio.poll_direction();
            assert!(Instant::now() < deadline, "{}", studio.status);
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn measured_pcm_direction_is_one_undo_and_keeps_provenance() {
        let root = Folder::new();
        let mut studio = start(&root);
        let original = studio.source.clone();
        finish(&mut studio);
        assert_ne!(studio.source, original);
        assert!(studio.direction_report.contains("24000 näytettä"));
        assert!(studio.direction_report.contains("SHA-256"));
        assert!(studio.history.can_undo());
        studio.undo_edit(false);
        assert_eq!(studio.source, original);
        assert!(!studio.history.can_undo());
    }
    #[test]
    fn stale_source_or_provider_context_cannot_commit_ready_audio() {
        for provider_change in [false, true] {
            let root = Folder::new();
            let mut studio = start(&root);
            if provider_change {
                studio.piper_config_path = "changed.json".into();
            } else {
                studio.source.push('\n');
            }
            let expected = studio.source.clone();
            finish(&mut studio);
            assert_eq!(studio.source, expected);
            assert!(!studio.history.can_undo());
            assert!(studio.status.contains("hylättiin"));
        }
    }
    #[test]
    fn cancel_never_commits_an_already_finished_pending_result() {
        let root = Folder::new();
        let mut studio = start(&root);
        let original = studio.source.clone();
        studio.active_cancel.as_ref().unwrap().cancel();
        finish(&mut studio);
        assert_eq!(studio.source, original);
        assert!(!studio.history.can_undo());
        assert!(studio.status.contains("peruttu"));
    }
}

#[cfg(test)]
mod piper_profile_context_tests {
    use super::*;

    fn finish_audio(studio: &mut Studio) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while studio.busy {
            studio.poll();
            assert!(Instant::now() < deadline, "{}", studio.status);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn profile_change_rejects_pending_scene_then_allows_fresh_preparation() {
        let mut studio = Studio::default();
        studio.load_sound_check();
        let source = studio.source.clone();
        let token = studio.active_cancel.clone().unwrap();
        let sequence_revision = studio.sequence_revision;
        studio.set_piper_profile("/new/profile.json".into());
        assert!(token.is_cancelled());
        assert!(
            studio.busy,
            "wait for the previous worker before replacing its token"
        );
        assert!(studio.sequence_revision > sequence_revision);
        finish_audio(&mut studio);
        assert!(studio.scene_preview.is_none());
        assert!(studio.preview.is_none());
        assert!(studio.script.is_some());
        assert_eq!(studio.source, source);
        assert!(!studio.history.can_undo());
        // Explicit imported demo WAVs still work; this never invokes Piper.
        studio.prepare_scene_audio();
        finish_audio(&mut studio);
        assert!(studio.scene_preview.is_some(), "{}", studio.status);
    }

    #[test]
    fn profile_change_rejects_an_inflight_line_and_invalidates_alignment_context() {
        let mut studio = Studio::default();
        studio.load_sound_check();
        finish_audio(&mut studio);
        studio.generate();
        let token = studio.active_cancel.clone().unwrap();
        studio.set_piper_profile("/another/profile.json".into());
        assert!(token.is_cancelled());
        finish_audio(&mut studio);
        assert!(studio.preview.is_none());
        assert!(studio.scene_preview.is_none());
        assert!(studio.alignment_identity.is_none());
        assert!(studio.alignment_audio.is_none());
        studio.generate();
        finish_audio(&mut studio);
        assert!(studio.preview.is_some(), "{}", studio.status);
    }

    #[test]
    fn unchanged_profile_is_idle_stable_but_a_changed_profile_drops_prepared_scene() {
        let mut studio = Studio::default();
        studio.load_sound_check();
        finish_audio(&mut studio);
        let revision = studio.sequence_revision;
        studio.set_piper_profile(String::new());
        assert_eq!(studio.sequence_revision, revision);
        assert!(studio.scene_preview.is_some());
        studio.set_piper_profile("/new/profile.json".into());
        assert!(studio.sequence_revision > revision);
        assert!(studio.scene_preview.is_none());
        assert!(!studio.history.can_undo());
    }

    #[test]
    fn kokoro_diagnostics_exclude_piper_and_system_voice_presets() {
        let mut studio = Studio::default();
        let mut script = motion_script::parse(&studio.source).unwrap();
        let mut english = script.characters.values().next().unwrap().clone();
        english.voice = Some(motion_script::VoiceConfig {
            provider: "kokoro".into(),
            preset: "af_heart".into(),
            language: "en-us".into(),
            speed: 1.0,
        });
        let mut finnish = english.clone();
        finnish.voice = Some(motion_script::VoiceConfig {
            provider: "piper".into(),
            preset: "piper_harri".into(),
            language: "fi-fi".into(),
            speed: 1.0,
        });
        let mut system = finnish.clone();
        system.voice.as_mut().unwrap().provider = "system".into();
        system.voice.as_mut().unwrap().preset = "Satu".into();
        script.characters = [
            ("EMMA".into(), english),
            ("ALEX".into(), finnish),
            ("SYSTEM".into(), system),
        ]
        .into_iter()
        .collect();
        studio.source = serde_yaml::to_string(&script).unwrap();
        assert_eq!(
            studio
                .kokoro_diagnostic_presets()
                .into_iter()
                .collect::<Vec<_>>(),
            vec!["af_heart"]
        );
    }
}

#[cfg(test)]
mod hand_prop_workflow_tests {
    use super::*;
    fn studio() -> Studio {
        let mut studio = Studio::default();
        studio.validate();
        studio.load_stick_studio();
        studio.history.reset(&studio.source);
        studio.selected_actor = Some("VEIKKO".into());
        studio.scene_index = 2;
        studio.playhead = 1.25;
        studio
    }
    #[test]
    fn actual_hand_prop_transaction_preserves_art_selection_time_and_has_one_undo_step() {
        let mut studio = studio();
        let before = studio.snapshot();
        let art = studio.project_document.as_ref().unwrap().art.clone();
        studio.hand_props.hand = hand_props::Hand::Left;
        studio.hand_props.item_index = 0;
        studio.apply_hand_prop(false);
        assert_ne!(studio.source, before.source, "{}", studio.status);
        assert_eq!(studio.selected_actor.as_deref(), Some("VEIKKO"));
        assert_eq!(studio.scene_index, 2);
        assert_eq!(studio.playhead, 1.25);
        assert_eq!(studio.project_document.as_ref().unwrap().art, art);
        let after = studio.snapshot();
        let rig = studio.script.as_ref().unwrap().characters["VEIKKO"]
            .rig
            .as_ref()
            .unwrap();
        assert!(rig.parts.iter().any(|p| p.id == "heldprop_l_puhelin_case"));
        studio.undo_edit(false);
        assert_eq!(studio.snapshot(), before);
        assert!(!studio.history.can_undo());
        studio.undo_edit(true);
        assert_eq!(studio.snapshot(), after);
        studio.selected_actor = Some("VEIKKO".into());
        studio.playhead = 1.25;
        studio.apply_hand_prop(true);
        assert!(!studio.script.as_ref().unwrap().characters["VEIKKO"]
            .rig
            .as_ref()
            .unwrap()
            .parts
            .iter()
            .any(|p| p.id.starts_with("heldprop_l_")));
        studio.undo_edit(false);
        assert_eq!(studio.snapshot(), after);
    }
    #[test]
    fn busy_guards_and_invalid_prop_changes_leave_source_art_and_history_untouched() {
        let mut studio = studio();
        let before = studio.snapshot();
        for guard in 0..4 {
            studio.busy = guard == 0;
            studio.export_busy = guard == 1;
            studio.diagnostics_busy = guard == 2;
            studio.setup_busy = guard == 3;
            studio.apply_hand_prop(false);
            studio.apply_camera_shot(camera_framing::Shot::CloseUp);
            assert_eq!(studio.snapshot(), before);
            assert!(!studio.history.can_undo());
        }
        studio.setup_busy = false;
        studio.hand_props.placement.scale = f64::NAN;
        studio.apply_hand_prop(false);
        assert_eq!(studio.snapshot(), before);
        assert!(!studio.history.can_undo());
        studio.apply_artist_source(Ok("x".repeat(MAX_SOURCE_BYTES + 1)), "must not succeed");
        assert_eq!(studio.snapshot(), before);
        assert!(!studio.history.can_undo());
    }
    #[test]
    fn camera_button_creates_one_native_key_and_preserves_artist_context() {
        let mut studio = studio();
        let before = studio.snapshot();
        studio.apply_camera_shot(camera_framing::Shot::CloseUp);
        assert_ne!(studio.source, before.source, "{}", studio.status);
        assert_eq!(studio.selected_actor.as_deref(), Some("VEIKKO"));
        assert_eq!(studio.playhead, 1.25);
        let camera = &studio.script.as_ref().unwrap().scenes[2].composition.camera;
        assert!(camera.iter().any(|k| k.at == 1.25));
        studio.undo_edit(false);
        assert_eq!(studio.snapshot(), before);
        assert!(!studio.history.can_undo());
    }
}

#[cfg(test)]
mod a09_transport_tests {
    use super::*;

    /// Play, pause and seek all land on the 24 kHz grid and show the same frame
    /// as evaluating that time directly with `motion_scene::evaluate` (A04).
    #[test]
    fn play_pause_and_seek_show_the_same_evaluated_frame() {
        let mut studio = Studio::default();
        studio.validate();
        let script = studio.script.clone().expect("default screenplay is valid");
        let duration = script.scenes[studio.scene_index].duration;
        assert!(duration > 1.0);

        // Play for 0.75 s of wall time without scene audio.
        studio.scene_transport.as_mut().unwrap().play();
        studio.scene_tick = Instant::now() - Duration::from_millis(750);
        studio.poll();
        let played = studio.playhead;
        assert!((0.75..0.85).contains(&played), "{played}");
        assert_eq!(
            (played * 24000.0).fract(),
            0.0,
            "playhead off the sample grid"
        );

        // Pause: time does not move however long we wait.
        studio.scene_transport.as_mut().unwrap().pause();
        studio.scene_tick = Instant::now() - Duration::from_secs(2);
        studio.poll();
        assert_eq!(studio.playhead, played);

        let from_play =
            motion_scene::evaluate(&script, studio.scene_index, studio.playhead).unwrap();

        // Seek away and back: identical frame to the one reached by playing.
        studio.seek_scene(duration);
        assert_eq!(studio.playhead, quantized_seek(duration, duration));
        studio.seek_scene(played);
        assert_eq!(studio.playhead, played);
        let from_seek =
            motion_scene::evaluate(&script, studio.scene_index, studio.playhead).unwrap();
        assert_eq!(from_seek, from_play);
        assert_eq!(
            studio.scene_transport.as_ref().unwrap().position_seconds(),
            played
        );
    }
}

#[cfg(test)]
mod screen_cue_history_tests {
    use super::*;

    #[test]
    fn screen_cue_add_edit_remove_are_single_undo_steps_with_inline_errors() {
        let (source, slot) = screen_cues::tests::phone_source();
        let mut studio = Studio::default();
        studio.source = source.clone();
        studio.validate();
        studio.history.reset(&studio.source);
        studio.scene_index = 0;

        let mut draft = screen_cues::Draft::new("VEIKKO", &slot, 0.0);
        draft.kind = screen_cues::Kind::Off;
        studio.screen_cue_action(screen_cues::Action::Apply(Box::new(draft.clone())));
        assert!(
            studio.screen_cues.error.is_none(),
            "{:?}",
            studio.screen_cues.error
        );
        let added = studio.source.clone();
        assert_ne!(added, source);
        assert_eq!(
            studio.script.as_ref().unwrap().scenes[0]
                .composition
                .screens
                .len(),
            1
        );

        // An invalid edit stays inline and leaves the screenplay untouched.
        let mut bad = draft.clone();
        bad.editing = Some(0);
        bad.kind = screen_cues::Kind::Video;
        bad.video = "../outside.mov".into();
        studio.screen_cue_action(screen_cues::Action::Apply(Box::new(bad)));
        assert!(studio
            .screen_cues
            .error
            .as_deref()
            .unwrap()
            .contains("project-relative"));
        assert_eq!(studio.source, added);

        let mut edit = draft.clone();
        edit.editing = Some(0);
        edit.at = 1.0;
        studio.screen_cue_action(screen_cues::Action::Apply(Box::new(edit)));
        assert!(studio.screen_cues.error.is_none());
        let edited = studio.source.clone();
        assert_eq!(
            studio.script.as_ref().unwrap().scenes[0]
                .composition
                .screens[0]
                .at,
            1.0
        );

        studio.screen_cue_action(screen_cues::Action::Remove(0));
        assert!(studio.script.as_ref().unwrap().scenes[0]
            .composition
            .screens
            .is_empty());

        studio.undo_edit(false);
        assert_eq!(studio.source, edited);
        studio.undo_edit(false);
        assert_eq!(studio.source, added);
        studio.undo_edit(false);
        assert_eq!(studio.source, source);
        assert!(!studio.history.can_undo());
        studio.undo_edit(true);
        assert_eq!(studio.source, added);
    }

    #[test]
    fn video_import_without_ffmpeg_reports_inline_and_starts_nothing() {
        let (source, _) = screen_cues::tests::phone_source();
        let mut studio = Studio::default();
        studio.source = source.clone();
        studio.validate();
        studio.ffmpeg_path.clear();
        studio.screen_cue_action(screen_cues::Action::ImportVideo("/tmp/demo.mov".into()));
        assert!(studio
            .screen_cues
            .error
            .as_deref()
            .unwrap()
            .contains("FFmpeg puuttuu"));
        assert!(studio.screen_videos.import.is_none());
        assert_eq!(studio.source, source);
    }
}

#[cfg(test)]
mod block_palette_tests {
    use super::*;

    /// Lays the palette out in a narrow panel (no window) and returns the panel
    /// rect, the chip rects with their full labels, and one text row's height.
    fn layout(width: f32, extra: &[&str]) -> (egui::Rect, Vec<(String, egui::Rect)>, f32) {
        let ctx = egui::Context::default();
        let mut chips = Vec::new();
        let mut panel = egui::Rect::NOTHING;
        let mut row = 0.0;
        // Two passes: egui measures fonts on the first frame.
        for _ in 0..2 {
            chips.clear();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::SidePanel::left("palette")
                    .exact_width(width)
                    .show(ctx, |ui| {
                        panel = ui.max_rect();
                        row = ui.text_style_height(&egui::TextStyle::Button);
                        let mut groups: Vec<Vec<motion_blocks::Block>> = motion_blocks::builtin()
                            .into_iter()
                            .map(|(_, blocks)| blocks)
                            .collect();
                        groups.push(
                            extra
                                .iter()
                                .map(|n| motion_blocks::Block::Clip((*n).to_owned()))
                                .collect(),
                        );
                        for blocks in groups {
                            ui.horizontal_wrapped(|ui| {
                                for block in blocks {
                                    let label = block.label();
                                    let response = block_chip(ui, block, true);
                                    chips.push((label, response.rect));
                                }
                            });
                        }
                    });
            });
        }
        (panel, chips, row)
    }

    #[test]
    fn every_palette_chip_is_one_line_and_inside_a_narrow_panel() {
        let long = "hyvin pitkä tallennettu liike nimeltään";
        for width in [140.0, 170.0, 320.0] {
            let (panel, chips, row) = layout(width, &["askel", "vilkutus", long]);
            assert!(chips.len() > 10);
            for (label, rect) in &chips {
                assert!(
                    rect.left() >= panel.left() - 0.5 && rect.right() <= panel.right() + 0.5,
                    "{label} at width {width}: {rect:?} outside {panel:?}"
                );
                assert!(
                    rect.height() < row * 1.9,
                    "{label} at width {width} wraps onto several lines: {rect:?}"
                );
            }
        }
    }

    #[test]
    fn long_chip_names_are_shortened_with_an_ellipsis() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert_eq!(chip_label(ui, "askel", 200.0), "askel");
                let short = chip_label(ui, "hyvin pitkä tallennettu liike", 60.0);
                assert!(
                    short.ends_with('…') && short.chars().count() < 29,
                    "{short}"
                );
            });
        });
    }
}
