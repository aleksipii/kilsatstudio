//! Explicit, background-only installed system voice discovery for private preview.
use eframe::egui;
use motion_tts::{CancellationToken, SystemVoice};
use std::{sync::mpsc, time::Duration};
#[derive(Default)]
pub struct Inventory {
    receiver: Option<mpsc::Receiver<Result<Vec<SystemVoice>, String>>>,
    cancel: Option<CancellationToken>,
    voices: Vec<SystemVoice>,
    status: String,
}
impl Drop for Inventory {
    fn drop(&mut self) {
        if let Some(token) = &self.cancel {
            token.cancel();
        }
    }
}
impl Inventory {
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<String> {
        if let Some(rx) = &self.receiver {
            match rx.try_recv() {
                Ok(result) => {
                    self.receiver = None;
                    self.cancel = None;
                    match result {
                        Ok(voices) => {
                            self.status =
                                format!("Löytyi {} asennettua suomenkielistä ääntä.", voices.len());
                            self.voices = voices;
                        }
                        Err(e) => self.status = e,
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.cancel = None;
                    self.status = "Äänten haku keskeytyi.".into();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let mut selected = None;
        egui::CollapsingHeader::new("Macin järjestelmä-äänet · vain yksityinen esikatselu").show(ui,|ui|{
   ui.weak("Järjestelmä-ääniä ei hyväksytä sarjan julkaisuääniksi. Tuotantoon tarvitaan erikseen tarkistettu Piper-ääni tai oma WAV-äänite.");
   if ui.add_enabled(self.receiver.is_none(),egui::Button::new("Hae asennetut suomenkieliset äänet")).clicked(){let(tx,rx)=mpsc::sync_channel(1);let cancel=CancellationToken::new();let worker_cancel=cancel.clone();std::thread::spawn(move||{let result=motion_tts::installed_system_voices(&worker_cancel).map_err(|e|e.to_string());let _=tx.send(result);});self.receiver=Some(rx);self.cancel=Some(cancel);}
   for voice in &self.voices{if ui.button(format!("{} → hahmon esikatseluääneksi",voice.preset)).clicked(){selected=Some(voice.preset.clone());}}
   ui.label(&self.status);
  });
        if self.receiver.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }
        selected
    }
}
