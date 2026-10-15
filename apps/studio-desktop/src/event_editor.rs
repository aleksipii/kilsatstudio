//! A detached event draft. Applying it is an explicit document transaction.
use eframe::egui;
use motion_script::{Event, Script};

pub struct Draft {
    pub scene_index: usize,
    pub event_index: usize,
    pub event: Event,
}
impl Draft {
    pub fn new(script: &Script, scene_index: usize, event_index: usize) -> Option<Self> {
        Some(Self {
            scene_index,
            event_index,
            event: script
                .scenes
                .get(scene_index)?
                .events
                .get(event_index)?
                .clone(),
        })
    }

    /// Return true only when the user requests validation and application.
    pub fn ui(&mut self, ui: &mut egui::Ui, actors: &[String]) -> bool {
        let (at, actor) = match &mut self.event {
            Event::Move { at, actor, .. }
            | Event::Speak { at, actor, .. }
            | Event::Emotion { at, actor, .. }
            | Event::Gesture { at, actor, .. } => (at, actor),
        };
        egui::ComboBox::from_id_salt("event_actor")
            .selected_text(actor.as_str())
            .show_ui(ui, |ui| {
                for name in actors {
                    ui.selectable_value(actor, name.clone(), name);
                }
            });
        number(ui, "Start (s)", at);
        match &mut self.event {
            Event::Move { x, y, duration, .. } => {
                number(ui, "X", x);
                number(ui, "Y", y);
                number(ui, "Duration (s)", duration);
            }
            Event::Emotion { value, .. } => emotion(ui, "Expression", value),
            Event::Gesture {
                value, duration, ..
            } => {
                egui::ComboBox::from_label("Gesture")
                    .selected_text(value.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(value, "wave".into(), "wave");
                    });
                number(ui, "Duration (s)", duration);
            }
            Event::Speak {
                text,
                emotion: style,
                audio,
                ..
            } => {
                ui.label("Dialogue");
                ui.add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(4)
                        .char_limit(100_000),
                );
                let mut enabled = style.is_some();
                if ui.checkbox(&mut enabled, "Expression metadata").changed() {
                    *style = enabled.then(|| "neutral".into());
                }
                if let Some(style) = style {
                    emotion(ui, "Dialogue expression", style);
                }
                ui.small("Expression metadata does not condition Kokoro speech.");
                let mut imported = audio.is_some();
                if ui.checkbox(&mut imported, "Use imported audio").changed() {
                    *audio = imported.then(String::new);
                }
                if let Some(path) = audio {
                    ui.label("Audio path");
                    ui.add(egui::TextEdit::singleline(path).char_limit(4096));
                }
            }
        }
        ui.small("Apply validates the entire screenplay. YAML formatting is normalized and comments are removed. Undo restores the previous source.");
        ui.button("Apply event").clicked()
    }
}
fn number(ui: &mut egui::Ui, label: &str, value: &mut f64) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).speed(0.1));
    });
}
fn emotion(ui: &mut egui::Ui, label: &str, value: &mut String) {
    egui::ComboBox::from_label(label)
        .selected_text(value.as_str())
        .show_ui(ui, |ui| {
            for expression in ["neutral", "happy", "sad", "surprised", "angry"] {
                ui.selectable_value(value, expression.into(), expression);
            }
        });
}
