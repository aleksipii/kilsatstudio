//! Explicit opt-in estimated alignment of an already prepared dialogue WAV.
use crate::alignment_worker::{Action, PreparedAudio, Request, Worker};
pub use crate::alignment_worker::{Identity, Ready};
use eframe::egui;
use motion_lipsync::rhubarb::Recognizer;
use motion_tts::CancellationToken;
use std::{path::PathBuf, sync::Arc};

struct TimingDraft {
    original: motion_lipsync::rhubarb::EstimatedVisemeAlignment,
    cues: Vec<motion_lipsync::rhubarb::MouthCue>,
}

pub struct Panel {
    worker: Worker,
    executable: String,
    cache_key: String,
    package_version: String,
    pending_action: Option<Action>,
    recognizer: Recognizer,
    active: Option<Identity>,
    candidate: Option<Ready>,
    ready: Option<Ready>,
    status: String,
    timing_draft: Option<TimingDraft>,
}
impl Panel {
    pub fn new() -> Self {
        Self {
            worker: Worker::new(),
            executable: String::new(),
            cache_key: String::new(),
            package_version: String::new(),
            pending_action: None,
            recognizer: Recognizer::PocketSphinx,
            active: None,
            candidate: None,
            ready: None,
            status: String::new(),
            timing_draft: None,
        }
    }
    /// Returns true only on an explicit Estimate, Load or Review click. The caller constructs the
    /// identity/audio request then; painting performs no file access or hashing.
    pub fn ui(&mut self, ui: &mut egui::Ui, can_run: bool) -> bool {
        let mut run = false;
        let mut settings_changed = false;
        ui.collapsing("Paikallinen suun ajoitusarvio (Rhubarb)", |ui| {
            ui.label("Arviointi lukee valmistellun repliikin WAV-äänen. Tallennettu ajoitus sidotaan täsmälliseen ääneen ja tekstiin. Suumuodot ovat arvioita, eivät mitattuja äännekohdistuksia. Tiedostoja ei ladata eikä puhetta tuoteta.");
            ui.add_enabled_ui(!self.busy(), |ui| {
                ui.horizontal(|ui| { ui.label("Ohjelmatiedosto"); settings_changed |= ui.add(egui::TextEdit::singleline(&mut self.executable).char_limit(4096).hint_text("/absolute/path/to/rhubarb")).changed(); });
                egui::ComboBox::from_id_salt("native-rhubarb-recognizer").selected_text(match self.recognizer { Recognizer::PocketSphinx=>"PocketSphinx (englanti)", Recognizer::Phonetic=>"Phonetic (äännepohjainen)" }).show_ui(ui, |ui| {
                    settings_changed |= ui.selectable_value(&mut self.recognizer,Recognizer::PocketSphinx,"PocketSphinx (englanti)").changed();
                    settings_changed |= ui.selectable_value(&mut self.recognizer,Recognizer::Phonetic,"Phonetic (äännepohjainen)").changed();
                });
            });
            ui.add_enabled_ui(!self.busy(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Tallennetun ajoituksen tunnus");
                    settings_changed |= ui.add(egui::TextEdit::singleline(&mut self.cache_key).char_limit(64).hint_text("64 merkin välimuistitunnus")).changed();
                });
            });
            ui.add_enabled_ui(!self.busy(), |ui| {
                ui.label("Valinnainen paketin rekisteröinti sitoo jokaisen tiedoston tiivisteen. Käytä vain luotettua, itsenäistä Phonetic-pakettia. Rekisteröinti ei tarkista lisenssejä tai laatua eikä eristä tiedostojärjestelmää.");
                settings_changed |= ui.add(egui::TextEdit::singleline(&mut self.package_version).char_limit(1024).hint_text("Paketin odotettu --version-tuloste täsmälleen")).changed();
                if ui.add_enabled(can_run && self.recognizer == Recognizer::Phonetic && !self.package_version.trim().is_empty(), egui::Button::new("Luota, rekisteröi ja arvioi paketti")).clicked() {
                    self.pending_action = Some(Action::EnrollAndEstimate { expected_version: self.package_version.clone() });
                    run = true;
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(can_run && !self.busy(), egui::Button::new("Tee uusi arvio")).on_hover_text("Käynnistä tunnistin uudelleen käyttämättä aiempaa ajoitusarviota.").clicked() {
                    self.pending_action = Some(Action::EstimateFresh); run = true;
                }
                if ui.add_enabled(can_run && !self.busy(),egui::Button::new("Arvioi valmis repliikki")).clicked() {
                    self.pending_action = Some(Action::Estimate); run = true;
                }
                if ui.add_enabled(can_run && !self.busy() && self.cache_key.len() == 64, egui::Button::new("Lataa tallennettu ajoitus")).clicked() {
                    self.pending_action = Some(Action::Load { key: self.cache_key.trim().to_owned() });
                    run = true;
                }
                if ui.add_enabled(self.busy(),egui::Button::new("Peru kohdistus")).clicked() { self.cancel(); }
            });
            if !can_run { ui.label("Valmistele ensin valitun repliikin ääni."); }
            if self.busy() { ui.spinner(); ui.ctx().request_repaint_after(std::time::Duration::from_millis(40)); }
            if !self.status.is_empty() { ui.label(&self.status); }
            let draft_changed = self.timing_draft.as_ref().is_some_and(|draft| draft.cues != draft.original.cues);
            if let Some(candidate)=&self.candidate {
                ui.label(match candidate.cache.origin {
                    crate::alignment_worker::ResultOrigin::ExplicitArchiveLoad => "Tallennettu ajoitusarvio. Tunnistinta ei suoritettu.",
                    crate::alignment_worker::ResultOrigin::AutomaticReuse => "Täsmälleen vastaava ajoitusarvio käytettiin uudelleen. Tunnistinta tai versiokyselyä ei käynnistetty.",
                    crate::alignment_worker::ResultOrigin::Fresh => "Uusi paikallinen ajoitusarvio.",
                    crate::alignment_worker::ResultOrigin::Manual => "Käsin muokatut suumuodot. Tunnistinta ei suoritettu.",
                });
                if let Some(key) = &candidate.cache.key {
                    ui.horizontal(|ui| {
                        ui.label(format!("Välimuisti: {key}"));
                        if ui.button("Kopioi tunnus").clicked() { ui.ctx().copy_text(key.clone()); }
                    });
                }
                if !candidate.cache.message.is_empty() { ui.label(format!("Välimuistin tekniset tiedot: {}", candidate.cache.message)); }
                ui.label(format!("Ajoituksen alkuperä: {}", candidate.cache.provenance));
                ui.label(format!("Ajoitus valmis repliikille {} ({} tavua). Käytä ajoitusta saman äänen valmisteluun uudelleen.",candidate.identity.line_id,candidate.json.len()));
                if draft_changed { ui.label("Tarkista alla olevat käsin tehdyt muutokset ennen niiden käyttöönottoa."); }
                if ui.add_enabled(can_run && !self.busy() && !draft_changed,egui::Button::new("Käytä tarkistettua ajoitusta")).clicked() { self.ready=self.candidate.take(); self.timing_draft=None; }
            }
            run |= self.timing_editor(ui, can_run);
        });
        if settings_changed {
            let action = if run {
                self.pending_action.take()
            } else {
                None
            };
            self.invalidate();
            self.pending_action = action;
        }
        run
    }
    fn timing_editor(&mut self, ui: &mut egui::Ui, can_run: bool) -> bool {
        let busy = self.busy();
        let Some(draft) = self.timing_draft.as_mut() else {
            return false;
        };
        let mut submit = false;
        let mut action = None;
        ui.collapsing("Muokkaa suun ajoitusta ääninäytteinä", |ui| {
            ui.label(format!(
                "{} Hz; {} näytettä. Välit ovat [alku, loppu); välien aukot ovat lepoa. Muodot A–H/X kuvaavat suuta, eivät mitattuja äänteitä.",
                draft.original.sample_rate, draft.original.frame_count
            ));
            ui.add_enabled_ui(!busy, |ui| {
                let mut remove = None;
                let mut split = None;
                egui::ScrollArea::vertical()
                    .id_salt("mouth-timing-cue-table")
                    .max_height(260.0)
                    .show_rows(ui, 26.0, draft.cues.len(), |ui, rows| {
                        for index in rows {
                            let cue = &mut draft.cues[index];
                            ui.push_id(index, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{}", index + 1));
                                    ui.add(egui::DragValue::new(&mut cue.start_sample).range(0.0..=draft.original.frame_count as f64).prefix("Alku "));
                                    ui.add(egui::DragValue::new(&mut cue.end_sample).range(0.0..=draft.original.frame_count as f64).prefix("Loppu "));
                                    egui::ComboBox::from_id_salt("shape")
                                        .selected_text(cue.shape.to_string())
                                        .show_ui(ui, |ui| {
                                            for shape in ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'X'] {
                                                ui.selectable_value(&mut cue.shape, shape, shape.to_string());
                                            }
                                        });
                                    if ui.button("Jaa").clicked() { split = Some(index); }
                                    if ui.button("Poista").clicked() { remove = Some(index); }
                                });
                            });
                        }
                    });
                if let Some(index) = remove {
                    draft.cues.remove(index);
                } else if let Some(index) = split {
                    let cue = &draft.cues[index];
                    if draft.cues.len() < motion_lipsync::manual::MAX_EDIT_CUES
                        && cue.end_sample.saturating_sub(cue.start_sample) >= 2
                    {
                        let midpoint = cue.start_sample + (cue.end_sample - cue.start_sample) / 2;
                        let right = motion_lipsync::rhubarb::MouthCue {
                            start_sample: midpoint,
                            end_sample: cue.end_sample,
                            shape: cue.shape,
                        };
                        draft.cues[index].end_sample = midpoint;
                        draft.cues.insert(index + 1, right);
                    }
                }
                ui.horizontal(|ui| {
                    let start = draft.cues.last().map_or(0, |cue| cue.end_sample);
                    if ui.add_enabled(draft.cues.len() < motion_lipsync::manual::MAX_EDIT_CUES && start < draft.original.frame_count, egui::Button::new("Lisää lepojakso")).clicked() {
                        draft.cues.push(motion_lipsync::rhubarb::MouthCue {
                            start_sample: start,
                            end_sample: draft.original.frame_count,
                            shape: 'X',
                        });
                    }
                    if ui.button("Palauta arvio").clicked() {
                        draft.cues.clone_from(&draft.original.cues);
                    }
                });
                let valid = motion_lipsync::manual::validate_cues(&draft.cues, draft.original.frame_count);
                if let Err(error) = &valid { ui.colored_label(egui::Color32::YELLOW, format!("Ajoitusta ei voi käyttää: {error}")); }
                if ui.add_enabled(can_run && valid.is_ok(), egui::Button::new("Tarkista käsin tehdyt muutokset")).clicked() {
                    action = Some(Action::Manual {
                        original: Box::new(draft.original.clone()),
                        cues: draft.cues.clone(),
                    });
                    submit = true;
                }
            });
        });
        if action.is_some() {
            self.pending_action = action;
        }
        submit
    }
    pub fn start(
        &mut self,
        identity: Identity,
        audio: Arc<dyn PreparedAudio>,
        scene_alignments: String,
        cache_root: PathBuf,
    ) -> Result<(), String> {
        let action = self
            .pending_action
            .take()
            .ok_or("Valitse ensin arviointi tai tallennetun ajoituksen lataus.")?;
        let loading = matches!(action, Action::Load { .. });
        let manual = matches!(action, Action::Manual { .. });
        let request = Request {
            cache_root,
            action,
            identity: identity.clone(),
            audio,
            scene_alignments,
            executable: PathBuf::from(self.executable.trim()),
            recognizer: self.recognizer,
            cancel: CancellationToken::new(),
        };
        match self.worker.submit(request) {
            Ok(()) => {
                self.active = Some(identity);
                self.candidate = None;
                self.timing_draft = None;
                self.ready = None;
                self.status = if loading {
                    "Ladataan tallennettua ajoitusarviota…"
                } else if manual {
                    "Tarkistetaan käsin tehtyä suun ajoitusta valmisteltua ääntä vasten…"
                } else {
                    "Arvioidaan valmistellun äänen ajoitusta paikallisesti…"
                }
                .into();
                Ok(())
            }
            Err(e) => {
                self.status = format!("Kohdistuksen käynnistys epäonnistui: {e}");
                Err(e)
            }
        }
    }
    /// Called every frame with the identity of the currently eligible prepared
    /// line. Source generation, selection, transcript or audio-path changes
    /// invalidate both queued completions and un-applied results.
    pub fn tick(&mut self, current: Option<&Identity>) {
        if self.active.as_ref().is_some_and(|id| Some(id) != current)
            || self
                .candidate
                .as_ref()
                .is_some_and(|r| Some(&r.identity) != current)
            || self
                .ready
                .as_ref()
                .is_some_and(|r| Some(&r.identity) != current)
        {
            self.invalidate();
        }
        if let Some(update) = self.worker.poll() {
            if self.active.as_ref() != Some(&update.identity) || current != Some(&update.identity) {
                return;
            }
            self.active = None;
            match update.result {
                Ok(ready) => {
                    self.timing_draft = serde_json::from_str::<
                        motion_lipsync::rhubarb::EstimatedVisemeAlignment,
                    >(&ready.json)
                    .ok()
                    .filter(|value| value.cues.len() <= motion_lipsync::manual::MAX_EDIT_CUES)
                    .map(|original| TimingDraft {
                        cues: original.cues.clone(),
                        original,
                    });
                    if let Some(key) = &ready.cache.key {
                        self.cache_key = key.clone();
                    }
                    self.status = if ready.cache.origin == crate::alignment_worker::ResultOrigin::Manual {
                        "Käsin tehty ajoitus on tarkistettu täsmällistä ääntä ja tekstiä vasten. Ota se käyttöön erikseen. Huulisynkan laatua ei ole mitattu."
                    } else if ready.cache.origin
                        == crate::alignment_worker::ResultOrigin::ExplicitArchiveLoad
                    {
                        "Tallennettu ajoitusarvio ladattu. Tarkista se ja ota käyttöön erikseen."
                    } else {
                        "Arviointi valmis. Tarkista ajoitus ja ota se käyttöön erikseen."
                    }
                    .into();
                    self.candidate = Some(ready);
                }
                Err(e) => self.status = format!("Suun ajoituksen käsittely epäonnistui: {e}"),
            }
        }
    }
    pub fn take_ready(&mut self) -> Option<Ready> {
        self.ready.take()
    }
    pub fn busy(&self) -> bool {
        self.worker.busy()
    }
    pub fn generation(&self) -> Option<u64> {
        self.active.as_ref().map(|i| i.generation)
    }
    pub fn cancel(&mut self) {
        self.worker.cancel();
        self.active = None;
        self.candidate = None;
        self.ready = None;
        self.pending_action = None;
        self.timing_draft = None;
        self.status = "Kohdistus peruttu. Odotetaan paikallisen ohjelman pysähtymistä.".into();
    }
    pub fn invalidate(&mut self) {
        self.worker.cancel();
        self.active = None;
        self.candidate = None;
        self.ready = None;
        self.pending_action = None;
        self.timing_draft = None;
        self.status =
            "Valmisteltu repliikki tai ajoitusasetukset muuttuivat. Aiempi ajoitusarvio hylättiin."
                .into();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> Identity {
        Identity {
            generation: 1,
            line_id: 7,
            audio_path: "/prepared.wav".into(),
            transcript: "hello".into(),
        }
    }
    #[test]
    fn source_selection_or_audio_changes_discard_candidate() {
        for change in 0..4 {
            let mut panel = Panel::new();
            let original = identity();
            panel.candidate = Some(Ready {
                cache: Default::default(),
                identity: original.clone(),
                json: "{}".into(),
                scene_json: "{}".into(),
            });
            let mut changed = original;
            match change {
                0 => changed.generation += 1,
                1 => changed.line_id += 1,
                2 => changed.audio_path = "/replacement.wav".into(),
                _ => changed.transcript = "different".into(),
            }
            panel.tick(Some(&changed));
            assert!(panel.candidate.is_none());
            assert!(panel.take_ready().is_none());
        }
    }
    #[test]
    fn unchanged_identity_keeps_candidate_but_does_not_auto_apply() {
        let mut p = Panel::new();
        let id = identity();
        p.candidate = Some(Ready {
            cache: Default::default(),
            identity: id.clone(),
            json: "{}".into(),
            scene_json: "{}".into(),
        });
        p.tick(Some(&id));
        assert!(p.candidate.is_some());
        assert!(p.take_ready().is_none());
        p.cancel();
        assert!(p.candidate.is_none());
    }
    #[test]
    fn start_requires_explicit_action_and_invalidation_clears_it() {
        let mut p = Panel::new();
        let audio: Arc<dyn PreparedAudio> = Arc::new(motion_audio::AudioBuffer {
            sample_rate: 24000,
            channels: 1,
            samples: vec![0.0; 24],
        });
        assert!(p
            .start(
                identity(),
                audio,
                String::new(),
                PathBuf::from("/not-accessed")
            )
            .unwrap_err()
            .contains("Valitse ensin arviointi"));
        assert!(!p.busy());
        p.pending_action = Some(Action::Load {
            key: "a".repeat(64),
        });
        p.invalidate();
        assert!(p.pending_action.is_none());
    }
    #[test]
    fn headless_panel_performs_no_work() {
        let context = egui::Context::default();
        let mut p = Panel::new();
        p.cache_key = "a".repeat(64);
        let _ = context.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!p.ui(ui, false));
            });
        });
        assert!(p.pending_action.is_none());
        assert!(!p.busy());
        assert_eq!(p.generation(), None);
    }
}
