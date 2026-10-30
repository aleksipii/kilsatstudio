//! X-sheet panel (P02): frame-by-frame dialogue, viseme, chart shape and timing
//! provenance of the prepared scene. The sheet is built from the scene export
//! manifest, so it shows exactly the mouth shapes video export writes. Building and
//! file export run on a worker thread; the UI only draws finished rows.
use eframe::egui;
use motion_lipsync::english::VisemeCue;
use motion_lipsync::xsheet::{self, XSheet, XSheetTrack};
use motion_voice::export::SceneManifest;
use std::{
    path::PathBuf,
    sync::{mpsc, Arc},
};

const ROW_HEIGHT: f32 = 18.0;

/// What a built sheet depends on: the prepared manifest (by identity) and the fps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key {
    manifest: usize,
    fps: u32,
}

enum Task {
    Build {
        key: Key,
        manifest: Arc<SceneManifest>,
        /// Screenplay and project root used to look up the dialogue text.
        script: Option<Box<(motion_script::Script, String)>>,
        fps: u32,
    },
    Export {
        path: PathBuf,
        contents: String,
    },
}

enum Done {
    Built(Key, Result<Arc<XSheet>, String>),
    Exported(Result<PathBuf, String>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Csv,
    Text,
}

pub struct XSheetPanel {
    tx: mpsc::SyncSender<Task>,
    rx: mpsc::Receiver<Done>,
    requested: Option<Key>,
    sheet: Option<(Key, Arc<XSheet>)>,
    status: String,
    path: String,
    format: Format,
    follow: bool,
    last_row: Option<usize>,
}

impl XSheetPanel {
    pub fn new() -> Self {
        let (tx, tasks) = mpsc::sync_channel::<Task>(1);
        let (done, rx) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("xsheet".into())
            .spawn(move || {
                while let Ok(task) = tasks.recv() {
                    let result = match task {
                        Task::Build {
                            key,
                            manifest,
                            script,
                            fps,
                        } => {
                            let lines = script.and_then(|pair| {
                                let (script, root) = *pair;
                                scene_lines(&script, &root, &manifest.scene_id)
                            });
                            Done::Built(key, build(&manifest, lines.as_deref(), fps).map(Arc::new))
                        }
                        Task::Export { path, contents } => Done::Exported(
                            std::fs::write(&path, contents)
                                .map(|()| path.clone())
                                .map_err(|e| format!("{}: {e}", path.display())),
                        ),
                    };
                    if done.send(result).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn X-sheet worker");
        Self {
            tx,
            rx,
            requested: None,
            sheet: None,
            status: String::new(),
            path: String::new(),
            format: Format::Csv,
            follow: true,
            last_row: None,
        }
    }

    fn busy(&self) -> bool {
        self.requested.is_some()
    }

    fn poll(&mut self) {
        while let Ok(done) = self.rx.try_recv() {
            match done {
                Done::Built(key, result) => {
                    if self.requested == Some(key) {
                        self.requested = None;
                    }
                    match result {
                        Ok(sheet) => {
                            self.status.clear();
                            self.sheet = Some((key, sheet));
                        }
                        Err(error) => {
                            self.status = format!("X-sheet: {error}");
                            self.sheet = None;
                        }
                    }
                }
                Done::Exported(result) => {
                    self.status = match result {
                        Ok(path) => format!("X-sheet viety: {}", path.display()),
                        Err(error) => format!("X-sheetin vienti epäonnistui: {error}"),
                    }
                }
            }
        }
    }
}

/// Manifest entries become scene-sample tracks. Dialogue text is attached only when
/// the caller matched it to the manifest; it is one span per line (no word timing).
fn build(
    manifest: &SceneManifest,
    lines: Option<&[(String, String)]>,
    fps: u32,
) -> Result<XSheet, String> {
    let tracks: Vec<XSheetTrack> = manifest
        .dialogue
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let track = XSheetTrack {
                clip_id: index as u64,
                speaker: entry.actor.clone(),
                start_sample: entry.start_frame,
                end_sample: entry.start_frame.saturating_add(entry.duration_frames),
                source: entry.alignment_source.clone(),
                visemes: entry
                    .visemes
                    .iter()
                    .map(|v| VisemeCue {
                        start_sample: v.start_frame,
                        end_sample: v.end_frame,
                        viseme: v.viseme,
                    })
                    .collect(),
                phonemes: Vec::new(),
                words: Vec::new(),
            };
            match lines.and_then(|l| l.get(index)) {
                Some((actor, text)) if *actor == entry.actor => track.with_line(text),
                _ => track,
            }
        })
        .collect();
    xsheet::build(&manifest.scene_id, manifest.total_frames, fps, &tracks)
        .map_err(|e| e.to_string())
}

/// Dialogue text of `scene` in screenplay order (the order of the scene manifest).
fn scene_lines(
    script: &motion_script::Script,
    root: &str,
    scene: &str,
) -> Option<Vec<(String, String)>> {
    let jobs = motion_voice::dialogue_jobs(script, std::path::Path::new(root)).ok()?;
    Some(
        jobs.into_iter()
            .filter(|job| job.scene == scene)
            .map(|job| (job.actor, job.request.text))
            .collect(),
    )
}

impl crate::Studio {
    /// Level 2 tool next to the viseme check. Returns nothing; row clicks seek.
    pub(crate) fn xsheet_ui(&mut self, ui: &mut egui::Ui) {
        self.xsheet.poll();
        ui.separator();
        egui::CollapsingHeader::new("X-sheet ja suukartta (A–H, X)")
            .id_salt("xsheet-panel")
            .show(ui, |ui| self.xsheet_body(ui));
        if self.xsheet.busy() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    fn xsheet_body(&mut self, ui: &mut egui::Ui) {
        let Some(preview) = &self.scene_preview else {
            ui.weak("Valmistele kohtauksen ääni Puhe-välilehdellä nähdäksesi X-sheetin.");
            return;
        };
        let fps = self.script.as_ref().map_or(24, |s| s.project.fps);
        let key = Key {
            manifest: Arc::as_ptr(&preview.manifest) as usize,
            fps,
        };
        let current = self.xsheet.sheet.as_ref().is_some_and(|(k, _)| *k == key);
        if !current && self.xsheet.requested != Some(key) {
            let task = Task::Build {
                key,
                manifest: Arc::clone(&preview.manifest),
                script: self
                    .script
                    .clone()
                    .map(|script| Box::new((script, self.project_root.clone()))),
                fps,
            };
            if self.xsheet.tx.try_send(task).is_ok() {
                self.xsheet.requested = Some(key);
                self.xsheet.status = "Rakennetaan X-sheetiä…".into();
            }
        }
        if !self.xsheet.status.is_empty() {
            ui.label(&self.xsheet.status);
        }
        let Some((_, sheet)) = self.xsheet.sheet.clone().filter(|(k, _)| *k == key) else {
            return;
        };
        ui.weak(
            "Suu näytetään ruudun ensimmäisen näytteen kohdalta, kuten videoviennissä. \
             Repliikki näkyy ensimmäisellä ruudullaan; sanakohtaista ajoitusta ei ole.",
        );
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.xsheet.follow, "Seuraa toistopäätä");
            ui.radio_value(&mut self.xsheet.format, Format::Csv, "CSV");
            ui.radio_value(&mut self.xsheet.format, Format::Text, "Teksti");
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.xsheet.path)
                    .hint_text("polku, esim. xsheet.csv")
                    .desired_width(180.0),
            );
            if ui
                .add_enabled(
                    !self.xsheet.path.trim().is_empty(),
                    egui::Button::new("Vie"),
                )
                .clicked()
            {
                let contents = match self.xsheet.format {
                    Format::Csv => sheet.to_csv(),
                    Format::Text => sheet.to_text(),
                };
                let path = PathBuf::from(self.xsheet.path.trim());
                let path = if path.is_relative() {
                    PathBuf::from(&self.project_root).join(path)
                } else {
                    path
                };
                self.xsheet.status = match self.xsheet.tx.try_send(Task::Export { path, contents })
                {
                    Ok(()) => "Viedään X-sheetiä…".into(),
                    Err(_) => "X-sheet-työ on kesken; yritä hetken päästä uudelleen.".into(),
                };
            }
        });
        let playhead_row = motion_audio::clock::seconds_to_sample(self.playhead)
            .ok()
            .and_then(|s| sheet.row_at_sample(s));
        let mut seek_to = None;
        let header = |ui: &mut egui::Ui| {
            ui.horizontal(|ui| {
                ui.monospace(format!("{:>5} {:<11}", "ruutu", "aikakoodi"));
                for speaker in &sheet.speakers {
                    ui.monospace(format!("| {speaker:<24}"));
                }
            });
        };
        header(ui);
        let mut area = egui::ScrollArea::vertical()
            .id_salt("xsheet-rows")
            .max_height(260.0)
            .auto_shrink([false, true]);
        let moved = playhead_row != self.xsheet.last_row;
        if let (true, true, Some(row)) = (self.xsheet.follow, moved, playhead_row) {
            // Keep the playhead row in the upper third of the view.
            area = area.vertical_scroll_offset((row as f32 - 4.0).max(0.0) * ROW_HEIGHT);
        }
        self.xsheet.last_row = playhead_row;
        area.show_rows(ui, ROW_HEIGHT, sheet.rows.len(), |ui, range| {
            for row in &sheet.rows[range] {
                let mut text = format!(
                    "{:>5} {:<11}",
                    row.frame,
                    xsheet::timecode(row.frame, sheet.fps)
                );
                for cell in &row.cells {
                    let word: String = cell.word.label().chars().take(10).collect();
                    text.push_str(&format!(
                        "| {:<10} {:<12} {} {:<1}",
                        word,
                        motion_lipsync::mouth_chart::viseme_name(cell.viseme),
                        cell.chart,
                        timing_mark(&cell.provenance),
                    ));
                }
                let selected = playhead_row == Some(row.frame as usize);
                let response = ui
                    .add_sized(
                        [ui.available_width(), ROW_HEIGHT],
                        egui::SelectableLabel::new(selected, egui::RichText::new(text).monospace()),
                    )
                    .on_hover_text(row_hover(&sheet, row));
                if response.clicked() {
                    seek_to = Some(row.sample);
                }
            }
        });
        ui.weak("Ajoitus: M = käsin, F = pakotettu, E = arvio, – = ei ajoitusta.");
        if let Some(sample) = seek_to {
            self.seek_scene(motion_audio::clock::sample_to_seconds(sample));
        }
    }
}

fn timing_mark(provenance: &xsheet::Provenance) -> &'static str {
    use motion_lipsync::AlignmentSource;
    match provenance {
        xsheet::Provenance::Silent => "",
        xsheet::Provenance::Untimed => "–",
        xsheet::Provenance::Timed(AlignmentSource::Manual) => "M",
        xsheet::Provenance::Timed(AlignmentSource::Forced { .. }) => "F",
        xsheet::Provenance::Timed(AlignmentSource::Estimated { .. }) => "E",
    }
}

fn row_hover(sheet: &XSheet, row: &xsheet::XSheetRow) -> String {
    let mut text = format!("Ruutu {} · näyte {}", row.frame, row.sample);
    for (speaker, cell) in sheet.speakers.iter().zip(&row.cells) {
        text.push_str(&format!(
            "\n{speaker}: {} · {} · {}",
            cell.word.label(),
            cell.chart,
            cell.provenance.label()
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use motion_lipsync::{AlignmentSource, Viseme};
    use motion_voice::export::{DialogueManifest, SampleViseme};

    fn manifest() -> SceneManifest {
        SceneManifest {
            schema_version: 1,
            scene_id: "s1".into(),
            sample_rate: 24_000,
            channels: 1,
            total_frames: 6000,
            dialogue: vec![DialogueManifest {
                actor: "EMMA".into(),
                start_frame: 1000,
                duration_frames: 3000,
                trim_start_frame: 0,
                gain: 1.0,
                alignment_source: Some(AlignmentSource::Manual),
                visemes: vec![SampleViseme {
                    start_frame: 2000,
                    end_frame: 3000,
                    viseme: Viseme::O,
                }],
            }],
            cues: Vec::new(),
        }
    }

    #[test]
    fn manifest_rows_match_export_sampling_and_attach_matching_text() {
        let lines = vec![("EMMA".to_string(), "Oh no".to_string())];
        let sheet = build(&manifest(), Some(&lines), 24).unwrap();
        assert_eq!(sheet.rows.len(), 6);
        let shapes: String = sheet
            .rows
            .iter()
            .map(|r| r.cells[0].chart.letter())
            .collect();
        assert_eq!(shapes, "XXEXXX");
        assert_eq!(sheet.rows[1].cells[0].word.label(), "Oh no");
        assert_eq!(timing_mark(&sheet.rows[1].cells[0].provenance), "M");
        assert_eq!(timing_mark(&sheet.rows[0].cells[0].provenance), "");
        // Text for another actor is not attached to the wrong line.
        let wrong = vec![("ALEX".to_string(), "Hi".to_string())];
        let sheet = build(&manifest(), Some(&wrong), 24).unwrap();
        assert_eq!(sheet.rows[1].cells[0].word.label(), "");
    }

    #[test]
    fn worker_builds_off_thread_and_reports_errors() {
        let mut panel = XSheetPanel::new();
        let key = Key {
            manifest: 1,
            fps: 0,
        };
        panel
            .tx
            .send(Task::Build {
                key,
                manifest: Arc::new(manifest()),
                script: None,
                fps: 0,
            })
            .unwrap();
        panel.requested = Some(key);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while panel.busy() && std::time::Instant::now() < deadline {
            std::thread::yield_now();
            panel.poll();
        }
        assert!(!panel.busy());
        assert!(panel.sheet.is_none());
        assert!(panel.status.contains("frame rate 0"), "{}", panel.status);
    }
}
