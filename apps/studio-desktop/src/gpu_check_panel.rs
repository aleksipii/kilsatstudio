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
                    self.status=format!("Metal-testikuva läpäisi tarkistuksen. Tulos koskee tätä laitetestiä; koko kohtauksen CPU/GPU-vastaavuus on vielä varmistamatta.\n{}",e.report);
                }
                Err(e) => self.status = format!("Metal-tarkistus: {e}"),
            }
        }
        ui.label("Metal-laitetarkistus käyttää paikallista testikuvaa. Mallia ei ladata. Koko kohtaus renderöidään edelleen suorittimella (CPU).");
        let helper = gpu_check_worker::bundled_helper();
        if let Err(e) = &helper {
            ui.label(format!("Metal-apuohjelma ei ole käytettävissä: {e}"));
        }
        ui.horizontal(|ui| {
            ui.label("Uuden tuloskansion koko polku");
            ui.add_enabled(
                !self.worker.busy(),
                egui::TextEdit::singleline(&mut self.output),
            );
        });
        if ui
            .add_enabled(
                helper.is_ok() && !self.worker.busy(),
                egui::Button::new("Tarkista Metal-laite"),
            )
            .clicked()
        {
            if let Ok(helper) = helper {
                self.status = match self.worker.start(helper, self.output.trim().into()) {
                    Ok(()) => "Suoritetaan Metal-laitetarkistusta…".into(),
                    Err(e) => format!("Metal-tarkistus: {e}"),
                };
            }
        }
        if self.worker.busy() {
            if ui.button("Peruuta Metal-tarkistus").clicked() {
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
