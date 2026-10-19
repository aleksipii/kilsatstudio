//! Detached controls for the project-local authored bone clip library.
use eframe::egui;
use motion_script::Script;

#[derive(Debug, PartialEq)]
pub enum Action {
    Capture {
        name: String,
        replace: bool,
    },
    Apply {
        name: String,
    },
    ApplyRetimed {
        name: String,
        at: f64,
        duration: f64,
    },
    Remove {
        name: String,
    },
    Import {
        text: String,
        replace: bool,
    },
}

impl Action {
    pub fn apply(
        self,
        source: &str,
        scene_index: usize,
        actor: &str,
    ) -> Result<String, motion_script::Error> {
        match self {
            Self::Capture { name, replace } => {
                motion_script::capture_animation_clip(source, scene_index, actor, &name, replace)
            }
            Self::Apply { name } => {
                motion_script::apply_animation_clip(source, scene_index, actor, &name)
            }
            Self::ApplyRetimed { name, at, duration } => {
                motion_script::apply_animation_clip_retimed(
                    source,
                    scene_index,
                    actor,
                    &name,
                    at,
                    duration,
                )
            }
            Self::Remove { name } => motion_script::remove_animation_clip(source, &name),
            Self::Import { text, replace } => import_library(source, &text, replace),
        }
    }
}

#[derive(Default)]
pub struct State {
    name: String,
    selected: String,
    replace_capture: bool,
    replace_target: bool,
    confirm_delete: bool,
    context: Option<(usize, String)>,
    retime: bool,
    at: f64,
    duration: f64,
    exchange: String,
    replace_import: bool,
    exchange_status: String,
}

impl State {
    /// Paints drafts only. The caller applies returned actions through source history.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        script: &Script,
        scene_index: usize,
        actor: &str,
    ) -> Option<Action> {
        let context = (scene_index, actor.to_owned());
        if self.context.as_ref() != Some(&context) {
            self.replace_capture = false;
            self.replace_import = false;
            self.replace_target = false;
            self.confirm_delete = false;
            self.retime = false;
            self.at = 0.0;
            self.duration = script
                .animation_clips
                .get(&self.selected)
                .map_or(0.0, |clip| clip.duration);
            self.context = Some(context);
        }
        ui.small("Project-local clips contain authored bone tracks. Root movement, procedural strengths, speech and facial animation are not copied.");
        ui.small("Applying replaces ALL target actor bone tracks. Key times stay unchanged unless retiming is enabled. Apply supports Undo. YAML formatting is normalized and comments are removed.");
        let scene = script.scenes.get(scene_index);
        let valid_target = scene.is_some() && script.characters.contains_key(actor);
        let has_tracks = scene
            .and_then(|scene| scene.composition.bone_tracks.get(actor))
            .is_some_and(|tracks| tracks.values().any(|keys| !keys.is_empty()));
        ui.label(format!("Capture from {actor}"));
        if ui.text_edit_singleline(&mut self.name).changed() {
            self.replace_capture = false;
        }
        let valid_name = !self.name.is_empty()
            && self.name.len() <= 64
            && self
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
        ui.small("Name: 1–64 ASCII letters, digits, underscores or hyphens.");
        let exists = script.animation_clips.contains_key(&self.name);
        if exists {
            let (scenes, instances) = reference_counts(script, &self.name);
            if instances > 0 {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    format!("Replacing this clip changes {instances} live instances in {scenes} scenes. Replacement must fit every reference."),
                );
            }
            ui.checkbox(
                &mut self.replace_capture,
                "Replace this existing library clip",
            );
        }
        let can_capture = valid_target
            && has_tracks
            && valid_name
            && (!exists || self.replace_capture)
            && (exists || script.animation_clips.len() < 64)
            && scene.is_some_and(|scene| scene.duration <= 600.0);
        let mut action = None;
        if ui
            .add_enabled(can_capture, egui::Button::new("Capture bone tracks"))
            .clicked()
        {
            action = Some(Action::Capture {
                name: self.name.clone(),
                replace: exists && self.replace_capture,
            });
            self.replace_capture = false;
        }
        if !has_tracks {
            ui.small("Add authored bone keys to this actor before capturing a clip.");
        }
        ui.separator();
        let previous = self.selected.clone();
        egui::ComboBox::from_id_salt("authored_bone_clip")
            .selected_text(if self.selected.is_empty() {
                "Select a clip"
            } else {
                &self.selected
            })
            .show_ui(ui, |ui| {
                for name in script.animation_clips.keys() {
                    ui.selectable_value(&mut self.selected, name.clone(), name);
                }
            });
        if previous != self.selected {
            self.retime = false;
            self.at = 0.0;
            self.duration = script
                .animation_clips
                .get(&self.selected)
                .map_or(0.0, |clip| clip.duration);
            self.replace_target = false;
            self.confirm_delete = false;
        }
        if let Some(clip) = script.animation_clips.get(&self.selected) {
            let key_count: usize = clip.bone_tracks.values().map(Vec::len).sum();
            ui.label(format!(
                "{:.2} seconds · {} bones · {key_count} keys",
                clip.duration,
                clip.bone_tracks.len()
            ));
            if ui
                .checkbox(&mut self.retime, "Scale and offset copied key times")
                .changed()
            {
                self.replace_target = false;
            }
            if self.retime {
                ui.small("Copied tracks are independent. Bone tracks hold their endpoint poses before and after their keys; this does not create a live clip window or mix clips.");
                ui.horizontal(|ui| {
                    ui.label("Start (seconds)");
                    if time_field(ui, &mut self.at) {
                        self.replace_target = false;
                    }
                    ui.label("Duration (seconds)");
                    if time_field(ui, &mut self.duration) {
                        self.replace_target = false;
                    }
                });
                if ui.button("Reset copied key timing").clicked() {
                    self.at = 0.0;
                    self.duration = clip.duration;
                    self.replace_target = false;
                }
            }
            let fits = scene.is_some_and(|scene| {
                if self.retime {
                    self.at.is_finite()
                        && self.at >= 0.0
                        && self.duration.is_finite()
                        && self.duration > 0.0
                        && (self.at + self.duration).is_finite()
                        && self.at + self.duration > self.at
                        && self.at + self.duration <= scene.duration
                } else {
                    clip.duration <= scene.duration
                }
            });
            if !fits {
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    if self.retime {
                        "Start must be finite and nonnegative; duration must be positive and the end must fit this scene."
                    } else {
                        "The selected clip is longer than this scene; enable retiming to compress it or choose a longer target scene."
                    },
                );
            }
            ui.checkbox(
                &mut self.replace_target,
                format!("Replace all bone tracks on {actor}"),
            );
            if ui
                .add_enabled(
                    valid_target && fits && self.replace_target,
                    egui::Button::new("Apply selected bone clip"),
                )
                .clicked()
            {
                action = Some(if self.retime {
                    Action::ApplyRetimed {
                        name: self.selected.clone(),
                        at: self.at,
                        duration: self.duration,
                    }
                } else {
                    Action::Apply {
                        name: self.selected.clone(),
                    }
                });
                self.replace_target = false;
            }
            let (referenced_scenes, references) = reference_counts(script, &self.selected);
            if references > 0 {
                self.confirm_delete = false;
                ui.small(format!("Used by {references} live instances in {referenced_scenes} scenes. Remove those instances before deleting this library clip."));
            }
            ui.add_enabled_ui(references == 0, |ui| {
                ui.checkbox(
                    &mut self.confirm_delete,
                    "Delete selected clip from this project",
                );
            });
            if ui
                .add_enabled(
                    self.confirm_delete && references == 0,
                    egui::Button::new("Delete library clip"),
                )
                .clicked()
            {
                action = Some(Action::Remove {
                    name: self.selected.clone(),
                });
                self.confirm_delete = false;
            }
        } else {
            self.replace_target = false;
            self.confirm_delete = false;
            ui.small("Select a captured clip to inspect or apply it.");
        }
        ui.separator();
        ui.collapsing("Share clips between projects", |ui| {
            ui.small("Version 1 · original seven-bone rig only. Copy the JSON text into another project's clip library. Custom rigs, textures, speech and assets are not included.");
            ui.horizontal(|ui| {
                if ui.add_enabled(!self.selected.is_empty(), egui::Button::new("Copy selected clip")).clicked() {
                    match export_library(script, Some(&self.selected)) {
                        Ok(text) => { ui.ctx().copy_text(text); self.exchange_status = "Clip copied. Paste it into the destination project below.".into(); }
                        Err(error) => self.exchange_status = error.to_string(),
                    }
                }
                if ui.add_enabled(!script.animation_clips.is_empty(), egui::Button::new("Copy all clips")).clicked() {
                    match export_library(script, None) {
                        Ok(text) => { ui.ctx().copy_text(text); self.exchange_status = "Library copied.".into(); }
                        Err(error) => self.exchange_status = error.to_string(),
                    }
                }
            });
            if ui.add(egui::TextEdit::multiline(&mut self.exchange).char_limit(EXCHANGE_LIMIT).desired_rows(5).hint_text("Paste exported clip library JSON")).changed() {
                self.replace_import = false;
                self.exchange_status.clear();
            }
            ui.checkbox(&mut self.replace_import, "Replace matching clip names (also changes their live instances)");
            ui.small("Import is one Undo step. Every affected live reference must remain valid. YAML comments and formatting are normalized.");
            if ui.add_enabled(!self.exchange.trim().is_empty(), egui::Button::new("Import clips into this project")).clicked() {
                action = Some(Action::Import { text: self.exchange.clone(), replace: self.replace_import });
                self.replace_import = false;
            }
            if !self.exchange_status.is_empty() { ui.label(&self.exchange_status); }
        });
        action
    }
}

const EXCHANGE_LIMIT: usize = 1024 * 1024;

fn exchange_error(message: impl Into<String>) -> motion_script::Error {
    motion_script::Error::Validation(message.into())
}

/// A portable, asset-free library; importing always validates the whole destination.
fn export_library(script: &Script, selected: Option<&str>) -> Result<String, motion_script::Error> {
    script.validate()?;
    let clips = if let Some(name) = selected {
        let clip = script
            .animation_clips
            .get(name)
            .ok_or_else(|| exchange_error("Select an existing clip to export"))?;
        serde_json::to_value(std::collections::BTreeMap::from([(name, clip)]))
            .map_err(|error| exchange_error(error.to_string()))?
    } else {
        serde_json::to_value(&script.animation_clips)
            .map_err(|error| exchange_error(error.to_string()))?
    };
    let text = serde_json::to_string_pretty(&serde_json::json!({
        "format": "kilsat-clip-library", "version": 1, "rig": "original-seven-bone", "clips": clips
    }))
    .map_err(|error| exchange_error(error.to_string()))?;
    if text.len() > EXCHANGE_LIMIT {
        return Err(exchange_error("Clip exchange exceeds the one MiB limit"));
    }
    Ok(text)
}

fn import_library(source: &str, text: &str, replace: bool) -> Result<String, motion_script::Error> {
    if source.len() > EXCHANGE_LIMIT || text.len() > EXCHANGE_LIMIT {
        return Err(exchange_error(
            "Clip exchange and screenplay are limited to one MiB each",
        ));
    }
    let envelope: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| exchange_error(format!("Clip library JSON: {error}")))?;
    let object = envelope
        .as_object()
        .ok_or_else(|| exchange_error("Clip library must be a JSON object"))?;
    if object.len() != 4
        || envelope["format"] != "kilsat-clip-library"
        || envelope["version"].as_u64() != Some(1)
        || envelope["rig"] != "original-seven-bone"
    {
        return Err(exchange_error("Expected kilsat-clip-library version 1 for the original-seven-bone rig with only format, version, rig and clips fields"));
    }
    let clips: std::collections::BTreeMap<String, motion_script::AnimationClip> =
        serde_json::from_value(envelope["clips"].clone())
            .map_err(|error| exchange_error(format!("Clip library: {error}")))?;
    if clips.is_empty() || clips.len() > 64 {
        return Err(exchange_error("Import requires 1..64 clips"));
    }
    let mut candidate = motion_script::parse(source)?;
    for (name, clip) in clips {
        if !replace && candidate.animation_clips.contains_key(&name) {
            return Err(exchange_error(format!(
                "Clip {name} already exists; explicitly enable replacement"
            )));
        }
        candidate.animation_clips.insert(name, clip);
    }
    candidate.validate()?;
    let output = serde_yaml::to_string(&candidate)?;
    if output.len() > EXCHANGE_LIMIT {
        return Err(exchange_error(
            "Imported screenplay exceeds the one MiB limit",
        ));
    }
    Ok(output)
}

fn reference_counts(script: &Script, name: &str) -> (usize, usize) {
    let counts: Vec<usize> = script
        .scenes
        .iter()
        .map(|scene| {
            scene
                .composition
                .clip_instances
                .iter()
                .filter(|instance| instance.clip == name)
                .count()
        })
        .collect();
    (
        counts.iter().filter(|count| **count > 0).count(),
        counts.iter().sum(),
    )
}

/// Nonfinite drafts remain visible and invalid rather than silently becoming valid.
fn time_field(ui: &mut egui::Ui, value: &mut f64) -> bool {
    if !value.is_finite() {
        ui.colored_label(egui::Color32::LIGHT_RED, value.to_string());
        return false;
    }
    let mut edited = *value;
    if ui
        .add(egui::DragValue::new(&mut edited).speed(0.05))
        .changed()
    {
        *value = edited;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "characters: {A: {asset: a}, B: {asset: b}}\nscenes: [{id: shot, duration: 4, events: [], composition: {bone_tracks: {A: {head: [{at: 0, x: 2}, {at: 2, x: 8}]}}}}]\n";

    #[test]
    fn portable_library_import_is_atomic_validated_and_cross_project() {
        let captured = Action::Capture {
            name: "Nod".into(),
            replace: false,
        }
        .apply(SOURCE, 0, "A")
        .unwrap();
        let script = motion_script::parse(&captured).unwrap();
        let text = export_library(&script, Some("Nod")).unwrap();
        let target = "characters: {Other: {asset: other}}\nscenes: [{id: elsewhere, duration: 8, events: []}]\n";
        let imported = Action::Import {
            text: text.clone(),
            replace: false,
        }
        .apply(target, 0, "Other")
        .unwrap();
        let parsed = motion_script::parse(&imported).unwrap();
        assert_eq!(parsed.animation_clips["Nod"], script.animation_clips["Nod"]);
        assert!(parsed.scenes[0].composition.bone_tracks.is_empty());
        assert!(import_library(&imported, &text, false).is_err());
        assert!(import_library(&imported, &text, true).is_ok());
        assert_eq!(
            motion_script::parse(target).unwrap().animation_clips.len(),
            0
        );
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        value["version"] = serde_json::json!(2);
        assert!(import_library(target, &value.to_string(), false).is_err());
        value["version"] = serde_json::json!(1);
        value["unexpected"] = serde_json::json!(true);
        assert!(import_library(target, &value.to_string(), false).is_err());
        value.as_object_mut().unwrap().remove("unexpected");
        value["clips"]["Nod"]["bone_tracks"]["head"][0]["at"] = serde_json::json!(-1);
        assert!(import_library(target, &value.to_string(), false).is_err());
        assert!(import_library(target, &" ".repeat(EXCHANGE_LIMIT + 1), false).is_err());
    }

    #[test]
    fn invalid_import_preserves_existing_live_instance_definitions() {
        let captured = Action::Capture {
            name: "Nod".into(),
            replace: false,
        }
        .apply(SOURCE, 0, "A")
        .unwrap();
        let mut script = motion_script::parse(&captured).unwrap();
        script.scenes[0]
            .composition
            .clip_instances
            .push(motion_script::ClipInstance {
                id: "live".into(),
                clip: "Nod".into(),
                actor: "B".into(),
                at: 0.0,
                duration: 4.0,
                weight: 1.0,
                fade_in: 0.0,
                fade_out: 0.0,
                weight_keys: Vec::new(),
                looping: false,
                joint_mask: Vec::new(),
                blend_mode: motion_script::ClipBlendMode::Override,
                normalized_group: None,
            });
        script.validate().unwrap();
        let source = serde_yaml::to_string(&script).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_str(&export_library(&script, None).unwrap()).unwrap();
        value["clips"]["Nod"]["bone_tracks"] = serde_json::json!({"not_a_bone": [{"at": 0}]});
        assert!(import_library(&source, &value.to_string(), true).is_err());
        assert_eq!(
            motion_script::parse(&source).unwrap().animation_clips["Nod"],
            script.animation_clips["Nod"]
        );
    }

    #[test]
    fn live_references_are_counted_and_cannot_be_deleted_from_library() {
        let captured = Action::Capture {
            name: "Nod".into(),
            replace: false,
        }
        .apply(SOURCE, 0, "A")
        .unwrap();
        let source = motion_script::upsert_clip_instance(
            &captured,
            0,
            motion_script::ClipInstance {
                id: "live".into(),
                clip: "Nod".into(),
                actor: "B".into(),
                at: 0.0,
                duration: 4.0,
                weight: 1.0,
                fade_in: 0.0,
                fade_out: 0.0,
                weight_keys: Vec::new(),
                looping: false,
                joint_mask: Vec::new(),
                blend_mode: motion_script::ClipBlendMode::Override,
                normalized_group: None,
            },
        )
        .unwrap();
        let script = motion_script::parse(&source).unwrap();
        assert_eq!(reference_counts(&script, "Nod"), (1, 1));
        assert_eq!(reference_counts(&script, "Missing"), (0, 0));
        assert!(Action::Remove { name: "Nod".into() }
            .apply(&source, 0, "A")
            .is_err());
        let mut state = State {
            selected: "Nod".into(),
            confirm_delete: true,
            context: Some((0, "A".into())),
            ..State::default()
        };
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(state.ui(ui, &script, 0, "A").is_none());
            });
        });
        assert!(!state.confirm_delete);
    }

    #[test]
    fn painting_changes_no_source_and_context_resets_destructive_confirmations() {
        let source = Action::Capture {
            name: "Nod".into(),
            replace: false,
        }
        .apply(SOURCE, 0, "A")
        .unwrap();
        let script = motion_script::parse(&source).unwrap();
        let before = serde_json::to_value(&script).unwrap();
        let mut state = State {
            name: "Draft name".into(),
            selected: "Nod".into(),
            replace_capture: true,
            replace_target: true,
            confirm_delete: true,
            context: Some((0, "A".into())),
            retime: true,
            at: 1.0,
            duration: 2.0,
            ..State::default()
        };
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(state.ui(ui, &script, 0, "B").is_none());
            });
        });
        assert_eq!(state.name, "Draft name");
        assert!(!state.retime);
        assert_eq!(state.at, 0.0);
        assert_eq!(state.duration, 4.0);
        assert!(!state.replace_capture && !state.replace_target && !state.confirm_delete);
        assert_eq!(serde_json::to_value(&script).unwrap(), before);
    }

    #[test]
    fn explicit_actions_capture_apply_and_remove_through_validated_source_api() {
        let captured = Action::Capture {
            name: "Nod".into(),
            replace: false,
        }
        .apply(SOURCE, 0, "A")
        .unwrap();
        let applied = Action::Apply { name: "Nod".into() }
            .apply(&captured, 0, "B")
            .unwrap();
        let script = motion_script::parse(&applied).unwrap();
        assert_eq!(
            script.scenes[0].composition.bone_tracks["A"],
            script.scenes[0].composition.bone_tracks["B"]
        );
        let removed = Action::Remove { name: "Nod".into() }
            .apply(&applied, 0, "B")
            .unwrap();
        let removed = motion_script::parse(&removed).unwrap();
        assert!(removed.animation_clips.is_empty());
        assert_eq!(removed.scenes[0].composition, script.scenes[0].composition);
        assert!(Action::Capture {
            name: "Nod".into(),
            replace: false,
        }
        .apply(&captured, 0, "A")
        .is_err());
    }

    #[test]
    fn retimed_action_maps_key_times_and_rejects_invalid_drafts() {
        let captured = Action::Capture {
            name: "Nod".into(),
            replace: false,
        }
        .apply(SOURCE, 0, "A")
        .unwrap();
        let applied = Action::ApplyRetimed {
            name: "Nod".into(),
            at: 1.0,
            duration: 2.0,
        }
        .apply(&captured, 0, "B")
        .unwrap();
        let script = motion_script::parse(&applied).unwrap();
        let keys = &script.scenes[0].composition.bone_tracks["B"]["head"];
        assert_eq!(keys[0].at, 1.0);
        assert_eq!(keys[1].at, 2.0);
        for (at, duration) in [
            (f64::NAN, 2.0),
            (0.0, f64::INFINITY),
            (-1.0, 2.0),
            (0.0, 0.0),
            (3.0, 2.0),
        ] {
            assert!(Action::ApplyRetimed {
                name: "Nod".into(),
                at,
                duration,
            }
            .apply(&captured, 0, "B")
            .is_err());
        }
    }

    #[test]
    fn painting_preserves_nonfinite_retiming_drafts_without_applying() {
        let captured = Action::Capture {
            name: "Nod".into(),
            replace: false,
        }
        .apply(SOURCE, 0, "A")
        .unwrap();
        let script = motion_script::parse(&captured).unwrap();
        let mut state = State {
            selected: "Nod".into(),
            retime: true,
            at: f64::NAN,
            duration: f64::INFINITY,
            replace_target: true,
            context: Some((0, "B".into())),
            ..State::default()
        };
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(state.ui(ui, &script, 0, "B").is_none());
            });
        });
        assert!(state.at.is_nan());
        assert_eq!(state.duration, f64::INFINITY);
        assert!(state.retime);
        assert_eq!(
            motion_script::parse(&captured).unwrap().scenes[0].composition,
            script.scenes[0].composition
        );
    }
}
