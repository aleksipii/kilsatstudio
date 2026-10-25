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
                        Err(e) => {
                            self.status = format!("Asennettujen äänten haku epäonnistui: {e}")
                        }
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
   for voice in &self.voices{if ui.button(format!("Käytä ääntä {} esikatselussa",voice.preset)).clicked(){selected=Some(voice.preset.clone());}}
   ui.label(&self.status);
  });
        if self.receiver.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }
        selected
    }
}

#[derive(Clone)]
struct ProfileFields {
    executable: String,
    model: String,
    config: String,
    destination: String,
    preset: String,
}
impl Default for ProfileFields {
    fn default() -> Self {
        Self {
            executable: String::new(),
            model: String::new(),
            config: String::new(),
            destination: String::new(),
            preset: "piper_harri".into(),
        }
    }
}
#[derive(Default)]
pub struct ProfileEditor {
    fields: ProfileFields,
    receiver: Option<mpsc::Receiver<Result<std::path::PathBuf, String>>>,
    status: String,
}
fn save_profile(fields: ProfileFields) -> Result<std::path::PathBuf, String> {
    use std::{io::Write, path::PathBuf};
    let executable = PathBuf::from(fields.executable.trim());
    let model = PathBuf::from(fields.model.trim());
    let config = PathBuf::from(fields.config.trim());
    let destination = PathBuf::from(fields.destination.trim());
    for (path, label, limit) in [
        (&executable, "Piper-ohjelma", 64 * 1024 * 1024),
        (&model, "ONNX-malli", 512 * 1024 * 1024),
        (&config, "mallin asetukset", 1024 * 1024),
    ] {
        let metadata = std::fs::symlink_metadata(path).map_err(|e| {
            format!("{label}: tiedostoa ei voitu lukea. Tarkista koko polku ja käyttöoikeudet. Tekninen virhe: {e}")
        })?;
        if !path.is_absolute()
            || !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() == 0
            || metadata.len() > limit
        {
            return Err(format!(
                "{label}: anna olemassa olevan tavallisen tiedoston koko polku."
            ));
        }
    }
    if !destination.is_absolute()
        || destination.extension().and_then(|s| s.to_str()) != Some("json")
        || !destination.parent().is_some_and(|p| p.is_dir())
    {
        return Err("Anna uuden JSON-tiedoston koko polku olemassa olevaan kansioon.".into());
    }
    if !fields.preset.starts_with("piper_")
        || fields.preset.len() > 128
        || !fields
            .preset
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        return Err("Piper-äänen tunnuksen on alettava piper_.".into());
    }
    let profile = motion_tts::PiperConfig {
        executable,
        model_path: model,
        config_path: config,
        cache_directory: destination.parent().unwrap().join("piper-cache"),
        preset: fields.preset,
        model_version: "operator-selected; exact model/config hashes checked at inference".into(),
        environment_identity:
            "operator-selected local runtime; dependency closure unverified; request reuse disabled"
                .into(),
        timeout_seconds: 60,
    };
    let bytes = serde_json::to_vec_pretty(&profile)
        .map_err(|e| format!("Profiilin JSON-asetuksia ei voitu muodostaa: {e}"))?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .map_err(|e| format!("Asetustiedostoa ei tallennettu: {e}. Käytä uutta tiedostonimeä."))?;
    if let Err(error) = file.write_all(&bytes) {
        drop(file);
        let _ = std::fs::remove_file(&destination);
        return Err(format!(
            "Piper-profiilin kirjoittaminen epäonnistui: {error}"
        ));
    }
    Ok(destination)
}
impl ProfileEditor {
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<std::path::PathBuf> {
        let mut saved = None;
        if let Some(rx) = &self.receiver {
            match rx.try_recv() {
                Ok(result) => {
                    self.receiver = None;
                    match result {
                        Ok(path) => {
                            self.status="Piper-profiili tallennettu. Valitse hahmolle Piper-ääni ja valmistele puhe; ensimmäinen synteesi tarkistaa mallin.".into();
                            saved = Some(path);
                        }
                        Err(error) => self.status = error,
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.status = "Profiilin tallennustyö keskeytyi.".into();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        egui::CollapsingHeader::new("Luo Piper-profiili asennetuista tiedostoista").show(ui,|ui|{
   ui.weak("Valitse erikseen asennettu Piper sekä ONNX-malli ja sen JSON-asetukset. Tämä ei lataa tiedostoja eikä myönnä julkaisuoikeuksia. Vanhaa profiilia ei korvata.");
   ui.add_enabled_ui(self.receiver.is_none(),|ui|{
    for (label,value) in [("Piper-ohjelman koko polku",&mut self.fields.executable),("ONNX-mallin koko polku",&mut self.fields.model),("Mallin JSON-asetusten koko polku",&mut self.fields.config),("Uuden profiilin koko polku (.json)",&mut self.fields.destination),("Äänen tunnus",&mut self.fields.preset)] {ui.label(label);ui.text_edit_singleline(value);}
    if ui.button("Tallenna uusi Piper-profiili").clicked(){let(tx,rx)=mpsc::sync_channel(1);let fields=self.fields.clone();std::thread::spawn(move||{let _=tx.send(save_profile(fields));});self.receiver=Some(rx);}
   });ui.label(&self.status);
  });
        if self.receiver.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }
        saved
    }
}
#[cfg(test)]
mod profile_tests {
    use super::*;
    #[test]
    fn saving_a_profile_does_not_run_piper_or_overwrite_an_existing_file() {
        let root = std::env::temp_dir().join(format!(
            "piper-profile-ui-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        for name in ["piper", "voice.onnx", "voice.json"] {
            std::fs::write(root.join(name), b"fixture-only").unwrap();
        }
        let fields = ProfileFields {
            executable: root.join("piper").display().to_string(),
            model: root.join("voice.onnx").display().to_string(),
            config: root.join("voice.json").display().to_string(),
            destination: root.join("profile.json").display().to_string(),
            preset: "piper_harri".into(),
        };
        let output = save_profile(fields.clone()).unwrap();
        let bytes = std::fs::read(&output).unwrap();
        assert!(save_profile(fields).is_err());
        assert_eq!(std::fs::read(&output).unwrap(), bytes);
        assert!(!root.join("piper-cache").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
