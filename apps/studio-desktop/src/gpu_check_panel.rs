//! Hardware evidence collection is explicit and does not imply full renderer parity.
use crate::gpu_check_worker::{self, Worker};
use eframe::egui;
#[derive(Default)]
pub struct Panel {
    worker: Worker,
    output: String,
    status: String,
    texture: Option<egui::TextureHandle>,
}
impl Panel {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if let Some(result) = self.worker.poll() {
            match result {
                Ok(e) => {
                    self.texture = Some(ui.ctx().load_texture(
                        "metal-fixture",
                        egui::ColorImage::from_rgba_unmultiplied([256, 256], &e.rgba),
                        egui::TextureOptions::LINEAR,
                    ));
                    self.status=format!("Metal fixture passed. This is device evidence only; full scene parity remains open.\n{}",e.report);
                }
                Err(e) => self.status = e,
            }
        }
        ui.label("Explicit Metal hardware fixture; no model downloads. Full scene rendering still uses CPU.");
        let helper = gpu_check_worker::bundled_helper();
        if let Err(e) = &helper {
            ui.label(e);
        }
        ui.horizontal(|ui| {
            ui.label("New absolute output directory");
            ui.add_enabled(
                !self.worker.busy(),
                egui::TextEdit::singleline(&mut self.output),
            );
        });
        if ui
            .add_enabled(
                helper.is_ok() && !self.worker.busy(),
                egui::Button::new("Run Metal device check"),
            )
            .clicked()
        {
            if let Ok(helper) = helper {
                self.status = match self.worker.start(helper, self.output.trim().into()) {
                    Ok(()) => "Running isolated Metal fixture…".into(),
                    Err(e) => e,
                };
            }
        }
        if self.worker.busy() {
            if ui.button("Cancel Metal check").clicked() {
                self.worker.cancel();
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
        if let Some(texture) = &self.texture {
            ui.image((texture.id(), egui::vec2(256., 256.)));
        }
        egui::ScrollArea::vertical()
            .max_height(160.)
            .show(ui, |ui| {
                ui.label(&self.status);
            });
    }
}
