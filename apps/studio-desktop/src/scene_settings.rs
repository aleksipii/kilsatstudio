//! Detached project and scene settings; the caller validates and commits Apply.
use eframe::egui;
use motion_script::{Project, Script};

pub struct Draft {
    pub scene_index: usize,
    pub project: Project,
    pub scene_id: String,
    pub duration_seconds: f64,
    pub composition: motion_script::Composition,
    actors: Vec<String>,
    is_last_scene: bool,
}

impl Draft {
    pub fn new(script: &Script, scene_index: usize) -> Option<Self> {
        let scene = script.scenes.get(scene_index)?;
        Some(Self {
            scene_index,
            project: script.project.clone(),
            scene_id: scene.id.clone(),
            duration_seconds: scene.duration,
            composition: scene.composition.clone(),
            actors: script.characters.keys().cloned().collect(),
            is_last_scene: scene_index + 1 == script.scenes.len(),
        })
    }

    /// Request application; document validation and history belong to the caller.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        ui.heading("Projektin asetukset");
        ui.small("Projektin asetukset vaikuttavat kaikkiin kohtauksiin.");
        ui.label("Projektin nimi");
        ui.add(egui::TextEdit::singleline(&mut self.project.title).char_limit(512));
        ui.horizontal(|ui| {
            ui.label("Kuvanopeus (fps)");
            ui.add(egui::DragValue::new(&mut self.project.fps).range(1..=240));
        });
        ui.small("Kuvanopeuden muutos säilyttää tapahtumien ajat ja kestot sekunteina.");
        ui.horizontal(|ui| {
            ui.label("Leveys");
            ui.add(egui::DragValue::new(&mut self.project.width).range(1..=16_384));
            ui.label("Korkeus");
            ui.add(egui::DragValue::new(&mut self.project.height).range(1..=16_384));
        });
        ui.small("Käsikirjoituksen kuvakoko voi olla enintään 16384 × 16384. SVG-vienti tukee enintään 8192 kuvapistettä kummallakin sivulla.");
        ui.separator();
        ui.heading("Kohtauksen asetukset");
        ui.label("Kohtauksen tunnus");
        ui.add(egui::TextEdit::singleline(&mut self.scene_id).char_limit(256));
        ui.horizontal(|ui| {
            ui.label("Kesto (s)");
            // The schema has no duration ceiling. Apply rejects nonfinite or
            // nonpositive input rather than silently truncating a long scene.
            ui.add(egui::DragValue::new(&mut self.duration_seconds).speed(0.1));
        });
        ui.small(
            "Anna äärellinen, nollaa suurempi kesto. Äänen viennin enimmäiskesto on yksi tunti.",
        );
        ui.small("Kohtausta ei voi lyhentää, jos tapahtuma jatkuisi sen lopun yli.");
        ui.small("Käytä tarkistaa koko käsikirjoituksen. YAML-muotoilu yhtenäistetään ja kommentit poistuvat. Kumoa palauttaa aiemman käsikirjoituksen.");
        ui.separator();
        egui::CollapsingHeader::new("Tausta, kamera, tasot, häivytykset ja siirtymät").show(
            ui,
            |ui| {
                crate::composition_editor::ui(
                    ui,
                    &mut self.composition,
                    &self.actors,
                    self.duration_seconds,
                    self.is_last_scene,
                );
            },
        );
        ui.button("Käytä asetuksia").clicked()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script() -> Script {
        motion_script::parse(
            "project:\n  title: Original\ncharacters:\n  EMMA:\n    asset: characters/emma\nscenes:\n  - id: first\n    background: backgrounds/studio\n    duration: 5.0\n    events:\n      - type: move\n        at: 1.0\n        actor: EMMA\n        x: 1.0\n        y: 2.0\n        duration: 1.0\n  - id: second\n    duration: 8.0\n    events: []\n",
        )
        .expect("valid settings fixture")
    }

    #[test]
    fn detached_edits_preserve_project_scenes_and_asset_references() {
        let script = script();
        let original = serde_json::to_string(&script).unwrap();
        let mut draft = Draft::new(&script, 0).unwrap();
        draft.project.title = "Changed".into();
        draft.project.fps = 60;
        draft.scene_id = "renamed".into();
        draft.duration_seconds = 2.0;
        assert_eq!(serde_json::to_string(&script).unwrap(), original);
        assert_eq!(
            script.scenes[0].background.as_deref(),
            Some("backgrounds/studio")
        );
        assert_eq!(script.characters["EMMA"].asset, "characters/emma");
    }

    #[test]
    fn selected_scene_is_copied_and_missing_index_returns_none() {
        let script = script();
        let draft = Draft::new(&script, 1).unwrap();
        assert_eq!(draft.scene_index, 1);
        assert_eq!(draft.scene_id, "second");
        assert_eq!(draft.duration_seconds, 8.0);
        assert_eq!(draft.project.title, "Original");
        assert!(Draft::new(&script, script.scenes.len()).is_none());
        assert!(Draft::new(&script, usize::MAX).is_none());
    }
}
