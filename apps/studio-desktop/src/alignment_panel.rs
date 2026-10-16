//! Explicit opt-in estimated alignment of an already prepared dialogue WAV.
pub use crate::alignment_worker::{Identity, Ready};
use crate::alignment_worker::{PreparedAudio, Request, Worker};
use eframe::egui;
use motion_lipsync::rhubarb::Recognizer;
use motion_tts::CancellationToken;
use std::{path::PathBuf, sync::Arc};

pub struct Panel {
    worker: Worker,
    executable: String,
    recognizer: Recognizer,
    active: Option<Identity>,
    candidate: Option<Ready>,
    ready: Option<Ready>,
    status: String,
}
impl Panel {
    pub fn new() -> Self {
        Self {
            worker: Worker::new(),
            executable: String::new(),
            recognizer: Recognizer::PocketSphinx,
            active: None,
            candidate: None,
            ready: None,
            status: String::new(),
        }
    }
    /// Returns true only on an explicit Run click. The caller constructs the
    /// identity/audio request then; painting performs no file access or hashing.
    pub fn ui(&mut self, ui: &mut egui::Ui, can_run: bool) -> bool {
        let mut run = false;
        let mut settings_changed = false;
        ui.collapsing("Local estimated mouth timing (Rhubarb)", |ui| {
            ui.label("Uses the prepared line's WAV. Mouth shapes are estimated, not forced-aligned phonemes. No downloads or synthesis.");
            ui.add_enabled_ui(!self.busy(), |ui| {
                ui.horizontal(|ui| { ui.label("Executable"); settings_changed |= ui.add(egui::TextEdit::singleline(&mut self.executable).char_limit(4096).hint_text("/absolute/path/to/rhubarb")).changed(); });
                egui::ComboBox::from_id_salt("native-rhubarb-recognizer").selected_text(match self.recognizer { Recognizer::PocketSphinx=>"PocketSphinx (English)", Recognizer::Phonetic=>"Phonetic" }).show_ui(ui, |ui| {
                    settings_changed |= ui.selectable_value(&mut self.recognizer,Recognizer::PocketSphinx,"PocketSphinx (English)").changed();
                    settings_changed |= ui.selectable_value(&mut self.recognizer,Recognizer::Phonetic,"Phonetic").changed();
                });
            });
            ui.horizontal(|ui| {
                run=ui.add_enabled(can_run && !self.busy(),egui::Button::new("Estimate prepared line")).clicked();
                if ui.add_enabled(self.busy(),egui::Button::new("Cancel alignment")).clicked() { self.cancel(); }
            });
            if !can_run { ui.label("Prepare the selected dialogue line first."); }
            if self.busy() { ui.spinner(); ui.ctx().request_repaint_after(std::time::Duration::from_millis(40)); }
            if !self.status.is_empty() { ui.label(&self.status); }
            if let Some(candidate)=&self.candidate {
                ui.label(format!("Estimated cues ready for line {} ({} bytes). Apply to reprepare the same audio.",candidate.identity.line_id,candidate.json.len()));
                if ui.add_enabled(can_run && !self.busy(),egui::Button::new("Apply estimated timing")).clicked() { self.ready=self.candidate.take(); }
            }
        });
        if settings_changed {
            self.invalidate();
        }
        run
    }
    pub fn start(
        &mut self,
        identity: Identity,
        audio: Arc<dyn PreparedAudio>,
        scene_alignments: String,
    ) -> Result<(), String> {
        let request = Request {
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
                self.ready = None;
                self.status = "Estimating prepared audio locally…".into();
                Ok(())
            }
            Err(e) => {
                self.status = e.clone();
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
                    self.candidate = Some(ready);
                    self.status = "Estimation complete. Review and apply explicitly.".into();
                }
                Err(e) => self.status = e,
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
        self.status = "Alignment cancelled; waiting for the local process to stop.".into();
    }
    pub fn invalidate(&mut self) {
        self.worker.cancel();
        self.active = None;
        self.candidate = None;
        self.ready = None;
        self.status = "Prepared line changed; previous estimated timing discarded.".into();
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
    fn headless_panel_performs_no_work() {
        let context = egui::Context::default();
        let mut p = Panel::new();
        let _ = context.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!p.ui(ui, false));
            });
        });
        assert!(!p.busy());
        assert_eq!(p.generation(), None);
    }
}
