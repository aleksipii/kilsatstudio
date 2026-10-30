//! Model sheet and character style panel (P08, docs/model-sheet.md).
//!
//! Builds the selected character's model sheet from its rig, checks the rest
//! pose silhouette and saves the sheet as SVG and PNG. Sheet rendering and PNG
//! encoding run on one background thread at a time, never on the event loop.
//! Choosing a style here previews it; the project rig is not changed.
use eframe::egui;
use motion_art::style::CharacterStyle;
use motion_render::model_sheet::{render_model_sheet, ModelSheetOptions, Proportions};
use motion_rig::RigDefinition;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

/// What a finished job reports back to the panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub actor: String,
    pub style: CharacterStyle,
    pub proportions: Option<Proportions>,
    /// `(limb, visible percent, warning)`.
    pub limbs: Vec<(&'static str, f64, bool)>,
    pub palette: usize,
    pub saved: Vec<PathBuf>,
}

#[derive(Default)]
pub struct Panel {
    style: CharacterStyle,
    output: String,
    status: String,
    job: Option<Receiver<Result<Summary, String>>>,
    summary: Option<Summary>,
}

/// Render the sheet and, when `output` is set, write `<rig>-<style>-model-sheet`
/// `.svg` and `.png` into that existing folder.
pub fn build(
    actor: &str,
    rig: &RigDefinition,
    style: CharacterStyle,
    output: Option<PathBuf>,
) -> Result<Summary, String> {
    let sheet = render_model_sheet(
        rig,
        &ModelSheetOptions {
            style,
            images: None,
        },
    )
    .map_err(|e| e.to_string())?;
    let mut saved = Vec::new();
    if let Some(dir) = output {
        if !dir.is_dir() {
            return Err(format!("kansiota {} ei ole", dir.display()));
        }
        let stem = format!("{}-{}-model-sheet", rig.id, style.name());
        let svg_path = dir.join(format!("{stem}.svg"));
        std::fs::write(&svg_path, &sheet.svg).map_err(|e| e.to_string())?;
        saved.push(svg_path);
        let png = motion_export::rasterize_svg_png(&sheet.svg).map_err(|e| e.to_string())?;
        let png_path = dir.join(format!("{stem}.png"));
        std::fs::write(&png_path, png).map_err(|e| e.to_string())?;
        saved.push(png_path);
    }
    Ok(Summary {
        actor: actor.to_string(),
        style,
        proportions: sheet.proportions,
        limbs: sheet
            .silhouette
            .limbs
            .iter()
            .map(|l| (l.limb, l.visible_fraction * 100.0, l.warning))
            .collect(),
        palette: sheet.palette.len(),
        saved,
    })
}

impl Panel {
    fn busy(&self) -> bool {
        self.job.is_some()
    }

    fn start(&mut self, actor: &str, rig: &RigDefinition, save: bool) {
        if self.busy() {
            return;
        }
        let output = save.then(|| PathBuf::from(self.output.trim()));
        let (actor, rig, style) = (actor.to_string(), rig.clone(), self.style);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(build(&actor, &rig, style, output));
        });
        self.job = Some(rx);
        self.status = "Luodaan model sheetiä…".into();
    }

    fn poll(&mut self) {
        let Some(job) = &self.job else {
            return;
        };
        match job.try_recv() {
            Ok(Ok(summary)) => {
                self.status = if summary.saved.is_empty() {
                    "Tarkistus valmis.".into()
                } else {
                    format!(
                        "Tallennettu: {}",
                        summary
                            .saved
                            .iter()
                            .map(|p| p.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                self.summary = Some(summary);
                self.job = None;
            }
            Ok(Err(e)) => {
                self.status = format!("Model sheet epäonnistui: {e}");
                self.job = None;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.status = "Model sheet -työ keskeytyi.".into();
                self.job = None;
            }
        }
    }

    /// `character` is the selected actor and its rig, when it has one.
    pub fn ui(&mut self, ui: &mut egui::Ui, character: Option<(&str, &RigDefinition)>) {
        self.poll();
        let Some((actor, rig)) = character else {
            ui.weak("Valitulla hahmolla ei ole omaa rigiä, joten model sheetiä ei voi luoda.");
            return;
        };
        ui.label(format!("Hahmo {actor}"));
        egui::ComboBox::from_id_salt("model_sheet_style")
            .selected_text(self.style.name())
            .show_ui(ui, |ui| {
                for style in CharacterStyle::ALL {
                    ui.selectable_value(&mut self.style, style, style.name());
                }
            });
        ui.weak("Tyyli näkyy vain model sheetissä. Projektin hahmo ei muutu.");
        ui.horizontal(|ui| {
            ui.label("Tuloskansion koko polku");
            ui.add_enabled(!self.busy(), egui::TextEdit::singleline(&mut self.output));
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.busy(),
                    egui::Button::new("Tarkista siluetti ja mittasuhteet"),
                )
                .clicked()
            {
                self.start(actor, rig, false);
            }
            if ui
                .add_enabled(
                    !self.busy() && !self.output.trim().is_empty(),
                    egui::Button::new("Tallenna model sheet (SVG + PNG)"),
                )
                .clicked()
            {
                self.start(actor, rig, true);
            }
        });
        if self.busy() {
            ui.spinner();
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
        if !self.status.is_empty() {
            ui.label(&self.status);
        }
        if let Some(s) = self.summary.as_ref().filter(|s| s.actor == actor) {
            ui.label(format!(
                "Tyyli {} · paletissa {} väriä",
                s.style.name(),
                s.palette
            ));
            match s.proportions {
                Some(p) => ui.label(format!(
                    "Korkeus {:.2} päänkorkeutta (pää {:.0}, koko hahmo {:.0} yksikköä)",
                    p.heads, p.head_height, p.total_height
                )),
                None => ui.weak("Päänivelen osia ei löytynyt; päänkorkeutta ei mitattu."),
            };
            if s.limbs.is_empty() {
                ui.weak("Rigissä ei ole tikku-ukon raajaniveliä; siluettia ei mitattu.");
            }
            for (limb, visible, warning) in &s.limbs {
                let text = format!("{limb}: {visible:.0} % näkyy siluetissa");
                if *warning {
                    ui.colored_label(
                        egui::Color32::from_rgb(200, 60, 50),
                        format!("Varoitus: {text}; raaja sulautuu vartaloon."),
                    );
                } else {
                    ui.label(text);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rig() -> RigDefinition {
        serde_yaml::from_str(
            "version: 1\nid: tiny\njoints: [{id: root, parent: null}, {id: head, parent: root, y: -40}]\nparts:\n  - {id: body, joint: root, points: [[-5, 0], [5, 0], [5, -30], [-5, -30]], color: [30, 30, 30, 255]}\n  - {id: face, joint: head, ellipse: {rx: 10, ry: 10}, color: [240, 240, 230, 255]}\n",
        )
        .unwrap()
    }

    #[test]
    fn build_reports_and_saves_svg_and_png() {
        let dir = std::env::temp_dir().join(format!("kilsat-model-sheet-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let summary = build("T", &rig(), CharacterStyle::FlatTv, Some(dir.clone())).unwrap();
        assert_eq!(summary.saved.len(), 2);
        let png = std::fs::read(&summary.saved[1]).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        let p = summary.proportions.unwrap();
        // Head circle top at -50, body bottom at 0: 50 / 20 = 2.5 heads.
        assert!((p.heads - 2.5).abs() < 1e-9, "{p:?}");
        // No stick limbs: nothing to measure, nothing to warn about.
        assert!(summary.limbs.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
        // A missing folder is a clear error, not a panic.
        assert!(build("T", &rig(), CharacterStyle::Default, Some(dir)).is_err());
    }
}
