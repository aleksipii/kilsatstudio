//! Detached character voice settings, committed only through an explicit Apply.
use eframe::egui;
use motion_script::{Script, VoiceConfig};

pub struct Draft {
    pub actor: String,
    pub voice: Option<VoiceConfig>,
}

impl Draft {
    pub fn new(script: &Script, actor: &str) -> Option<Self> {
        Some(Self {
            actor: actor.to_owned(),
            voice: script.characters.get(actor)?.voice.clone(),
        })
    }

    /// Request application; the caller validates and commits the document.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        ui.label(format!("Hahmon ääni: {}", self.actor));
        let mut enabled = self.voice.is_some();
        if ui
            .checkbox(&mut enabled, "Käytä paikallista puheääntä")
            .changed()
        {
            self.voice = enabled.then(default_voice);
        }
        if let Some(voice) = &mut self.voice {
            let before = voice.provider.clone();
            egui::ComboBox::from_label("Puhepalvelu")
                .selected_text(&voice.provider)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut voice.provider, "kokoro".into(), "Kokoro · englanti");
                    ui.selectable_value(&mut voice.provider, "piper".into(), "Piper · suomi");
                    ui.selectable_value(
                        &mut voice.provider,
                        "system".into(),
                        "macOS · yksityinen esikatselu",
                    );
                });
            if before != voice.provider {
                match voice.provider.as_str() {
                    "piper" => {
                        voice.language = "fi-fi".into();
                        voice.preset = "piper_harri".into();
                    }
                    "system" => {
                        voice.language = "fi-fi".into();
                        voice.preset.clear();
                    }
                    _ => {
                        voice.language = "en-us".into();
                        voice.preset = "af_heart".into();
                    }
                }
            }
            ui.label("Äänen tunnus (asennettava erikseen)");
            ui.add(egui::TextEdit::singleline(&mut voice.preset).char_limit(128));
            if voice.provider == "kokoro" {
                ui.horizontal(|ui| {
                    for preset in ["af_heart", "am_michael"] {
                        if ui.button(preset).clicked() {
                            voice.preset = preset.into();
                        }
                    }
                });
                egui::ComboBox::from_label("Kieli")
                    .selected_text(&voice.language)
                    .show_ui(ui, |ui| {
                        for language in ["en-us", "en-gb"] {
                            ui.selectable_value(&mut voice.language, language.into(), language);
                        }
                    });
            } else {
                ui.label("Kieli: suomi (fi-fi)");
                if voice.provider == "system" {
                    ui.colored_label(egui::Color32::YELLOW,"Vain yksityinen esikatselu. Valitse asennettu ääni Puhe-välilehden hakutoiminnolla.");
                } else {
                    ui.weak("Anna Piper-asetustiedoston polku Puhe-välilehdellä. Sovellus ei lataa mallia eikä myönnä julkaisuoikeuksia.");
                }
            }
            ui.horizontal(|ui| {
                ui.label("Puheen nopeus");
                ui.add(
                    egui::DragValue::new(&mut voice.speed)
                        .range(0.5..=2.0)
                        .speed(0.05),
                );
            });
            ui.small("The preset asset must already be installed locally. This editor does not download models or voices.");
            ui.small("Expression metadata does not condition Kokoro speech.");
        } else {
            ui.small("Disabling voice is valid only when all this character's dialogue lines use imported audio.");
        }
        ui.small("Apply validates the entire screenplay. YAML formatting is normalized and comments are removed. Undo restores the previous source.");
        ui.button("Käytä ääniasetuksia").clicked()
    }
}

fn default_voice() -> VoiceConfig {
    VoiceConfig {
        provider: "kokoro".into(),
        preset: "af_heart".into(),
        language: "en-us".into(),
        speed: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script() -> Script {
        motion_script::parse(
            "characters:\n  EMMA:\n    asset: characters/emma\n    voice:\n      provider: kokoro\n      preset: af_heart\n      language: en-us\n      speed: 1.0\nscenes: []\n",
        )
        .expect("valid voice fixture")
    }

    #[test]
    fn editing_or_disabling_draft_preserves_source_voice() {
        let script = script();
        let mut draft = Draft::new(&script, "EMMA").unwrap();
        draft.voice.as_mut().unwrap().preset = "am_michael".into();
        assert_eq!(
            script.characters["EMMA"].voice.as_ref().unwrap().preset,
            "af_heart"
        );
        draft.voice = None;
        assert!(script.characters["EMMA"].voice.is_some());
        assert!(Draft::new(&script, "MISSING").is_none());
    }

    #[test]
    fn character_without_voice_remains_disabled_in_draft() {
        let mut script = script();
        script.characters.get_mut("EMMA").unwrap().voice = None;
        assert!(Draft::new(&script, "EMMA").unwrap().voice.is_none());
    }
}
