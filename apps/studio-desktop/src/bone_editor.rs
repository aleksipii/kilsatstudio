//! Detached parent-local bone and custom-rig joint key editor. Apply returns a complete
//! validated source transaction.
use eframe::egui;
use motion_script::{ActorEasing, ActorKey, Scene};

pub struct Draft {
    pub scene_index: usize,
    pub actor: String,
    /// Bone name, or joint id when `joint` is set.
    pub bone: String,
    /// Edits a custom-rig joint track instead of an original-rig bone track.
    pub joint: bool,
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
        Self::from_keys(scene, scene_index, actor, bone, keys)
    }

    pub fn for_joint(scene: &Scene, scene_index: usize, actor: &str, joint: &str) -> Self {
        let keys = scene
            .composition
            .joint_tracks
            .get(actor)
            .and_then(|tracks| tracks.get(joint))
            .cloned()
            .unwrap_or_default();
        Self {
            joint: true,
            ..Self::from_keys(scene, scene_index, actor, joint, keys)
        }
    }

    fn from_keys(
        scene: &Scene,
        scene_index: usize,
        actor: &str,
        name: &str,
        keys: Vec<ActorKey>,
    ) -> Self {
        Self {
            scene_index,
            actor: actor.into(),
            bone: name.into(),
            joint: false,
            original: keys.clone(),
            keys,
            duration: scene.duration,
        }
    }

    pub fn apply(&self, source: &str) -> Result<String, motion_script::Error> {
        if self.joint {
            return motion_script::apply_joint_track(
                source,
                self.scene_index,
                &self.actor,
                &self.bone,
                &self.keys,
            );
        }
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
        let (noun, hint) = if self.joint {
            (
                "nivelen",
                "Siirtymä ja kierto lisätään nivelen lepoasentoon, mittakaava kerrotaan. Alanivelet seuraavat mukana. Siirtymä ja kierto ±1000, mittakaava 0,1–4.",
            )
        } else {
            (
                "luun",
                "Siirtymät ovat suhteessa yläniveleen ja vaikuttavat sääntöliikkeiden jälkeen. Ylänivelen kierto ja mittakaava liikuttavat alaniveliä. Siirtymä ja kierto ±1000, mittakaava 0,1–4.",
            )
        };
        ui.heading(format!(
            "{} · {} · {noun} avainruudut",
            self.actor, self.bone
        ));
        ui.small(hint);
        ui.small("Avainruutujen aikojen tulee kasvaa kohtauksen sisällä. Siirtymätapa vaikuttaa avainruudun jälkeiseen väliin.");
        let mut remove = None;
        for (index, key) in self.keys.iter_mut().enumerate() {
            ui.push_id(index, |ui| {
                egui::CollapsingHeader::new(format!("Avainruutu {} · {:.3} s", index + 1, key.at))
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("key_fields").num_columns(2).show(ui, |ui| {
                            for (label, value, speed) in [
                                ("Aika (sekuntia)", &mut key.at, 0.05),
                                ("X-siirtymä", &mut key.x, 1.0),
                                ("Y-siirtymä", &mut key.y, 1.0),
                                ("Kierto (radiaania)", &mut key.rotation, 0.01),
                                ("X-mittakaava", &mut key.scale_x, 0.01),
                                ("Y-mittakaava", &mut key.scale_y, 0.01),
                            ] {
                                ui.label(label);
                                ui.add(egui::DragValue::new(value).speed(speed));
                                ui.end_row();
                            }
                            ui.label("Siirtymätapa");
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
                        if ui.button("Poista avainruutu").clicked() {
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
            .add_enabled(can_add, egui::Button::new("Lisää avainruutu"))
            .clicked()
        {
            self.add_key();
        }
        if !can_add {
            ui.small("Seuraava avainruutu ei mahdu sekunnin päähän edellisestä. Muuta edellisen aikaa tai kohtauksen kestoa.");
        }
        if self.keys.is_empty() {
            ui.small("Tyhjän luettelon käyttäminen poistaa tämän liikeradan.");
        }
        ui.small("Käytä tarkistaa käsikirjoituksen. Muutoksen voi kumota. YAML-muotoilu yhtenäistetään ja kommentit poistetaan.");
        let mut apply = false;
        ui.horizontal(|ui| {
            apply = ui.button(format!("Käytä {noun} avainruudut")).clicked();
            if ui.button("Palauta avainruutuluonnos").clicked() {
                self.reset();
            }
        });
        apply
    }
}

fn easing_label(easing: ActorEasing) -> &'static str {
    match easing {
        ActorEasing::Linear => "Lineaarinen",
        ActorEasing::Smoothstep => "Pehmeä",
        ActorEasing::Hold => "Pidä arvo",
        ActorEasing::EaseIn => "Hidas alku",
        ActorEasing::EaseOut => "Hidas loppu",
        ActorEasing::EaseInOut => "Hidas alku ja loppu",
        ActorEasing::Back(_) => "Ylilyönti",
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
    const RIG: &str = "characters:\n  P:\n    asset: custom\n    rig:\n      version: 1\n      id: p\n      joints: [{id: root, parent: null}, {id: arm, parent: root, x: 5}]\n      parts: [{id: a, joint: arm, points: [[0, 0], [2, 0], [0, 2]], color: [1, 2, 3, 255]}]\nscenes:\n  - {id: shot, duration: 4, events: []}\n";
    #[test]
    fn joint_draft_loads_applies_and_removes_tracks_without_touching_bone_tracks() {
        let scene = motion_script::parse(RIG).unwrap().scenes.remove(0);
        let mut draft = Draft::for_joint(&scene, 0, "P", "arm");
        assert!(draft.joint && draft.keys.is_empty());
        draft.add_key();
        draft.add_key();
        draft.keys[1].rotation = 0.5;
        let updated = draft.apply(RIG).unwrap();
        let script = motion_script::parse(&updated).unwrap();
        let saved = &script.scenes[0].composition.joint_tracks["P"]["arm"];
        assert_eq!(saved, &draft.keys);
        assert!(script.scenes[0].composition.bone_tracks.is_empty());
        // Reopening loads the saved keys; applying none removes the track.
        let mut reopened = Draft::for_joint(&script.scenes[0], 0, "P", "arm");
        assert_eq!(reopened.keys, draft.keys);
        reopened.keys.clear();
        let cleared = motion_script::parse(&reopened.apply(&updated).unwrap()).unwrap();
        assert!(cleared.scenes[0].composition.joint_tracks.is_empty());
        // Unknown joints and out-of-range scale are rejected and leave the source alone.
        let mut bad = Draft::for_joint(&scene, 0, "P", "missing");
        bad.add_key();
        assert!(bad.apply(RIG).is_err());
        let mut bad = Draft::for_joint(&scene, 0, "P", "arm");
        bad.add_key();
        bad.keys[0].scale_x = 0.0;
        assert!(bad.apply(RIG).is_err());
    }
    #[test]
    fn joint_controls_render_headlessly_without_rewriting_keys() {
        let scene = motion_script::parse(RIG).unwrap().scenes.remove(0);
        let mut draft = Draft::for_joint(&scene, 0, "P", "arm");
        draft.keys.push(ActorKey {
            at: 0.3,
            rotation: 0.7,
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
