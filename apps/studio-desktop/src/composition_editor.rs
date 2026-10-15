//! Detached composition controls; Apply is validated as one screenplay edit.
use eframe::egui;
use motion_script::{CameraKey, Composition, Layer};

pub fn ui(ui: &mut egui::Ui, value: &mut Composition, actors: &[String], duration: f64) {
    ui.label("Background color (#RRGGBB)");
    ui.add(egui::TextEdit::singleline(&mut value.background_color).char_limit(7));
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
                ui(panel, &mut value, &["EMMA".into()], 5.0);
            });
        });
        assert_eq!(serde_json::to_value(&value).unwrap(), before);
    }
}
