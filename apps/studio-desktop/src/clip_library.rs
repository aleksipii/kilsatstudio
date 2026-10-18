//! Detached controls for the project-local authored bone clip library.
use eframe::egui;
use motion_script::Script;

#[derive(Debug, PartialEq)]
pub enum Action {
    Capture { name: String, replace: bool },
    Apply { name: String },
    ApplyRetimed {
        name: String,
        at: f64,
        duration: f64,
    },
    Remove { name: String },
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
            Self::ApplyRetimed { name, at, duration } => motion_script::apply_animation_clip_retimed(
                source, scene_index, actor, &name, at, duration,
            ),
            Self::Remove { name } => motion_script::remove_animation_clip(source, &name),
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
            ui.checkbox(
                &mut self.confirm_delete,
                "Delete selected clip from this project",
            );
            if ui
                .add_enabled(
                    self.confirm_delete,
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
        action
    }
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
