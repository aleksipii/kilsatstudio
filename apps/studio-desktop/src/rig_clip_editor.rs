//! Custom-rig clip library controls. Each button yields one validated source transaction.
use eframe::egui;
use motion_script::{RigClipPlacement, Script};
use std::collections::BTreeMap;

pub enum Action {
    Capture {
        name: String,
        replace: bool,
    },
    Apply {
        name: String,
        placement: RigClipPlacement,
    },
    Remove {
        name: String,
    },
}

pub struct State {
    name: String,
    replace: bool,
    selected: String,
    mapping: String,
    scale: f64,
    retime: bool,
    start: f64,
    duration: f64,
}
impl Default for State {
    fn default() -> Self {
        Self {
            name: String::new(),
            replace: false,
            selected: String::new(),
            mapping: String::new(),
            scale: 1.0,
            retime: false,
            start: 0.0,
            duration: 1.0,
        }
    }
}

/// `clip_joint=target_joint` per line; blank lines and `#` comments are ignored.
pub fn parse_mapping(text: &str) -> Result<BTreeMap<String, String>, String> {
    if text.len() > 16_384 {
        return Err("Joint mapping exceeds 16 KiB".into());
    }
    let mut map = BTreeMap::new();
    for line in text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let (from, to) = line
            .split_once('=')
            .ok_or("Use clip_joint=target_joint on each line")?;
        let (from, to) = (from.trim(), to.trim());
        if from.is_empty() || to.is_empty() {
            return Err("Mapping names must not be empty".into());
        }
        if map.insert(from.to_owned(), to.to_owned()).is_some() {
            return Err(format!("Clip joint {from} is mapped twice"));
        }
    }
    Ok(map)
}

impl State {
    pub fn placement(&self) -> Result<RigClipPlacement, String> {
        Ok(RigClipPlacement {
            joint_map: parse_mapping(&self.mapping)?,
            translation_scale: self.scale,
            interval: self.retime.then_some((self.start, self.duration)),
        })
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, script: &Script) -> Option<Action> {
        let mut action = None;
        ui.label("Rig clip library (copies joint tracks; later edits to a clip never change applied copies)");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.name)
                    .hint_text("clip name")
                    .desired_width(120.0),
            );
            ui.checkbox(&mut self.replace, "Replace existing");
            if ui.button("Capture joint tracks").clicked() {
                action = Some(Action::Capture {
                    name: self.name.clone(),
                    replace: self.replace,
                });
            }
        });
        if script.rig_clips.is_empty() {
            ui.small("No rig clips yet. Author joint keys, name a clip and capture it.");
            return action;
        }
        egui::ComboBox::from_id_salt("rig_clip_selected")
            .selected_text(if self.selected.is_empty() {
                "Select clip"
            } else {
                &self.selected
            })
            .show_ui(ui, |ui| {
                for name in script.rig_clips.keys() {
                    ui.selectable_value(&mut self.selected, name.clone(), name);
                }
            });
        let Some(clip) = script.rig_clips.get(&self.selected) else {
            return action;
        };
        let joints: Vec<_> = clip.joint_tracks.keys().cloned().collect();
        ui.small(format!(
            "{} s · joints: {}",
            clip.duration,
            joints.join(", ")
        ));
        ui.label(
            "Joint mapping (clip_joint=target_joint per line; unlisted joints keep their ids)",
        );
        ui.add(
            egui::TextEdit::multiline(&mut self.mapping)
                .desired_rows(2)
                .char_limit(16_384),
        );
        ui.add(
            egui::Slider::new(&mut self.scale, 0.01..=100.0)
                .logarithmic(true)
                .text("Translation scale"),
        );
        ui.checkbox(&mut self.retime, "Place in scene interval");
        if self.retime {
            ui.horizontal(|ui| {
                ui.label("Start (s)");
                ui.add(egui::DragValue::new(&mut self.start).speed(0.1));
                ui.label("Duration (s)");
                ui.add(egui::DragValue::new(&mut self.duration).speed(0.1));
            });
        }
        ui.horizontal(|ui| {
            if ui.button("Apply to selected actor").clicked() {
                action = Some(Action::Apply {
                    name: self.selected.clone(),
                    placement: RigClipPlacement::default(),
                });
            }
            if ui.button("Delete clip").clicked() {
                action = Some(Action::Remove {
                    name: self.selected.clone(),
                });
            }
        });
        if let Some(Action::Apply { placement, .. }) = &mut action {
            match self.placement() {
                Ok(next) => *placement = next,
                Err(error) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, &error);
                    return None;
                }
            }
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mapping_parses_trims_and_rejects_ambiguity() {
        let map = parse_mapping("# note\n arm = wing \n\nhead=top").unwrap();
        assert_eq!(map["arm"], "wing");
        assert_eq!(map["head"], "top");
        for bad in ["arm", "arm=", "=wing", "a=b\na=c"] {
            assert!(parse_mapping(bad).is_err(), "{bad}");
        }
        assert!(parse_mapping(&"x=y\n".repeat(5000)).is_err());
    }
    #[test]
    fn placement_reflects_controls_without_validation_side_effects() {
        let state = State {
            mapping: "arm=wing".into(),
            scale: 2.0,
            retime: true,
            start: 1.0,
            duration: 2.0,
            ..State::default()
        };
        let placement = state.placement().unwrap();
        assert_eq!(placement.interval, Some((1.0, 2.0)));
        assert_eq!(placement.translation_scale, 2.0);
        assert_eq!(placement.joint_map["arm"], "wing");
        let broken = State {
            mapping: "nonsense".into(),
            ..State::default()
        };
        assert!(broken.placement().is_err());
    }
}
