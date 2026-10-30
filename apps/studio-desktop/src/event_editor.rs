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
            | Event::Gesture { at, actor, .. }
            | Event::Face { at, actor, .. }
            | Event::Hold { at, actor, .. }
            | Event::Pose { at, actor, .. } => (at, actor),
        };
        egui::ComboBox::from_id_salt("event_actor")
            .selected_text(actor.as_str())
            .show_ui(ui, |ui| {
                for name in actors {
                    ui.selectable_value(actor, name.clone(), name);
                }
            });
        number(ui, "Alku (s)", at);
        match &mut self.event {
            Event::Move {
                x,
                y,
                duration,
                gait,
                ease,
                ..
            } => {
                number(ui, "X", x);
                number(ui, "Y", y);
                number(ui, "Kesto (s)", duration);
                let current = gait.as_deref().unwrap_or("walk").to_owned();
                egui::ComboBox::from_label("Askellaji")
                    .selected_text(gait_label(&current))
                    .show_ui(ui, |ui| {
                        for name in motion_scene::GAITS {
                            if ui
                                .selectable_label(current == name, gait_label(name))
                                .clicked()
                            {
                                // Walking is the default and stays implicit in the source.
                                *gait = (name != "walk").then(|| name.into());
                            }
                        }
                    });
                let current = ease.as_deref().unwrap_or("linear").to_owned();
                egui::ComboBox::from_label("Vauhti")
                    .selected_text(ease_label(&current))
                    .show_ui(ui, |ui| {
                        for name in motion_scene::MOVE_EASES {
                            if ui
                                .selectable_label(current == name, ease_label(name))
                                .clicked()
                            {
                                // Constant speed is the default and stays implicit.
                                *ease = (name != "linear").then(|| name.into());
                            }
                        }
                    });
            }
            Event::Face { value, .. } => {
                egui::ComboBox::from_label("Suunta")
                    .selected_text(facing_label(value))
                    .show_ui(ui, |ui| {
                        for name in motion_scene::FACINGS {
                            ui.selectable_value(value, name.into(), facing_label(name));
                        }
                    });
            }
            Event::Emotion { value, .. } => emotion(ui, "Ilme", value),
            Event::Pose { value, .. } => {
                egui::ComboBox::from_label("Asento")
                    .selected_text(pose_label(value))
                    .show_ui(ui, |ui| {
                        for name in motion_script::POSES {
                            ui.selectable_value(value, name.into(), pose_label(name));
                        }
                    });
            }
            Event::Hold { hand, item, .. } => {
                egui::ComboBox::from_label("Käsi")
                    .selected_text(match hand {
                        motion_rig::HandSide::Left => "Vasen",
                        motion_rig::HandSide::Right => "Oikea",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(hand, motion_rig::HandSide::Left, "Vasen");
                        ui.selectable_value(hand, motion_rig::HandSide::Right, "Oikea");
                    });
                let items: Vec<String> = motion_rig::stick_studio_hand_props()
                    .map(|c| c.items.iter().map(|i| i.id.clone()).collect())
                    .unwrap_or_default();
                egui::ComboBox::from_label("Esine")
                    .selected_text(item.as_deref().unwrap_or("(laskee käsistä)"))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(item, None, "(laskee käsistä)");
                        for id in items {
                            ui.selectable_value(item, Some(id.clone()), id);
                        }
                    });
            }
            Event::Gesture {
                value, duration, ..
            } => {
                egui::ComboBox::from_label("Ele")
                    .selected_text(gesture_label(value))
                    .show_ui(ui, |ui| {
                        for name in motion_scene::GESTURES {
                            ui.selectable_value(value, name.into(), gesture_label(name));
                        }
                    });
                number(ui, "Kesto (s)", duration);
            }
            Event::Speak {
                text,
                emotion: style,
                audio,
                ..
            } => {
                ui.label("Vuorosana");
                ui.add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(4)
                        .char_limit(100_000),
                );
                let mut enabled = style.is_some();
                if ui
                    .checkbox(&mut enabled, "Repliikin ilmemerkintä")
                    .changed()
                {
                    *style = enabled.then(|| "neutral".into());
                }
                if let Some(style) = style {
                    emotion(ui, "Repliikin ilme", style);
                }
                ui.small("Ilmemerkintä ei ohjaa Kokoro-puheen äänen ilmaisua.");
                let mut imported = audio.is_some();
                if ui.checkbox(&mut imported, "Käytä omaa äänitettä").changed() {
                    *audio = imported.then(String::new);
                }
                if let Some(path) = audio {
                    ui.label("Äänitiedoston polku");
                    ui.add(egui::TextEdit::singleline(path).char_limit(4096));
                }
            }
        }
        ui.small("Kun käytät muutoksia, koko käsikirjoitus tarkistetaan. YAML-muotoilu yhtenäistetään ja kommentit poistetaan. Kumoa palauttaa edellisen lähteen.");
        ui.button("Käytä").clicked()
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
        .selected_text(expression_label(value))
        .show_ui(ui, |ui| {
            for expression in ["neutral", "happy", "sad", "surprised", "angry"] {
                ui.selectable_value(value, expression.into(), expression_label(expression));
            }
        });
}

pub(crate) fn expression_label(value: &str) -> &str {
    match value {
        "neutral" => "Neutraali",
        "happy" => "Iloinen",
        "sad" => "Surullinen",
        "surprised" => "Yllättynyt",
        "angry" => "Vihainen",
        _ => value,
    }
}

pub(crate) fn gesture_label(value: &str) -> &str {
    match value {
        "wave" => "Vilkutus",
        "point" => "Osoitus",
        "nod" => "Nyökkäys",
        "shrug" => "Olankohautus",
        "shake_head" => "Päänpudistus",
        "facepalm" => "Facepalm",
        "arms_crossed" => "Kädet puuskassa",
        "thumbs_up" => "Peukku ylös",
        "bow" => "Kumarrus",
        "jump" => "Hyppy",
        "clap" => "Taputus",
        "think" => "Mietintä",
        _ => value,
    }
}

pub(crate) fn pose_label(value: &str) -> &str {
    match value {
        "sit" => "Istuu",
        "stand" => "Seisoo",
        "crouch" => "Kyykky",
        "kneel" => "Polvillaan",
        "lie" => "Makaa",
        _ => value,
    }
}

pub(crate) fn gait_label(value: &str) -> &str {
    match value {
        "walk" => "Kävely",
        "jog" => "Hölkkä",
        "run" => "Juoksu",
        "stroll" => "Löntystely",
        "sneak" => "Hiipiminen",
        "tiptoe" => "Varpaillaan",
        "limp" => "Ontuminen",
        "sprint" => "Pyrähdys",
        "backwards" => "Takaperin",
        "leap" => "Hyppy",
        _ => value,
    }
}

pub(crate) fn ease_label(value: &str) -> &str {
    match value {
        "linear" => "Tasainen",
        "in" => "Kiihdyttää",
        "out" => "Jarruttaa",
        "in_out" => "Kiihdyttää ja jarruttaa",
        _ => value,
    }
}

pub(crate) fn facing_label(value: &str) -> &str {
    match value {
        "front" => "Edestä",
        "left" => "Profiili vasemmalle",
        "right" => "Profiili oikealle",
        "back" => "Selkä katsojaan",
        _ => value,
    }
}
