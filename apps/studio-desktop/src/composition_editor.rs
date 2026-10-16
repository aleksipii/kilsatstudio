//! Detached composition controls; Apply is validated as one screenplay edit.
use eframe::egui;
use motion_script::{BackgroundKey, CameraKey, Composition, Layer, Transition};

pub fn ui(
    ui: &mut egui::Ui,
    value: &mut Composition,
    actors: &[String],
    duration: f64,
    is_last_scene: bool,
) {
    outgoing_transition_ui(ui, &mut value.transition, is_last_scene);
    ui.separator();
    ui.label("Background color (#RRGGBB)");
    ui.add(egui::TextEdit::singleline(&mut value.background_color).char_limit(7));
    ui.separator();
    ui.label("Timed background colors");
    ui.small("The base color is used before the first key. Each key switches color at its exact time and holds until the next key.");
    let mut remove_background = None;
    for (index, key) in value.background_keys.iter_mut().enumerate() {
        ui.push_id(("background", index), |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Key {} · seconds", index + 1));
                ui.add(egui::DragValue::new(&mut key.at).speed(0.1));
                if ui.small_button("Remove key").clicked() {
                    remove_background = Some(index);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Color");
                ui.add(
                    egui::TextEdit::singleline(&mut key.color)
                        .char_limit(7)
                        .desired_width(85.0),
                );
                if let Ok(color) = egui::Color32::from_hex(&key.color) {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, color);
                }
            });
        });
    }
    if let Some(index) = remove_background {
        value.background_keys.remove(index);
    }
    // Do not create duplicate times when the last key is already at scene end.
    let next_at = value.background_keys.last().map_or(Some(0.0), |key| {
        if key.at.is_finite() && key.at < duration {
            let candidate = (key.at + 1.0).min(duration);
            (candidate > key.at).then_some(candidate)
        } else {
            None
        }
    });
    if ui
        .add_enabled(
            value.background_keys.len() < 4096 && next_at.is_some(),
            egui::Button::new("Add background key"),
        )
        .clicked()
    {
        let color = value
            .background_keys
            .last()
            .map(|key| key.color.clone())
            .unwrap_or_else(|| value.background_color.clone());
        value.background_keys.push(BackgroundKey {
            at: next_at.unwrap(),
            color,
        });
    }
    ui.small("Use #RRGGBB and unique ascending times from 0 to scene duration. Apply settings commits all background edits together; Undo restores the previous screenplay.");
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Fade in / out (seconds)");
        ui.add(egui::DragValue::new(&mut value.fade_in).speed(0.1));
        ui.add(egui::DragValue::new(&mut value.fade_out).speed(0.1));
    });
    ui.small("Camera offsets are pixels from center. Key times remain in seconds.");
    let mut remove = None;
    for (index, key) in value.camera.iter_mut().enumerate() {
        ui.push_id(index, |ui| {
            ui.horizontal(|ui| {
                ui.label("Time");
                ui.add(egui::DragValue::new(&mut key.at).speed(0.1));
                if ui.small_button("Remove key").clicked() {
                    remove = Some(index);
                }
            });
            ui.horizontal(|ui| {
                ui.label("X");
                ui.add(egui::DragValue::new(&mut key.x));
                ui.label("Y");
                ui.add(egui::DragValue::new(&mut key.y));
                ui.label("Zoom");
                ui.add(egui::DragValue::new(&mut key.zoom).speed(0.05));
            });
        });
    }
    if let Some(index) = remove {
        value.camera.remove(index);
    }
    if value.camera.len() < 1024 && ui.button("Add camera key").clicked() {
        let at = value
            .camera
            .last()
            .map(|k| (k.at + 1.0).min(duration))
            .unwrap_or(0.0);
        value.camera.push(CameraKey {
            at,
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        });
    }
    ui.small("Keys must have unique ascending times; reorder by editing time before Apply.");
    for actor in actors {
        let mut layer = value.layers.get(actor).cloned().unwrap_or(Layer {
            z: 0,
            visible: true,
        });
        let mut changed = false;
        ui.horizontal(|ui| {
            changed |= ui.checkbox(&mut layer.visible, actor).changed();
            ui.label("Layer");
            changed |= ui.add(egui::DragValue::new(&mut layer.z)).changed();
        });
        if changed {
            value.layers.insert(actor.clone(), layer);
        }
    }
}

fn outgoing_transition_ui(ui: &mut egui::Ui, transition: &mut Transition, is_last_scene: bool) {
    ui.label("Outgoing transition to next scene");
    let crossfade = matches!(transition, Transition::Crossfade { .. });
    ui.horizontal(|ui| {
        if ui.selectable_label(!crossfade, "Cut").clicked() {
            *transition = Transition::Cut;
        }
        if ui
            .add_enabled(
                !is_last_scene,
                egui::Button::new("Crossfade").selected(crossfade),
            )
            .clicked()
            && !crossfade
        {
            *transition = Transition::Crossfade { duration: 0.5 };
        }
    });
    if let Transition::Crossfade { duration } = transition {
        ui.horizontal(|ui| {
            ui.label("Overlap (seconds)");
            ui.add(egui::DragValue::new(duration).speed(0.05));
        });
    }
    if is_last_scene {
        ui.small("The final scene has no next scene and must use Cut. Apply rejects a final-scene crossfade.");
    } else {
        ui.small("Crossfade overlaps this scene's end with the next scene's start. Use a positive duration shorter than both scenes; incoming and outgoing overlaps may not overlap each other.");
    }
    ui.small("Outgoing transitions appear in Whole project preview and PNG/MP4 sequence export. Selected-scene preview and exports remain scene-local.");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drawing_controls_does_not_create_layer_overrides() {
        let mut value = Composition::default();
        let before = serde_json::to_value(&value).unwrap();
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |panel| {
                ui(panel, &mut value, &["EMMA".into()], 5.0, true);
            });
        });
        assert_eq!(serde_json::to_value(&value).unwrap(), before);
    }
}
