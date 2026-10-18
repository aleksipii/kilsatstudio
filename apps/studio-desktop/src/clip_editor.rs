//! Detached scene-local rule clip intensities. Only Apply changes screenplay data.
use eframe::egui;
use motion_script::{ClipWeights, Scene};

pub struct Draft {
    pub scene_index: usize,
    pub actor: String,
    pub weights: ClipWeights,
    original: ClipWeights,
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
        ui.heading(format!("{} · rule clip intensities", self.actor));
        ui.small("Independent additive intensities, not a normalized mix. 0 disables a clip contribution; 1 keeps its full strength.");
        ui.small("These controls affect skeleton rule clips only. Root paths, facial emotion, speech head motion and authored bone offsets remain unchanged.");
        for (label, weight) in [
            ("Posture", &mut self.weights.posture),
            ("Walk", &mut self.weights.walk),
            ("Gesture", &mut self.weights.gesture),
        ] {
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
                "Every intensity must be finite and between 0 and 1 before Apply.",
            );
        }
        ui.small("Apply validates the whole screenplay and supports Undo. YAML formatting is normalized and comments are removed.");
        let mut apply = false;
        ui.horizontal(|ui| {
            apply = ui
                .add_enabled(valid, egui::Button::new("Apply clip intensities"))
                .clicked();
            if ui.button("Reset draft").clicked() {
                self.reset();
            }
            if ui.button("Restore full strength").clicked() {
                self.weights = ClipWeights::default();
            }
        });
        apply
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
}
