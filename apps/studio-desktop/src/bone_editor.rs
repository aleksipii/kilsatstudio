//! Detached parent-local bone editor. Apply returns a complete validated source transaction.
use eframe::egui;
use motion_script::{ActorEasing, ActorKey, Scene};

pub struct Draft {
    pub scene_index: usize,
    pub actor: String,
    pub bone: String,
    pub keys: Vec<ActorKey>,
    original: Vec<ActorKey>,
    duration: f64,
}

impl Draft {
    pub fn for_bone(scene: &Scene, scene_index: usize, actor: &str, bone: &str) -> Self {
        let keys = scene
            .composition
            .bone_tracks
            .get(actor)
            .and_then(|tracks| tracks.get(bone))
            .cloned()
            .unwrap_or_default();
        Self {
            scene_index,
            actor: actor.into(),
            bone: bone.into(),
            original: keys.clone(),
            keys,
            duration: scene.duration,
        }
    }

    pub fn apply(&self, source: &str) -> Result<String, motion_script::Error> {
        motion_script::apply_bone_track(
            source,
            self.scene_index,
            &self.actor,
            &self.bone,
            &self.keys,
        )
    }

    pub fn reset(&mut self) {
        self.keys.clone_from(&self.original);
    }

    /// Add at the end without modifying previous keys. Full validation is on Apply.
    fn add_key(&mut self) {
        let at = self.keys.last().map_or(0.0, |key| key.at + 1.0);
        if self.keys.len() < 4096 && at <= self.duration {
            self.keys.push(ActorKey {
                at,
                ..self.keys.last().cloned().unwrap_or_default()
            });
        }
    }

    /// Returns true only when Apply is requested. Drawing controls does not sort,
    /// clamp, retime or otherwise rewrite existing keys.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        ui.heading(format!("{} · {} bone keys", self.actor, self.bone));
        ui.small("Parent-local offsets layer after rule clips; parent rotation/scale moves descendants. Translation/rotation ±1000; scale 0.1..4.");
        ui.small("Keys must be in increasing time order inside the scene. Easing controls the segment after a key.");
        let mut remove = None;
        for (index, key) in self.keys.iter_mut().enumerate() {
            ui.push_id(index, |ui| {
                egui::CollapsingHeader::new(format!("Key {} · {:.3}s", index + 1, key.at))
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("key_fields").num_columns(2).show(ui, |ui| {
                            for (label, value, speed) in [
                                ("Time (seconds)", &mut key.at, 0.05),
                                ("X offset", &mut key.x, 1.0),
                                ("Y offset", &mut key.y, 1.0),
                                ("Rotation (radians)", &mut key.rotation, 0.01),
                                ("Scale X", &mut key.scale_x, 0.01),
                                ("Scale Y", &mut key.scale_y, 0.01),
                            ] {
                                ui.label(label);
                                ui.add(egui::DragValue::new(value).speed(speed));
                                ui.end_row();
                            }
                            ui.label("Easing");
                            egui::ComboBox::from_id_salt("easing")
                                .selected_text(easing_label(key.easing))
                                .show_ui(ui, |ui| {
                                    for easing in [
                                        ActorEasing::Linear,
                                        ActorEasing::Smoothstep,
                                        ActorEasing::Hold,
                                    ] {
                                        ui.selectable_value(
                                            &mut key.easing,
                                            easing,
                                            easing_label(easing),
                                        );
                                    }
                                });
                            ui.end_row();
                        });
                        if ui.button("Remove key").clicked() {
                            remove = Some(index);
                        }
                    });
            });
        }
        if let Some(index) = remove {
            self.keys.remove(index);
        }
        let can_add = self.keys.len() < 4096
            && self
                .keys
                .last()
                .is_none_or(|key| key.at + 1.0 <= self.duration);
        if ui
            .add_enabled(can_add, egui::Button::new("Add key"))
            .clicked()
        {
            self.add_key();
        }
        if !can_add {
            ui.small("No room for another key one second after the last; adjust its time or scene duration.");
        }
        if self.keys.is_empty() {
            ui.small("Applying an empty list removes this bone's transform track.");
        }
        ui.small("Apply validates the screenplay and supports Undo. YAML formatting is normalized and comments are removed.");
        let mut apply = false;
        ui.horizontal(|ui| {
            apply = ui.button("Apply bone keys").clicked();
            if ui.button("Reset key draft").clicked() {
                self.reset();
            }
        });
        apply
    }
}

fn easing_label(easing: ActorEasing) -> &'static str {
    match easing {
        ActorEasing::Linear => "Linear",
        ActorEasing::Smoothstep => "Smoothstep",
        ActorEasing::Hold => "Hold",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str =
        "characters: {EMMA: {asset: a}}\nscenes: [{id: shot, duration: 4, events: []}]";
    #[test]
    fn detached_apply_and_reset_preserve_other_tracks() {
        let scene = motion_script::parse(SOURCE).unwrap().scenes.remove(0);
        let mut draft = Draft::for_bone(&scene, 0, "EMMA", "head");
        draft.add_key();
        draft.keys[0].rotation = 0.25;
        assert!(scene.composition.bone_tracks.is_empty());
        let updated = draft.apply(SOURCE).unwrap();
        assert_eq!(
            motion_script::parse(&updated).unwrap().scenes[0]
                .composition
                .bone_tracks["EMMA"]["head"][0]
                .rotation,
            0.25
        );
        draft.reset();
        assert!(draft.keys.is_empty());
    }
    #[test]
    fn headless_controls_do_not_change_precision_or_apply() {
        let scene = motion_script::parse(SOURCE).unwrap().scenes.remove(0);
        let mut draft = Draft::for_bone(&scene, 0, "EMMA", "hip");
        draft.keys.push(ActorKey {
            at: 0.123456789,
            rotation: 0.987654321,
            ..Default::default()
        });
        let expected = draft.keys.clone();
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| assert!(!draft.ui(ui)));
        });
        assert_eq!(draft.keys, expected);
    }
}
