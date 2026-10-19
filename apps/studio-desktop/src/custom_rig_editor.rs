//! Detached custom rigid-character editor; Apply is an atomic source-history edit.
use eframe::egui;
use motion_script::Script;

pub struct Draft {
    pub actor: String,
    pub yaml: String,
    original: String,
}
impl Draft {
    pub fn for_actor(script: &Script, actor: &str) -> Self {
        let yaml = script.characters.get(actor).and_then(|c| c.rig.as_ref())
            .map(|rig| serde_yaml::to_string(rig).expect("validated serializable rig"))
            .unwrap_or_else(|| TEMPLATE.into());
        Self { actor: actor.into(), original: yaml.clone(), yaml }
    }
    pub fn apply(&self, source: &str) -> Result<String, String> {
        if self.yaml.len() > 100_000 { return Err("Rig draft exceeds the 100 KB editor limit".into()); }
        let mut script = motion_script::parse(source).map_err(|e| e.to_string())?;
        let rig = serde_yaml::from_str(&self.yaml).map_err(|e| e.to_string())?;
        let character = script.characters.get_mut(&self.actor).ok_or("Character no longer exists")?;
        character.rig = Some(rig);
        script.validate().map_err(|e| e.to_string())?;
        serde_yaml::to_string(&script).map_err(|e| e.to_string())
    }
    pub fn remove(&self, source: &str) -> Result<String, String> {
        let mut script = motion_script::parse(source).map_err(|e| e.to_string())?;
        script.characters.get_mut(&self.actor).ok_or("Character no longer exists")?.rig = None;
        script.validate().map_err(|e| e.to_string())?;
        serde_yaml::to_string(&script).map_err(|e| e.to_string())
    }
    /// 1=Apply, 2=restore original humanoid; parent provides context guard/history.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> u8 {
        ui.heading(format!("{} · Custom rigid character", self.actor));
        ui.small("Edit version 1 joint hierarchy and joint-local colored polygons. Joints must precede their children. Part order controls drawing. This replaces humanoid artwork; custom clips, textures and deformation are separate capabilities.");
        ui.add(egui::TextEdit::multiline(&mut self.yaml).code_editor().desired_rows(20).desired_width(f32::INFINITY));
        let mut action=0;
        ui.horizontal(|ui| {
            if ui.button("Apply custom rig").clicked() { action=1; }
            if ui.button("Reset draft").clicked() { self.yaml.clone_from(&self.original); }
            if ui.button("Use original humanoid").clicked() { action=2; }
        });
        action
    }
}
const TEMPLATE: &str = "version: 1\nid: my_character\njoints:\n  - id: root\n    parent: null\n    x: 0\n    y: -40\n  - id: head\n    parent: root\n    x: 0\n    y: -40\nparts:\n  - id: body\n    joint: root\n    points: [[-20, -20], [20, -20], [20, 40], [-20, 40]]\n    color: [70, 160, 220, 255]\n  - id: head_shape\n    joint: head\n    points: [[-18, -18], [18, -18], [18, 18], [-18, 18]]\n    color: [245, 200, 160, 255]\n";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn template_applies_roundtrips_and_invalid_edit_is_atomic() {
        let source=include_str!("../../../examples/hello-emma.yaml");
        let script=motion_script::parse(source).unwrap();
        let actor=script.characters.keys().next().unwrap();
        let mut draft=Draft::for_actor(&script,actor);
        let updated=draft.apply(source).unwrap();
        let parsed=motion_script::parse(&updated).unwrap();
        assert_eq!(parsed.characters[actor].rig.as_ref().unwrap().parts.len(),2);
        draft.yaml=draft.yaml.replace("version: 1","version: 99");
        assert!(draft.apply(&updated).is_err());
        assert!(motion_script::parse(&draft.remove(&updated).unwrap()).unwrap().characters[actor].rig.is_none());
        assert!(script.characters[actor].rig.is_none());
    }
}
