//! Detached editors for a scene's audio cues and the screenplay's animation rules.
//! Apply returns a complete validated source transaction; drawing the controls never
//! sorts, clamps or otherwise rewrites values. Cues reference project-relative `.wav`
//! paths only: nothing here opens or decodes a file.
use eframe::egui;
use motion_script::{
    AudioCue, ClipBlendMode, Rule, RuleCameraPreset, RuleCondition, RuleEffect, RuleTrigger, Rules,
    Scene, MAX_AUDIO_CUES, MAX_RULES, MAX_RULE_NAME,
};

const ID_LIMIT: usize = 64;
const ASSET_LIMIT: usize = 512;

fn text(ui: &mut egui::Ui, value: &mut String, limit: usize) {
    ui.add(egui::TextEdit::singleline(value).char_limit(limit));
}

pub struct CueDraft {
    pub scene_index: usize,
    pub scene_id: String,
    pub cues: Vec<AudioCue>,
    original: Vec<AudioCue>,
}

impl CueDraft {
    pub fn new(scene: &Scene, scene_index: usize) -> Self {
        let cues = scene.composition.audio_cues.clone();
        Self {
            scene_index,
            scene_id: scene.id.clone(),
            original: cues.clone(),
            cues,
        }
    }

    pub fn apply(&self, source: &str) -> Result<String, motion_script::Error> {
        motion_script::apply_audio_cues(source, self.scene_index, &self.cues)
    }

    pub fn reset(&mut self) {
        self.cues.clone_from(&self.original);
    }

    fn add_cue(&mut self) {
        if self.cues.len() < MAX_AUDIO_CUES {
            let mut n = self.cues.len() + 1;
            while self.cues.iter().any(|c| c.id == format!("cue_{n}")) {
                n += 1;
            }
            self.cues.push(AudioCue {
                id: format!("cue_{n}"),
                asset: "audio/".into(),
                at: 0.0,
                gain: 1.0,
                trim_start: 0.0,
                duration: None,
            });
        }
    }

    /// Returns true only when Apply is requested.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        ui.small("Sound effects from project-relative 24 kHz mono PCM16 .wav files, placed on the scene's sample clock. Files are read when audio is prepared, not here.");
        let mut remove = None;
        for (index, cue) in self.cues.iter_mut().enumerate() {
            ui.push_id(index, |ui| {
                egui::CollapsingHeader::new(format!(
                    "Cue {} · {} · {:.3}s",
                    index + 1,
                    cue.id,
                    cue.at
                ))
                .default_open(true)
                .show(ui, |ui| {
                    egui::Grid::new("cue_fields").num_columns(2).show(ui, |ui| {
                        ui.label("Id");
                        text(ui, &mut cue.id, ID_LIMIT);
                        ui.end_row();
                        ui.label("Asset (.wav)");
                        text(ui, &mut cue.asset, ASSET_LIMIT);
                        ui.end_row();
                        for (label, value, speed) in [
                            ("Start (scene seconds)", &mut cue.at, 0.05),
                            ("Gain (0..4)", &mut cue.gain, 0.01),
                            ("Trim start (source seconds)", &mut cue.trim_start, 0.05),
                        ] {
                            ui.label(label);
                            ui.add(egui::DragValue::new(value).speed(speed));
                            ui.end_row();
                        }
                        ui.label("Duration");
                        ui.horizontal(|ui| {
                            let mut limited = cue.duration.is_some();
                            if ui.checkbox(&mut limited, "Limit").changed() {
                                cue.duration = limited.then_some(1.0);
                            }
                            if let Some(duration) = &mut cue.duration {
                                ui.add(egui::DragValue::new(duration).speed(0.05));
                            }
                        });
                        ui.end_row();
                    });
                    if ui.button("Remove cue").clicked() {
                        remove = Some(index);
                    }
                });
            });
        }
        if let Some(index) = remove {
            self.cues.remove(index);
        }
        let can_add = self.cues.len() < MAX_AUDIO_CUES;
        if ui
            .add_enabled(can_add, egui::Button::new("Add cue"))
            .clicked()
        {
            self.add_cue();
        }
        if self.cues.is_empty() {
            ui.small("Applying an empty list removes this scene's cues.");
        }
        ui.small("Apply validates the screenplay and supports Undo. A missing or unsupported file is reported when audio is prepared.");
        let mut apply = false;
        ui.horizontal(|ui| {
            apply = ui.button("Apply audio cues").clicked();
            if ui.button("Reset cue draft").clicked() {
                self.reset();
            }
        });
        apply
    }
}

/// One named rule plus the text forms of its optional string fields.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleRow {
    pub name: String,
    pub rule: Rule,
    scenes: String,
    value: String,
    when_actor: String,
    when_emotion: String,
    effect_actor: String,
}

fn opt(text: &str) -> Option<String> {
    let t = text.trim();
    (!t.is_empty()).then(|| t.to_string())
}

impl RuleRow {
    fn from_rule(name: &str, rule: &Rule) -> Self {
        let effect_actor = match &rule.effect {
            RuleEffect::Clip { actor, .. } => actor.clone().unwrap_or_default(),
            RuleEffect::Camera { .. } => String::new(),
        };
        Self {
            name: name.into(),
            rule: rule.clone(),
            scenes: rule.condition.scenes.join(", "),
            value: rule.value.clone().unwrap_or_default(),
            when_actor: rule.condition.actor.clone().unwrap_or_default(),
            when_emotion: rule.condition.emotion.clone().unwrap_or_default(),
            effect_actor,
        }
    }

    fn build(&self) -> Rule {
        let mut rule = self.rule.clone();
        rule.value = opt(&self.value);
        rule.condition.scenes = self.scenes.split(',').filter_map(opt).collect();
        rule.condition.actor = opt(&self.when_actor);
        rule.condition.emotion = opt(&self.when_emotion);
        if let RuleEffect::Clip { actor, .. } = &mut rule.effect {
            *actor = opt(&self.effect_actor);
        }
        rule
    }

    fn new(name: String) -> Self {
        Self::from_rule(
            &name,
            &Rule {
                on: RuleTrigger::Emotion,
                value: None,
                at: None,
                condition: RuleCondition::default(),
                effect: RuleEffect::Camera {
                    preset: RuleCameraPreset::PushIn,
                    offset: 0.0,
                    duration: 1.0,
                },
            },
        )
    }
}

pub struct RuleDraft {
    pub rows: Vec<RuleRow>,
    original: Vec<RuleRow>,
}

impl RuleDraft {
    pub fn new(rules: &Rules) -> Self {
        let rows: Vec<RuleRow> = rules
            .iter()
            .map(|(n, r)| RuleRow::from_rule(n, r))
            .collect();
        Self {
            original: rows.clone(),
            rows,
        }
    }

    /// Duplicate names are an error rather than silently merged.
    pub fn rules(&self) -> Result<Rules, motion_script::Error> {
        let mut rules = Rules::new();
        for row in &self.rows {
            if rules.insert(row.name.clone(), row.build()).is_some() {
                return Err(motion_script::Error::Validation(format!(
                    "duplicate rule name {}",
                    row.name
                )));
            }
        }
        Ok(rules)
    }

    pub fn apply(&self, source: &str) -> Result<String, motion_script::Error> {
        motion_script::apply_rules(source, self.rules()?)
    }

    pub fn reset(&mut self) {
        self.rows.clone_from(&self.original);
    }

    pub(crate) fn add_rule(&mut self) {
        if self.rows.len() < MAX_RULES {
            let mut n = self.rows.len() + 1;
            while self.rows.iter().any(|r| r.name == format!("rule_{n}")) {
                n += 1;
            }
            self.rows.push(RuleRow::new(format!("rule_{n}")));
        }
    }

    /// Returns true only when Apply is requested.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        ui.small("Declarative rules fire on authored events and add a clip instance or camera move. Rules apply to the whole screenplay; validation covers clip/actor/scene references, overlaps and bounds.");
        let mut remove = None;
        for (index, row) in self.rows.iter_mut().enumerate() {
            ui.push_id(index, |ui| {
                egui::CollapsingHeader::new(format!("Rule {}", row.name))
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("rule_fields")
                            .num_columns(2)
                            .show(ui, |ui| {
                                ui.label("Name");
                                text(ui, &mut row.name, MAX_RULE_NAME);
                                ui.end_row();
                                ui.label("Trigger");
                                egui::ComboBox::from_id_salt("trigger")
                                    .selected_text(trigger_label(row.rule.on))
                                    .show_ui(ui, |ui| {
                                        for t in [
                                            RuleTrigger::Emotion,
                                            RuleTrigger::Speak,
                                            RuleTrigger::Gesture,
                                            RuleTrigger::Move,
                                            RuleTrigger::Time,
                                        ] {
                                            ui.selectable_value(
                                                &mut row.rule.on,
                                                t,
                                                trigger_label(t),
                                            );
                                        }
                                    });
                                ui.end_row();
                                ui.label("Match value");
                                text(ui, &mut row.value, ID_LIMIT);
                                ui.end_row();
                                if row.rule.on == RuleTrigger::Time {
                                    ui.label("At (scene seconds)");
                                    opt_drag(ui, &mut row.rule.at, 0.0);
                                    ui.end_row();
                                }
                                ui.label("Only scenes (comma list)");
                                text(ui, &mut row.scenes, ASSET_LIMIT);
                                ui.end_row();
                                ui.label("Subject actor");
                                text(ui, &mut row.when_actor, ID_LIMIT);
                                ui.end_row();
                                ui.label("Subject emotion");
                                text(ui, &mut row.when_emotion, ID_LIMIT);
                                ui.end_row();
                                ui.label("From (seconds)");
                                opt_drag(ui, &mut row.rule.condition.from, 0.0);
                                ui.end_row();
                                ui.label("Until (seconds)");
                                opt_drag(ui, &mut row.rule.condition.until, 1.0);
                                ui.end_row();
                                effect_ui(ui, row);
                            });
                        if ui.button("Remove rule").clicked() {
                            remove = Some(index);
                        }
                    });
            });
        }
        if let Some(index) = remove {
            self.rows.remove(index);
        }
        if ui
            .add_enabled(self.rows.len() < MAX_RULES, egui::Button::new("Add rule"))
            .clicked()
        {
            self.add_rule();
        }
        if self.rows.is_empty() {
            ui.small("Applying an empty list removes all rules.");
        }
        ui.small("Apply validates the expanded screenplay and supports Undo. YAML formatting is normalized and comments are removed.");
        let mut apply = false;
        ui.horizontal(|ui| {
            apply = ui.button("Apply rules").clicked();
            if ui.button("Reset rule draft").clicked() {
                self.reset();
            }
        });
        apply
    }
}

fn opt_drag(ui: &mut egui::Ui, value: &mut Option<f64>, default: f64) {
    ui.horizontal(|ui| {
        let mut on = value.is_some();
        if ui.checkbox(&mut on, "Set").changed() {
            *value = on.then_some(default);
        }
        if let Some(v) = value {
            ui.add(egui::DragValue::new(v).speed(0.05));
        }
    });
}

fn effect_ui(ui: &mut egui::Ui, row: &mut RuleRow) {
    let is_clip = matches!(row.rule.effect, RuleEffect::Clip { .. });
    ui.label("Effect");
    let mut choose = None;
    egui::ComboBox::from_id_salt("effect")
        .selected_text(if is_clip {
            "Clip instance"
        } else {
            "Camera move"
        })
        .show_ui(ui, |ui| {
            if ui.selectable_label(is_clip, "Clip instance").clicked() && !is_clip {
                choose = Some(true);
            }
            if ui.selectable_label(!is_clip, "Camera move").clicked() && is_clip {
                choose = Some(false);
            }
        });
    ui.end_row();
    match choose {
        Some(true) => {
            row.rule.effect = RuleEffect::Clip {
                clip: String::new(),
                actor: None,
                offset: 0.0,
                duration: 1.0,
                weight: 1.0,
                blend_mode: ClipBlendMode::Override,
                fade_in: 0.0,
                fade_out: 0.0,
            }
        }
        Some(false) => {
            row.rule.effect = RuleEffect::Camera {
                preset: RuleCameraPreset::PushIn,
                offset: 0.0,
                duration: 1.0,
            };
            row.effect_actor.clear();
        }
        None => {}
    }
    match &mut row.rule.effect {
        RuleEffect::Clip {
            clip,
            offset,
            duration,
            weight,
            blend_mode,
            fade_in,
            fade_out,
            ..
        } => {
            ui.label("Clip");
            text(ui, clip, ID_LIMIT);
            ui.end_row();
            ui.label("Target actor");
            text(ui, &mut row.effect_actor, ID_LIMIT);
            ui.end_row();
            for (label, value) in [
                ("Offset (seconds)", offset),
                ("Duration", duration),
                ("Weight (0..1)", weight),
                ("Fade in", fade_in),
                ("Fade out", fade_out),
            ] {
                ui.label(label);
                ui.add(egui::DragValue::new(value).speed(0.05));
                ui.end_row();
            }
            ui.label("Blend");
            egui::ComboBox::from_id_salt("blend")
                .selected_text(format!("{blend_mode:?}"))
                .show_ui(ui, |ui| {
                    for b in [ClipBlendMode::Override, ClipBlendMode::Additive] {
                        ui.selectable_value(blend_mode, b, format!("{b:?}"));
                    }
                });
            ui.end_row();
        }
        RuleEffect::Camera {
            preset,
            offset,
            duration,
        } => {
            ui.label("Preset");
            egui::ComboBox::from_id_salt("preset")
                .selected_text(format!("{preset:?}"))
                .show_ui(ui, |ui| {
                    for p in [
                        RuleCameraPreset::PushIn,
                        RuleCameraPreset::PullOut,
                        RuleCameraPreset::PanLeft,
                        RuleCameraPreset::PanRight,
                    ] {
                        ui.selectable_value(preset, p, format!("{p:?}"));
                    }
                });
            ui.end_row();
            for (label, value) in [("Offset (seconds)", offset), ("Duration", duration)] {
                ui.label(label);
                ui.add(egui::DragValue::new(value).speed(0.05));
                ui.end_row();
            }
        }
    }
}

fn trigger_label(t: RuleTrigger) -> &'static str {
    match t {
        RuleTrigger::Emotion => "Emotion event",
        RuleTrigger::Speak => "Speak event",
        RuleTrigger::Gesture => "Gesture event",
        RuleTrigger::Move => "Move event",
        RuleTrigger::Time => "Fixed time",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(crate) const SOURCE: &str = "animation_clips:\n  wave: {duration: 2, bone_tracks: {head: [{at: 0, x: 2}, {at: 2, x: 4}]}}\ncharacters: {EMMA: {asset: a}}\nscenes:\n  - {id: shot, duration: 4, events: [{type: emotion, at: 1, actor: EMMA, value: happy}]}\n";
    fn scene() -> Scene {
        motion_script::parse(SOURCE).unwrap().scenes.remove(0)
    }
    fn run(mut draw: impl FnMut(&mut egui::Ui) -> bool) {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| assert!(!draw(ui)));
        });
    }
    #[test]
    fn cue_draft_applies_removes_and_rejects_invalid() {
        let mut draft = CueDraft::new(&scene(), 0);
        draft.add_cue();
        draft.cues[0].asset = "audio/door.wav".into();
        draft.cues[0].at = 1.5;
        let updated = draft.apply(SOURCE).unwrap();
        assert_eq!(
            motion_script::parse(&updated).unwrap().scenes[0]
                .composition
                .audio_cues,
            draft.cues
        );
        draft.cues[0].at = 9.0;
        assert!(draft.apply(SOURCE).is_err());
        draft.reset();
        assert!(draft.cues.is_empty());
        for _ in 0..80 {
            draft.add_cue();
        }
        assert_eq!(draft.cues.len(), MAX_AUDIO_CUES);
    }
    #[test]
    fn cue_controls_render_without_rewriting() {
        let mut draft = CueDraft::new(&scene(), 0);
        draft.cues.push(AudioCue {
            id: "x".into(),
            asset: "audio/x.wav".into(),
            at: 0.123456789,
            gain: 0.987654321,
            trim_start: 0.0,
            duration: Some(0.5),
        });
        let expected = draft.cues.clone();
        run(|ui| draft.ui(ui));
        assert_eq!(draft.cues, expected);
    }
    #[test]
    fn rule_draft_round_trips_applies_and_rejects_invalid() {
        let mut draft = RuleDraft::new(&Rules::new());
        draft.add_rule();
        draft.rows[0].name = "wave_happy".into();
        draft.rows[0].value = "happy".into();
        draft.rows[0].scenes = "shot, other".into();
        draft.rows[0].rule.effect = RuleEffect::Clip {
            clip: "wave".into(),
            actor: None,
            offset: 0.5,
            duration: 1.0,
            weight: 0.5,
            blend_mode: ClipBlendMode::Override,
            fade_in: 0.0,
            fade_out: 0.0,
        };
        draft.rows[0].effect_actor = "EMMA".into();
        // Unknown scene "other" is a real validation error.
        assert!(draft.apply(SOURCE).is_err());
        draft.rows[0].scenes = "shot".into();
        let updated = draft.apply(SOURCE).unwrap();
        let script = motion_script::parse(&updated).unwrap();
        let reopened = RuleDraft::new(&script.rules);
        assert_eq!(reopened.rules().unwrap(), draft.rules().unwrap());
        assert_eq!(reopened.rows[0].scenes, "shot");
        let mut dup = RuleDraft::new(&script.rules);
        dup.rows.push(dup.rows[0].clone());
        assert!(dup.apply(&updated).is_err());
        draft.rows.clear();
        let cleared = motion_script::parse(&draft.apply(&updated).unwrap()).unwrap();
        assert!(cleared.rules.is_empty());
    }
    #[test]
    fn rule_controls_render_without_rewriting() {
        let mut draft = RuleDraft::new(&Rules::new());
        draft.add_rule();
        draft.rows[0].rule.condition.from = Some(0.123456789);
        let expected = draft.rows.clone();
        run(|ui| draft.ui(ui));
        assert_eq!(draft.rows, expected);
    }
}
