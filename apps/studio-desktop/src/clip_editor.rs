//! Detached scene-local rule clip intensities. Only Apply changes screenplay data.
use eframe::egui;
use motion_script::{ActorEasing, ClipWeightKeyframe, ClipWeights, Scene};

pub struct Draft {
    pub scene_index: usize,
    pub actor: String,
    pub weights: ClipWeights,
    original: ClipWeights,
    animated: bool,
}

impl Draft {
    pub fn new(scene: &Scene, scene_index: usize, actor: &str) -> Self {
        let weights = scene
            .composition
            .clip_weights
            .get(actor)
            .copied()
            .unwrap_or_default();
        Self {
            scene_index,
            actor: actor.into(),
            weights,
            original: weights,
            animated: scene.composition.clip_weight_tracks.contains_key(actor),
        }
    }

    pub fn apply(&self, source: &str) -> Result<String, motion_script::Error> {
        motion_script::apply_clip_weights(source, self.scene_index, &self.actor, self.weights)
    }

    pub fn reset(&mut self) {
        self.weights = self.original;
    }

    fn valid(&self) -> bool {
        [
            self.weights.posture,
            self.weights.walk,
            self.weights.gesture,
        ]
        .into_iter()
        .all(|weight| weight.is_finite() && (0.0..=1.0).contains(&weight))
    }

    /// Paints detached controls without normalizing or clamping existing values.
    /// Returns true only for an explicit, locally valid Apply request.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        ui.heading(format!("{} · sääntöliikkeiden voimakkuudet", self.actor));
        if self.animated {
            ui.label("Animoitu voimakkuusrata korvaa nämä vakioarvot, kunnes rata poistetaan.");
        }
        ui.small("Voimakkuudet ovat riippumattomia ja ne summataan. Arvo 0 poistaa liikekatkelman vaikutuksen, arvo 1 säilyttää täyden voimakkuuden.");
        ui.small("Säädöt vaikuttavat luurangon sääntöliikkeisiin. Hahmon kulkureitti, kasvojen tunne, puheen päänliike ja käsin asetetut luusiirtymät säilyvät.");
        for (label, weight) in [
            ("Ryhti", &mut self.weights.posture),
            ("Kävely", &mut self.weights.walk),
            ("Eleet", &mut self.weights.gesture),
        ] {
            if !weight.is_finite() {
                ui.label(format!(
                    "{label}: virheellinen arvo. Palauta luonnos jatkaaksesi."
                ));
                continue;
            }
            let mut edited = *weight;
            let response = ui.add(
                egui::Slider::new(&mut edited, 0.0..=1.0)
                    .text(label)
                    .clamping(egui::SliderClamping::Never),
            );
            if response.changed() {
                *weight = edited;
            }
        }
        let valid = self.valid();
        if !valid {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                "Jokaisen voimakkuuden tulee olla äärellinen luku väliltä 0–1 ennen käyttämistä.",
            );
        }
        ui.small("Käytä tarkistaa koko käsikirjoituksen. Muutoksen voi kumota. YAML-muotoilu yhtenäistetään ja kommentit poistetaan.");
        let mut apply = false;
        ui.horizontal(|ui| {
            apply = ui
                .add_enabled(valid, egui::Button::new("Käytä liikevoimakkuudet"))
                .clicked();
            if ui.button("Palauta luonnos").clicked() {
                self.reset();
            }
            if ui.button("Palauta täysi voimakkuus").clicked() {
                self.weights = ClipWeights::default();
            }
        });
        apply
    }
}

/// Detached animated strengths; applying replaces only this actor's track.
pub struct TrackDraft {
    pub scene_index: usize,
    pub actor: String,
    pub keys: Vec<ClipWeightKeyframe>,
    original: Vec<ClipWeightKeyframe>,
    duration: f64,
}

impl TrackDraft {
    pub fn new(scene: &Scene, scene_index: usize, actor: &str) -> Self {
        let keys = scene
            .composition
            .clip_weight_tracks
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

    pub fn apply(&self, source: &str) -> Result<String, motion_script::Error> {
        motion_script::apply_clip_weight_track(
            source,
            self.scene_index,
            &self.actor,
            self.keys.clone(),
        )
    }

    pub fn reset(&mut self) {
        self.keys = self.original.clone();
    }

    fn valid(&self) -> bool {
        self.keys.len() <= 256
            && self.keys.iter().all(|key| {
                key.time.is_finite()
                    && (0.0..=self.duration).contains(&key.time)
                    && key.weights.is_valid()
            })
            && self.keys.windows(2).all(|pair| pair[0].time < pair[1].time)
    }

    fn next_key_time(&self) -> Option<f64> {
        if self.keys.len() >= 256 || !self.duration.is_finite() || self.duration < 0.0 {
            return None;
        }
        let time = match self.keys.last() {
            None => 0.0,
            Some(key) => {
                if !key.time.is_finite() || !(0.0..self.duration).contains(&key.time) {
                    return None;
                }
                let next = key.time + (self.duration - key.time).min(1.0);
                if next <= key.time {
                    return None;
                }
                next
            }
        };
        (time.is_finite() && (0.0..=self.duration).contains(&time)).then_some(time)
    }

    /// Never sorts, clamps or writes source while controls are painted.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        ui.heading(format!("{} · animoidut liikevoimakkuudet", self.actor));
        ui.small("Tämä rata korvaa vakiovoimakkuudet. Ensimmäinen ja viimeinen arvo säilyvät avainruutujen ulkopuolella. Siirtymätapa vaikuttaa avainruudun jälkeiseen väliin.");
        ui.small("Aikojen tulee kasvaa kohtauksen sisällä. Jokainen voimakkuus on 0–1. Poista kaikki avainruudut ja käytä muutos palauttaaksesi vakiovoimakkuudet.");
        let mut remove = None;
        for (index, key) in self.keys.iter_mut().enumerate() {
            ui.push_id(index, |ui| {
                egui::CollapsingHeader::new(format!("Voimakkuuden avainruutu {}", index + 1))
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Aika (sekuntia)");
                            if key.time.is_finite() {
                                let mut edited = key.time;
                                if ui
                                    .add(egui::DragValue::new(&mut edited).speed(0.05))
                                    .changed()
                                {
                                    key.time = edited;
                                }
                            } else {
                                ui.label("Virheellinen aika. Palauta tai poista tämä avainruutu.");
                            }
                        });
                        for (label, weight) in [
                            ("Ryhti", &mut key.weights.posture),
                            ("Kävely", &mut key.weights.walk),
                            ("Eleet", &mut key.weights.gesture),
                        ] {
                            if weight.is_finite() {
                                let mut edited = *weight;
                                if ui
                                    .add(
                                        egui::Slider::new(&mut edited, 0.0..=1.0)
                                            .text(label)
                                            .clamping(egui::SliderClamping::Never),
                                    )
                                    .changed()
                                {
                                    *weight = edited;
                                }
                            } else {
                                ui.label(format!(
                                    "{label}: virheellinen arvo. Palauta tai poista tämä avainruutu."
                                ));
                            }
                        }
                        egui::ComboBox::from_id_salt("strength_easing")
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
                        if ui.button("Poista voimakkuuden avainruutu").clicked() {
                            remove = Some(index);
                        }
                    });
            });
        }
        if let Some(index) = remove {
            self.keys.remove(index);
        }
        let next_time = self.next_key_time();
        if ui
            .add_enabled(
                next_time.is_some(),
                egui::Button::new("Lisää voimakkuuden avainruutu"),
            )
            .clicked()
        {
            self.keys.push(ClipWeightKeyframe {
                time: next_time.expect("enabled Add requires an available key time"),
                weights: self
                    .keys
                    .last()
                    .map_or_else(ClipWeights::default, |key| key.weights),
                easing: ActorEasing::Linear,
            });
        }
        let valid = self.valid();
        if !valid {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                "Korjaa avainruutujen ajat ja voimakkuudet ennen käyttämistä. Avainruutuja saa olla enintään 256.",
            );
        }
        ui.small("Käytä tarkistaa koko käsikirjoituksen. Muutoksen voi kumota. YAML-muotoilu yhtenäistetään ja kommentit poistetaan.");
        let mut apply = false;
        ui.horizontal(|ui| {
            apply = ui
                .add_enabled(valid, egui::Button::new("Käytä animoidut voimakkuudet"))
                .clicked();
            if ui.button("Palauta voimakkuusradan luonnos").clicked() {
                self.reset();
            }
            if ui.button("Tyhjennä voimakkuusradan luonnos").clicked() {
                self.keys.clear();
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

    const SOURCE: &str = "characters: {A: {asset: a}, B: {asset: b}}\nscenes:\n  - id: shot\n    duration: 4\n    events: [{type: gesture, at: 0, actor: A, value: wave, duration: 2}]\n    composition:\n      clip_weights:\n        B: {posture: 0.2, walk: 0.3, gesture: 0.4}\n  - id: other\n    duration: 2\n    events: []\n";

    #[test]
    fn headless_paint_preserves_invalid_draft_and_original_source() {
        let script = motion_script::parse(SOURCE).unwrap();
        let before = serde_json::to_value(&script).unwrap();
        let mut draft = Draft::new(&script.scenes[0], 0, "A");
        draft.weights.posture = -0.5;
        draft.weights.walk = f64::NAN;
        draft.weights.gesture = 1.5;
        let ctx = egui::Context::default();
        let mut applied = false;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                applied = draft.ui(ui);
            });
        });
        assert!(!applied);
        assert_eq!(draft.weights.posture, -0.5);
        assert!(draft.weights.walk.is_nan());
        assert_eq!(draft.weights.gesture, 1.5);
        assert_eq!(serde_json::to_value(&script).unwrap(), before);
        assert!(draft.apply(SOURCE).is_err());
        draft.reset();
        assert_eq!(draft.weights, ClipWeights::default());
    }

    #[test]
    fn apply_changes_only_selected_actor_and_defaults_remove_entry() {
        let script = motion_script::parse(SOURCE).unwrap();
        let mut draft = Draft::new(&script.scenes[0], 0, "A");
        draft.weights = ClipWeights {
            posture: 0.4,
            walk: 0.5,
            gesture: 0.6,
        };
        let updated = draft.apply(SOURCE).unwrap();
        let mut actual = serde_json::to_value(motion_script::parse(&updated).unwrap()).unwrap();
        let expected = serde_json::to_value(&script).unwrap();
        actual["scenes"][0]["composition"]["clip_weights"]
            .as_object_mut()
            .unwrap()
            .remove("A");
        assert_eq!(actual, expected);
        draft.weights = ClipWeights::default();
        let restored = motion_script::parse(&draft.apply(&updated).unwrap()).unwrap();
        assert!(!restored.scenes[0]
            .composition
            .clip_weights
            .contains_key("A"));
        assert_eq!(serde_json::to_value(restored).unwrap(), expected);
    }

    #[test]
    fn invalid_apply_is_atomic_and_missing_actor_is_rejected() {
        let script = motion_script::parse(SOURCE).unwrap();
        let mut draft = Draft::new(&script.scenes[0], 0, "B");
        let before = SOURCE.to_owned();
        draft.weights.gesture = f64::INFINITY;
        assert!(draft.apply(SOURCE).is_err());
        assert_eq!(SOURCE, before);
        draft.reset();
        draft.actor = "missing".into();
        assert!(draft.apply(SOURCE).is_err());
    }

    #[test]
    fn animated_track_apply_is_actor_local_and_empty_restores_static() {
        let script = motion_script::parse(SOURCE).unwrap();
        let mut draft = TrackDraft::new(&script.scenes[0], 0, "B");
        draft.keys = vec![ClipWeightKeyframe {
            time: 0.5,
            weights: ClipWeights {
                posture: 0.8,
                walk: 0.6,
                gesture: 0.2,
            },
            easing: ActorEasing::Smoothstep,
        }];
        let updated = draft.apply(SOURCE).unwrap();
        let parsed = motion_script::parse(&updated).unwrap();
        assert_eq!(
            parsed.scenes[0].composition.clip_weight_tracks["B"],
            draft.keys
        );
        let mut actual = serde_json::to_value(&parsed).unwrap();
        actual["scenes"][0]["composition"]["clip_weight_tracks"]
            .as_object_mut()
            .unwrap()
            .remove("B");
        assert_eq!(actual, serde_json::to_value(&script).unwrap());
        draft.keys.clear();
        let restored = motion_script::parse(&draft.apply(&updated).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(&restored).unwrap(),
            serde_json::to_value(&script).unwrap()
        );
        assert_eq!(
            restored.scenes[0].composition.sample_clip_weights("B", 1.0),
            script.scenes[0].composition.clip_weights["B"]
        );
    }

    #[test]
    fn animated_track_headless_paint_preserves_invalid_values_and_reset() {
        let script = motion_script::parse(SOURCE).unwrap();
        let mut draft = TrackDraft::new(&script.scenes[0], 0, "A");
        draft.keys.push(ClipWeightKeyframe {
            time: f64::NAN,
            weights: ClipWeights {
                posture: -0.5,
                walk: f64::INFINITY,
                gesture: 1.5,
            },
            easing: ActorEasing::Hold,
        });
        let ctx = egui::Context::default();
        let mut applied = false;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                applied = draft.ui(ui);
            });
        });
        assert!(!applied);
        assert!(draft.keys[0].time.is_nan());
        assert_eq!(draft.keys[0].weights.posture, -0.5);
        assert!(draft.keys[0].weights.walk.is_infinite());
        assert_eq!(draft.keys[0].weights.gesture, 1.5);
        assert!(draft.apply(SOURCE).is_err());
        draft.reset();
        assert!(draft.keys.is_empty());
        assert!(draft.valid());
    }

    #[test]
    fn animated_track_rejects_duplicate_times_outside_scene_and_overflow() {
        let script = motion_script::parse(SOURCE).unwrap();
        let mut draft = TrackDraft::new(&script.scenes[0], 0, "A");
        let key = ClipWeightKeyframe {
            time: 0.0,
            weights: ClipWeights::default(),
            easing: ActorEasing::Linear,
        };
        draft.keys = vec![key.clone(), key.clone()];
        assert!(!draft.valid());
        assert!(draft.apply(SOURCE).is_err());
        draft.keys = vec![key.clone()];
        draft.keys[0].time = 4.5;
        assert!(!draft.valid());
        assert!(draft.apply(SOURCE).is_err());
        draft.keys = (0..257)
            .map(|index| ClipWeightKeyframe {
                time: index as f64 / 100.0,
                ..key.clone()
            })
            .collect();
        assert!(!draft.valid());
        assert!(draft.apply(SOURCE).is_err());
    }

    #[test]
    fn animated_track_add_can_reach_subsecond_scene_and_fractional_tail() {
        let script = motion_script::parse(SOURCE).unwrap();
        let mut draft = TrackDraft::new(&script.scenes[0], 0, "A");
        draft.duration = 0.5;
        assert_eq!(draft.next_key_time(), Some(0.0));
        draft.keys.push(ClipWeightKeyframe {
            time: 0.0,
            weights: ClipWeights::default(),
            easing: ActorEasing::Linear,
        });
        assert_eq!(draft.next_key_time(), Some(0.5));
        draft.keys[0].time = 0.5;
        assert_eq!(draft.next_key_time(), None);
        draft.duration = 4.0;
        draft.keys[0].time = 3.75;
        assert_eq!(draft.next_key_time(), Some(4.0));
        draft.keys[0].time = f64::NAN;
        assert_eq!(draft.next_key_time(), None);
        draft.duration = f64::INFINITY;
        draft.keys.clear();
        assert_eq!(draft.next_key_time(), None);
    }
}
