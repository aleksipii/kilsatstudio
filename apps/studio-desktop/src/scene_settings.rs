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
        })
    }

    /// Request application; document validation and history belong to the caller.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        ui.heading("Project settings");
        ui.small("Project settings affect every scene.");
        ui.label("Project title");
        ui.add(egui::TextEdit::singleline(&mut self.project.title).char_limit(512));
        ui.horizontal(|ui| {
            ui.label("Frames per second");
            ui.add(egui::DragValue::new(&mut self.project.fps).range(1..=240));
        });
        ui.small("Changing frame rate keeps event times and durations in seconds.");
        ui.horizontal(|ui| {
            ui.label("Width");
            ui.add(egui::DragValue::new(&mut self.project.width).range(1..=16_384));
            ui.label("Height");
            ui.add(egui::DragValue::new(&mut self.project.height).range(1..=16_384));
        });
        ui.small("The screenplay permits dimensions up to 16384; SVG export currently supports at most 8192 per side.");
        ui.separator();
        ui.heading("Scene settings");
        ui.label("Scene ID");
        ui.add(egui::TextEdit::singleline(&mut self.scene_id).char_limit(256));
        ui.horizontal(|ui| {
            ui.label("Duration (seconds)");
            // The schema has no duration ceiling. Apply rejects nonfinite or
            // nonpositive input rather than silently truncating a long scene.
            ui.add(egui::DragValue::new(&mut self.duration_seconds).speed(0.1));
        });
        ui.small("Duration must be finite and positive. Audio export supports up to one hour.");
        ui.small("Shortening a scene is rejected if an event extends beyond its end.");
        ui.small("Apply validates the entire screenplay. YAML formatting is normalized and comments are removed. Undo restores the previous source.");
        ui.separator();
        egui::CollapsingHeader::new("Background, camera, layers and fades").show(ui, |ui| {
            crate::composition_editor::ui(
                ui,
                &mut self.composition,
                &self.actors,
                self.duration_seconds,
            );
        });
        ui.button("Apply settings").clicked()
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
