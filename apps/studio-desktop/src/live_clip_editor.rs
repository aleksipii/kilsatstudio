//! Detached live clip instance controls; all mutations use validated source actions.
use eframe::egui;
use motion_script::{ClipInstance, Script};

#[derive(Debug, PartialEq)]
pub enum Action {
    Upsert { instance: ClipInstance },
    Remove { id: String },
    Reorder { id: String, new_index: usize },
}

impl Action {
    pub fn apply(self, source: &str, scene_index: usize) -> Result<String, motion_script::Error> {
        match self {
            Self::Upsert { instance } => {
                motion_script::upsert_clip_instance(source, scene_index, instance)
            }
            Self::Remove { id } => motion_script::remove_clip_instance(source, scene_index, &id),
            Self::Reorder { id, new_index } => {
                motion_script::reorder_clip_instance(source, scene_index, &id, new_index)
            }
        }
    }
}

#[derive(Default)]
pub struct State {
    draft: Option<ClipInstance>,
    editing: bool,
    confirm_delete: bool,
    context: Option<(usize, String)>,
}

impl State {
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        script: &Script,
        scene_index: usize,
        actor: &str,
    ) -> Option<Action> {
        let context = (scene_index, actor.to_owned());
        if self.context.as_ref() != Some(&context) {
            self.draft = None;
            self.editing = false;
            self.confirm_delete = false;
            self.context = Some(context);
        }
        let scene = script.scenes.get(scene_index)?;
        ui.small("Live library references use constant, ordered override weights, not normalized or additive mixing. Later rows override earlier rows on shared bones. Hard window edges can pop; windows include their start and exclude their end.");
        ui.small("Editing a library clip changes every live reference. Apply and removal are explicit source edits with Undo; YAML formatting is normalized and comments are removed.");
        let mut action = None;
        for (index, instance) in scene.composition.clip_instances.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} · {} · {}",
                    instance.id, instance.actor, instance.clip
                ));
                if instance.actor == actor {
                    if ui.button("Edit").clicked() {
                        self.draft = Some(instance.clone());
                        self.editing = true;
                        self.confirm_delete = false;
                    }
                    if ui.add_enabled(index > 0, egui::Button::new("Up")).clicked() {
                        action = Some(Action::Reorder {
                            id: instance.id.clone(),
                            new_index: index - 1,
                        });
                    }
                    if ui
                        .add_enabled(
                            index + 1 < scene.composition.clip_instances.len(),
                            egui::Button::new("Down"),
                        )
                        .clicked()
                    {
                        action = Some(Action::Reorder {
                            id: instance.id.clone(),
                            new_index: index + 1,
                        });
                    }
                }
            });
        }
        let actor_count = scene
            .composition
            .clip_instances
            .iter()
            .filter(|instance| instance.actor == actor)
            .count();
        let total: usize = script
            .scenes
            .iter()
            .map(|scene| scene.composition.clip_instances.len())
            .sum();
        let can_add = script.characters.contains_key(actor)
            && !script.animation_clips.is_empty()
            && scene.composition.clip_instances.len() < 128
            && actor_count < 32
            && total < 1024;
        if ui
            .add_enabled(can_add, egui::Button::new("New live clip instance"))
            .clicked()
        {
            if let Some((name, clip)) = script.animation_clips.iter().next() {
                self.draft = Some(ClipInstance {
                    id: String::new(),
                    clip: name.clone(),
                    actor: actor.to_owned(),
                    at: 0.0,
                    duration: clip.duration,
                    weight: 1.0,
                });
                self.editing = false;
                self.confirm_delete = false;
            }
        }
        let Some(draft) = self.draft.as_mut() else {
            return action;
        };
        if self.editing {
            ui.label(format!("Instance ID: {}", draft.id));
        } else if ui.text_edit_singleline(&mut draft.id).changed() {
            self.confirm_delete = false;
        }
        ui.small(
            "ID: 1–64 ASCII letters, digits, underscores or hyphens; immutable while editing.",
        );
        let previous = draft.clip.clone();
        egui::ComboBox::from_id_salt("live_clip_reference")
            .selected_text(&draft.clip)
            .show_ui(ui, |ui| {
                for name in script.animation_clips.keys() {
                    ui.selectable_value(&mut draft.clip, name.clone(), name);
                }
            });
        if previous != draft.clip {
            self.confirm_delete = false;
        }
        for (label, value) in [
            ("Start (seconds)", &mut draft.at),
            ("Duration (seconds)", &mut draft.duration),
            ("Override weight (0–1)", &mut draft.weight),
        ] {
            ui.horizontal(|ui| {
                ui.label(label);
                if number_field(ui, value) {
                    self.confirm_delete = false;
                }
            });
        }
        if ui.button("Reset draft timing and weight").clicked() {
            draft.at = 0.0;
            draft.duration = script
                .animation_clips
                .get(&draft.clip)
                .map_or(0.0, |clip| clip.duration);
            draft.weight = 1.0;
            self.confirm_delete = false;
        }
        let valid_id = !draft.id.is_empty()
            && draft.id.len() <= 64
            && draft
                .id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
        let valid = valid_id
            && script.animation_clips.contains_key(&draft.clip)
            && draft.at.is_finite()
            && draft.at >= 0.0
            && draft.duration.is_finite()
            && draft.duration > 0.0
            && (draft.at + draft.duration).is_finite()
            && draft.at + draft.duration > draft.at
            && draft.at + draft.duration <= scene.duration
            && draft.weight.is_finite()
            && (0.0..=1.0).contains(&draft.weight)
            && (self.editing
                || (can_add
                    && !scene
                        .composition
                        .clip_instances
                        .iter()
                        .any(|instance| instance.id == draft.id)));
        if !valid {
            ui.colored_label(egui::Color32::LIGHT_RED, "Choose an existing clip, a unique valid ID, finite timing inside this scene and weight between 0 and 1.");
        }
        if ui
            .add_enabled(valid, egui::Button::new("Apply live clip instance"))
            .clicked()
        {
            action = Some(Action::Upsert {
                instance: draft.clone(),
            });
            self.confirm_delete = false;
        }
        if self.editing {
            ui.checkbox(
                &mut self.confirm_delete,
                "Confirm removal of this live instance",
            );
            if ui
                .add_enabled(
                    self.confirm_delete,
                    egui::Button::new("Remove live instance"),
                )
                .clicked()
            {
                action = Some(Action::Remove {
                    id: draft.id.clone(),
                });
                self.confirm_delete = false;
            }
        }
        action
    }
}

fn number_field(ui: &mut egui::Ui, value: &mut f64) -> bool {
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

    fn captured() -> String {
        motion_script::capture_animation_clip(SOURCE, 0, "A", "Nod", false).unwrap()
    }

    fn instance(id: &str) -> ClipInstance {
        ClipInstance {
            id: id.into(),
            clip: "Nod".into(),
            actor: "B".into(),
            at: 0.0,
            duration: 4.0,
            weight: 0.5,
        }
    }

    #[test]
    fn explicit_actions_upsert_reorder_remove_and_reject_invalid_source_edits() {
        let source = captured();
        let first = Action::Upsert {
            instance: instance("first"),
        }
        .apply(&source, 0)
        .unwrap();
        let second = Action::Upsert {
            instance: instance("second"),
        }
        .apply(&first, 0)
        .unwrap();
        let reordered = Action::Reorder {
            id: "second".into(),
            new_index: 0,
        }
        .apply(&second, 0)
        .unwrap();
        assert_eq!(
            motion_script::parse(&reordered).unwrap().scenes[0]
                .composition
                .clip_instances[0]
                .id,
            "second"
        );
        let removed = Action::Remove { id: "first".into() }
            .apply(&reordered, 0)
            .unwrap();
        assert_eq!(
            motion_script::parse(&removed).unwrap().scenes[0]
                .composition
                .clip_instances
                .len(),
            1
        );
        let mut bad = instance("bad");
        bad.weight = f64::NAN;
        assert!(Action::Upsert { instance: bad }.apply(&source, 0).is_err());
        assert!(motion_script::parse(&source).unwrap().scenes[0]
            .composition
            .clip_instances
            .is_empty());
    }

    #[test]
    fn painting_preserves_invalid_detached_draft_and_context_clears_removal_confirmation() {
        let script = motion_script::parse(&captured()).unwrap();
        let before = serde_json::to_value(&script).unwrap();
        let mut draft = instance("draft");
        draft.at = f64::NAN;
        draft.weight = f64::INFINITY;
        let mut state = State {
            draft: Some(draft),
            editing: false,
            confirm_delete: true,
            context: Some((0, "B".into())),
        };
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(state.ui(ui, &script, 0, "B").is_none());
            });
        });
        assert!(state.draft.as_ref().unwrap().at.is_nan());
        assert_eq!(state.draft.as_ref().unwrap().weight, f64::INFINITY);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(state.ui(ui, &script, 0, "A").is_none());
            });
        });
        assert!(state.draft.is_none());
        assert!(!state.confirm_delete);
        assert_eq!(serde_json::to_value(&script).unwrap(), before);
    }
}
