//! Detached actor root-transform track editor. The caller owns validation/history.
use eframe::egui;
use motion_script::{ActorEasing, ActorKey, Scene};

pub struct Draft {
    pub scene_index: usize,
    pub actor: String,
    pub keys: Vec<ActorKey>,
    original: Vec<ActorKey>,
    duration: f64,
}

impl Draft {
    pub fn new(scene: &Scene, scene_index: usize, actor: &str) -> Self {
        let keys = scene
            .composition
            .actor_tracks
            .get(actor)
            .cloned()
            .unwrap_or_default();
        Self {
            scene_index,
            actor: actor.into(),
            original: keys.clone(),
            keys,
            duration: scene.duration,
        }
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
        ui.heading(format!("{} transform keys", self.actor));
        ui.small("Offsets add to movement events. Rotation is radians; scales multiply the character root.");
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
            ui.small("Applying an empty list removes this actor's transform track.");
        }
        ui.small("Apply validates the screenplay and supports Undo. YAML formatting is normalized and comments are removed.");
        let mut apply = false;
        ui.horizontal(|ui| {
            apply = ui.button("Apply transform keys").clicked();
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
    fn scene() -> Scene {
        let script = motion_script::parse("project:\n  title: Keys\ncharacters:\n  EMMA:\n    asset: characters/emma\nscenes:\n  - id: intro\n    duration: 5\n    events: []\n").unwrap();
        script.scenes.into_iter().next().unwrap()
    }
    #[test]
    fn draft_is_detached_and_reset_restores_loaded_track() {
        let mut scene = scene();
        scene.composition.actor_tracks.insert(
            "EMMA".into(),
            vec![ActorKey {
                at: 0.125,
                x: 8.5,
                ..ActorKey::default()
            }],
        );
        let original = scene.composition.clone();
        let mut draft = Draft::new(&scene, 2, "EMMA");
        assert_eq!(draft.scene_index, 2);
        assert_eq!(draft.actor, "EMMA");
        draft.keys[0].x = 100.0;
        draft.add_key();
        assert_eq!(scene.composition, original);
        draft.reset();
        assert_eq!(draft.keys, original.actor_tracks["EMMA"]);
    }
    #[test]
    fn headless_controls_preserve_precise_existing_values() {
        let mut scene = scene();
        scene.composition.actor_tracks.insert(
            "EMMA".into(),
            vec![ActorKey {
                at: 0.123456789,
                rotation: -0.987654321,
                scale_x: 1.012345678,
                easing: ActorEasing::Hold,
                ..ActorKey::default()
            }],
        );
        let mut draft = Draft::new(&scene, 0, "EMMA");
        let expected = draft.keys.clone();
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| assert!(!draft.ui(ui)));
        });
        assert_eq!(draft.keys, expected);
    }
    #[test]
    fn adding_keys_respects_duration_and_limit() {
        let mut draft = Draft::new(&scene(), 0, "EMMA");
        for _ in 0..10 {
            draft.add_key();
        }
        assert_eq!(draft.keys.len(), 6);
        assert_eq!(draft.keys.last().unwrap().at, 5.0);
        draft.keys.clear();
        draft.reset();
        assert!(draft.keys.is_empty());
    }
}
