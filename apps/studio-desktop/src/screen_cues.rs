//! Screen-cue editor: what a phone or tablet screen slot shows over scene time.
//!
//! Every add, change and removal is one validated `composition.screens` edit
//! through `motion_script::apply_composition`, applied by the caller on the
//! undoable screenplay path. Errors are returned for inline display.
use eframe::egui;
use motion_rig::TextureFit;
use motion_script::{ScreenCue, ScreenSource, Script};

/// `(actor, slot)` for every character rig with a screen slot.
pub fn slots(script: &Script) -> Vec<(String, String)> {
    script
        .characters
        .iter()
        .filter_map(|(actor, c)| c.rig.as_ref().map(|rig| (actor, rig)))
        .flat_map(|(actor, rig)| {
            rig.screens
                .iter()
                .map(move |slot| (actor.clone(), slot.id.clone()))
        })
        .collect()
}

fn edit(
    source: &str,
    scene_index: usize,
    change: impl FnOnce(&mut Vec<ScreenCue>) -> Result<(), String>,
) -> Result<String, String> {
    let script = motion_script::parse(source).map_err(|e| e.to_string())?;
    let mut composition = script
        .scenes
        .get(scene_index)
        .ok_or("Valittua kohtausta ei enää ole.")?
        .composition
        .clone();
    change(&mut composition.screens)?;
    motion_script::apply_composition(source, scene_index, composition).map_err(|e| e.to_string())
}

/// Append `cue` to the scene's screen cues.
pub fn add(source: &str, scene_index: usize, cue: ScreenCue) -> Result<String, String> {
    edit(source, scene_index, |cues| {
        cues.push(cue);
        Ok(())
    })
}

/// Replace cue `index`.
pub fn replace(
    source: &str,
    scene_index: usize,
    index: usize,
    cue: ScreenCue,
) -> Result<String, String> {
    edit(source, scene_index, |cues| {
        *cues.get_mut(index).ok_or("Näyttömuutosta ei enää ole.")? = cue;
        Ok(())
    })
}

/// Remove cue `index`.
pub fn remove(source: &str, scene_index: usize, index: usize) -> Result<String, String> {
    edit(source, scene_index, |cues| {
        if index >= cues.len() {
            return Err("Näyttömuutosta ei enää ole.".into());
        }
        cues.remove(index);
        Ok(())
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Off,
    Image,
    Video,
    /// Image sequences are kept as authored; edit their frames in YAML.
    Sequence,
}

/// Editable copy of one cue.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    /// `Some(i)` edits cue `i`; `None` adds a new cue.
    pub editing: Option<usize>,
    pub actor: String,
    pub slot: String,
    pub at: f64,
    pub kind: Kind,
    pub image: String,
    pub video: String,
    pub fps: f64,
    pub duration: f64,
    sequence: Option<ScreenSource>,
    pub fit: TextureFit,
    pub trim_start: f64,
    pub trim_end: Option<f64>,
    pub speed: f64,
    pub looping: bool,
    /// Source seconds separated by commas, semicolons or spaces.
    pub taps: String,
}
impl Draft {
    pub fn new(actor: &str, slot: &str, at: f64) -> Self {
        Self {
            editing: None,
            actor: actor.into(),
            slot: slot.into(),
            at,
            kind: Kind::Off,
            image: String::new(),
            video: String::new(),
            fps: 30.0,
            duration: 1.0,
            sequence: None,
            fit: TextureFit::Cover,
            trim_start: 0.0,
            trim_end: None,
            speed: 1.0,
            looping: false,
            taps: String::new(),
        }
    }
    pub fn from_cue(index: usize, cue: &ScreenCue) -> Self {
        let mut draft = Self::new(&cue.actor, &cue.slot, cue.at);
        draft.editing = Some(index);
        draft.fit = cue.fit;
        draft.trim_start = cue.trim_start;
        draft.trim_end = cue.trim_end;
        draft.speed = cue.speed;
        draft.looping = cue.looping;
        draft.taps = cue
            .taps
            .iter()
            .map(f64::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        match &cue.source {
            ScreenSource::Off => draft.kind = Kind::Off,
            ScreenSource::Image { asset } => {
                draft.kind = Kind::Image;
                draft.image = asset.clone();
            }
            ScreenSource::Video {
                asset,
                fps,
                duration,
            } => {
                draft.kind = Kind::Video;
                draft.video = asset.clone();
                draft.fps = *fps;
                draft.duration = *duration;
            }
            ScreenSource::Sequence { .. } => {
                draft.kind = Kind::Sequence;
                draft.sequence = Some(cue.source.clone());
            }
        }
        draft
    }
    /// A video cue for a freshly imported recording.
    pub fn set_video(&mut self, asset: &str, fps: f64, duration: f64) {
        self.kind = Kind::Video;
        self.video = asset.into();
        self.fps = fps;
        self.duration = duration;
        self.trim_start = 0.0;
        self.trim_end = None;
        self.taps.clear();
    }
    pub fn to_cue(&self) -> Result<ScreenCue, String> {
        if self.actor.is_empty() || self.slot.is_empty() {
            return Err("Valitse hahmo ja näyttöpaikka.".into());
        }
        let source = match self.kind {
            Kind::Off => ScreenSource::Off,
            Kind::Image => ScreenSource::Image {
                asset: self.image.trim().into(),
            },
            Kind::Video => ScreenSource::Video {
                asset: self.video.trim().into(),
                fps: self.fps,
                duration: self.duration,
            },
            Kind::Sequence => self
                .sequence
                .clone()
                .ok_or("Kuvasarjaa voi muokata vain käsikirjoituksessa.")?,
        };
        let timed = matches!(self.kind, Kind::Video | Kind::Sequence);
        let mut taps = Vec::new();
        if timed {
            for item in self
                .taps
                .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
                .filter(|s| !s.is_empty())
            {
                taps.push(item.parse::<f64>().map_err(|_| {
                    format!("Napautus \"{item}\" ei ole luku (käytä pistettä desimaaleissa).")
                })?);
            }
        }
        // Stills and `off` take no timing fields; never send stale ones.
        Ok(ScreenCue {
            actor: self.actor.clone(),
            slot: self.slot.clone(),
            at: self.at,
            source,
            fit: self.fit,
            trim_start: if timed { self.trim_start } else { 0.0 },
            trim_end: if timed { self.trim_end } else { None },
            speed: if timed { self.speed } else { 1.0 },
            looping: timed && self.looping,
            taps,
        })
    }
}

/// What the user asked for this frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Apply(Box<Draft>),
    Remove(usize),
    ImportVideo(String),
    CancelImport,
    RetryDecode,
}

/// Panel UI state (never part of the document).
#[derive(Default)]
pub struct Panel {
    pub draft: Option<Draft>,
    pub error: Option<String>,
    pub import_path: String,
    pub slot_index: usize,
}

pub fn source_label(source: &ScreenSource) -> String {
    match source {
        ScreenSource::Off => "sammutettu".into(),
        ScreenSource::Image { asset } => format!("kuva {asset}"),
        ScreenSource::Sequence { frames, fps } => {
            format!("kuvasarja, {} ruutua @ {fps} fps", frames.len())
        }
        ScreenSource::Video {
            asset, duration, ..
        } => format!("video {asset} ({duration:.2} s)"),
    }
}

fn fit_label(fit: TextureFit) -> &'static str {
    match fit {
        TextureFit::Cover => "Täytä (rajaa reunat)",
        TextureFit::Contain => "Sovita kokonaan",
        TextureFit::Stretch => "Venytä",
    }
}

/// Inputs the panel shows.
pub struct View<'a> {
    pub script: &'a Script,
    pub scene_index: usize,
    pub playhead: f64,
    pub images: &'a [String],
    pub enabled: bool,
    pub importing: bool,
    pub decode_status: &'a [String],
    pub has_failures: bool,
}

pub fn ui(ui: &mut egui::Ui, panel: &mut Panel, view: View<'_>) -> Option<Action> {
    let mut action = None;
    let scene = view.script.scenes.get(view.scene_index)?;
    let slots = slots(view.script);
    if slots.is_empty() {
        ui.weak("Yhdelläkään hahmolla ei ole näyttöpaikkaa. Kiinnitä puhelin tai tabletti käteen (Esine käteen).");
        return None;
    }
    ui.add_enabled_ui(view.enabled, |ui| {
        let mut remove = None;
        for (index, cue) in scene.composition.screens.iter().enumerate() {
            ui.horizontal_wrapped(|ui| {
                let selected = panel.draft.as_ref().and_then(|d| d.editing) == Some(index);
                if ui
                    .selectable_label(
                        selected,
                        format!(
                            "{:.2} s · {}.{} · {}",
                            cue.at,
                            cue.actor,
                            cue.slot,
                            source_label(&cue.source)
                        ),
                    )
                    .clicked()
                {
                    panel.draft = Some(Draft::from_cue(index, cue));
                    panel.error = None;
                }
                if ui.small_button("Poista").clicked() {
                    remove = Some(index);
                }
            });
        }
        if let Some(index) = remove {
            action = Some(Action::Remove(index));
        }
        if scene.composition.screens.is_empty() {
            ui.weak("Ei näyttömuutoksia: näytöt näyttävät hahmon oman piirroksen.");
        }
        panel.slot_index = panel.slot_index.min(slots.len() - 1);
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("screen_cue_slot")
                .selected_text(format!(
                    "{}.{}",
                    slots[panel.slot_index].0, slots[panel.slot_index].1
                ))
                .show_ui(ui, |ui| {
                    for (i, (actor, slot)) in slots.iter().enumerate() {
                        ui.selectable_value(&mut panel.slot_index, i, format!("{actor}.{slot}"));
                    }
                });
            if ui.button("+ Uusi näyttömuutos toistopäähän").clicked() {
                let (actor, slot) = &slots[panel.slot_index];
                panel.draft = Some(Draft::new(actor, slot, view.playhead));
                panel.error = None;
            }
        });
        let mut close = false;
        if let Some(draft) = &mut panel.draft {
            ui.separator();
            ui.label(match draft.editing {
                Some(i) => format!("Muokkaa näyttömuutosta {}", i + 1),
                None => "Uusi näyttömuutos".into(),
            });
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("screen_cue_draft_slot")
                    .selected_text(format!("{}.{}", draft.actor, draft.slot))
                    .show_ui(ui, |ui| {
                        for (actor, slot) in &slots {
                            if ui
                                .selectable_label(
                                    draft.actor == *actor && draft.slot == *slot,
                                    format!("{actor}.{slot}"),
                                )
                                .clicked()
                            {
                                draft.actor = actor.clone();
                                draft.slot = slot.clone();
                            }
                        }
                    });
                ui.label("alkaa (s)");
                ui.add(
                    egui::DragValue::new(&mut draft.at)
                        .speed(0.05)
                        .range(0.0..=scene.duration),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Sisältö");
                ui.selectable_value(&mut draft.kind, Kind::Off, "Sammutettu");
                ui.selectable_value(&mut draft.kind, Kind::Image, "Kuva");
                ui.selectable_value(&mut draft.kind, Kind::Video, "Video");
                if draft.sequence.is_some() {
                    ui.selectable_value(&mut draft.kind, Kind::Sequence, "Kuvasarja");
                }
            });
            match draft.kind {
                Kind::Off => {}
                Kind::Image => {
                    egui::ComboBox::from_id_salt("screen_cue_image")
                        .selected_text(if draft.image.is_empty() {
                            "Valitse kuvaresurssi"
                        } else {
                            draft.image.as_str()
                        })
                        .show_ui(ui, |ui| {
                            for id in view.images {
                                ui.selectable_value(&mut draft.image, id.clone(), id);
                            }
                        });
                    if view.images.is_empty() {
                        ui.weak("Projektissa ei ole kuvaresursseja. Tuo kuva Lisäasetukset-välilehden resursseista.");
                    }
                }
                Kind::Video => {
                    ui.horizontal(|ui| {
                        ui.label("Video");
                        ui.text_edit_singleline(&mut draft.video);
                    });
                    ui.horizontal(|ui| {
                        ui.label("fps");
                        ui.add(egui::DragValue::new(&mut draft.fps).speed(0.1).range(0.001..=240.0));
                        ui.label("kesto (s)");
                        ui.add(egui::DragValue::new(&mut draft.duration).speed(0.05).range(0.001..=3600.0));
                    });
                }
                Kind::Sequence => {
                    ui.weak("Kuvasarjan ruudut muokataan käsikirjoituksessa; täällä voi muuttaa ajoitusta.");
                }
            }
            if draft.kind != Kind::Off {
                egui::ComboBox::from_id_salt("screen_cue_fit")
                    .selected_text(fit_label(draft.fit))
                    .show_ui(ui, |ui| {
                        for fit in [TextureFit::Cover, TextureFit::Contain, TextureFit::Stretch] {
                            ui.selectable_value(&mut draft.fit, fit, fit_label(fit));
                        }
                    });
            }
            if matches!(draft.kind, Kind::Video | Kind::Sequence) {
                ui.horizontal(|ui| {
                    ui.label("Rajaus alku (s)");
                    ui.add(egui::DragValue::new(&mut draft.trim_start).speed(0.05).range(0.0..=3600.0));
                    let mut has_end = draft.trim_end.is_some();
                    if ui.checkbox(&mut has_end, "loppu (s)").changed() {
                        draft.trim_end = has_end.then_some(if draft.kind == Kind::Video {
                            draft.duration
                        } else {
                            draft.trim_start + 1.0
                        });
                    }
                    if let Some(end) = &mut draft.trim_end {
                        ui.add(egui::DragValue::new(end).speed(0.05).range(0.0..=3600.0));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Nopeus");
                    ui.add(egui::DragValue::new(&mut draft.speed).speed(0.05).range(0.01..=16.0));
                    ui.checkbox(&mut draft.looping, "Toista silmukkana");
                });
                ui.horizontal(|ui| {
                    ui.label("Napautukset (lähteen s)");
                    ui.text_edit_singleline(&mut draft.taps);
                });
            }
            ui.horizontal(|ui| {
                if ui
                    .button(if draft.editing.is_some() {
                        "Tallenna muutos"
                    } else {
                        "Lisää näyttömuutos"
                    })
                    .clicked()
                {
                    action = Some(Action::Apply(Box::new(draft.clone())));
                }
                if ui.button("Sulje").clicked() {
                    close = true;
                }
            });
        }
        if close {
            panel.draft = None;
            panel.error = None;
        }
        if let Some(error) = &panel.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        ui.separator();
        ui.label("Tuo video (.mp4, .mov, .m4v) projektin kansioon assets/screens");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut panel.import_path);
            if view.importing {
                ui.spinner();
                if ui.button("Peru tuonti").clicked() {
                    action = Some(Action::CancelImport);
                }
            } else if ui.button("Tuo video").clicked() {
                action = Some(Action::ImportVideo(panel.import_path.trim().to_owned()));
            }
        });
        ui.small("Tuonti kopioi videon, lukee kuvanopeuden ja keston ffprobella ja lisää videomuutoksen valittuun näyttöpaikkaan toistopään kohdalle.");
    });
    for line in view.decode_status {
        ui.colored_label(egui::Color32::from_rgb(220, 180, 90), line);
    }
    if view.has_failures && ui.button("Yritä videon purkua uudelleen").clicked() {
        action = Some(Action::RetryDecode);
    }
    action
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The stick studio with the catalog phone in VEIKKO's left hand.
    pub(crate) fn phone_source() -> (String, String) {
        let source = crate::stick_studio::load().unwrap().source;
        let catalog = crate::hand_props::catalog().unwrap();
        let phone = catalog
            .items
            .iter()
            .find(|i| i.screen.is_some())
            .expect("catalog has a phone");
        let source = crate::hand_props::attach(
            &source,
            "VEIKKO",
            crate::hand_props::Hand::Left,
            phone,
            Default::default(),
        )
        .unwrap();
        let script = motion_script::parse(&source).unwrap();
        let (_, slot) = slots(&script)
            .into_iter()
            .find(|(a, _)| a == "VEIKKO")
            .unwrap();
        (source, slot)
    }

    fn screens(source: &str) -> Vec<ScreenCue> {
        motion_script::parse(source).unwrap().scenes[0]
            .composition
            .screens
            .clone()
    }

    #[test]
    fn add_edit_and_remove_cues_are_validated_screenplay_edits() {
        let (source, slot) = phone_source();
        let mut draft = Draft::new("VEIKKO", &slot, 0.5);
        draft.set_video("assets/screens/demo.mov", 30.0, 4.0);
        draft.trim_start = 0.5;
        draft.looping = true;
        draft.taps = "1.0, 2.5".into();
        let added = add(&source, 0, draft.to_cue().unwrap()).unwrap();
        let cues = screens(&added);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].taps, vec![1.0, 2.5]);
        assert!(cues[0].looping);

        // Switching to `off` drops trim/speed/loop/taps instead of failing validation.
        let mut edit = Draft::from_cue(0, &cues[0]);
        assert_eq!(edit, {
            let mut d = draft.clone();
            d.editing = Some(0);
            d.taps = "1, 2.5".into();
            d
        });
        edit.kind = Kind::Off;
        edit.at = 1.0;
        let edited = replace(&added, 0, 0, edit.to_cue().unwrap()).unwrap();
        let cues = screens(&edited);
        assert_eq!(cues[0].source, ScreenSource::Off);
        assert_eq!(
            (cues[0].at, cues[0].looping, cues[0].taps.len()),
            (1.0, false, 0)
        );

        let removed = remove(&edited, 0, 0).unwrap();
        assert!(screens(&removed).is_empty());
        assert!(remove(&removed, 0, 0).is_err());
        assert!(replace(&removed, 0, 3, edit.to_cue().unwrap()).is_err());
        assert!(add(&removed, 99, edit.to_cue().unwrap()).is_err());
    }

    #[test]
    fn invalid_cues_return_actionable_errors_and_leave_source_alone() {
        let (source, slot) = phone_source();
        let mut draft = Draft::new("VEIKKO", "no_such_slot", 0.0);
        let error = add(&source, 0, draft.to_cue().unwrap()).unwrap_err();
        assert!(error.contains("no screen slot"), "{error}");
        draft.slot = slot.clone();
        draft.set_video("../outside.mov", 30.0, 2.0);
        assert!(add(&source, 0, draft.to_cue().unwrap())
            .unwrap_err()
            .contains("project-relative"));
        draft.set_video("assets/screens/a.avi", 30.0, 2.0);
        assert!(add(&source, 0, draft.to_cue().unwrap()).is_err());
        draft.set_video("assets/screens/a.mp4", 30.0, 2.0);
        draft.trim_start = 3.0;
        assert!(add(&source, 0, draft.to_cue().unwrap())
            .unwrap_err()
            .contains("trim_start"));
        draft.trim_start = 0.0;
        draft.taps = "1,x".into();
        assert!(draft.to_cue().unwrap_err().contains("\"x\""));
        draft.taps = "1.5, 0.5".into();
        assert!(add(&source, 0, draft.to_cue().unwrap())
            .unwrap_err()
            .contains("taps"));
        draft.taps.clear();
        draft.at = 1.0e6;
        assert!(add(&source, 0, draft.to_cue().unwrap()).is_err());
        assert!(Draft::new("", "", 0.0).to_cue().is_err());
        // Two cues for one slot at the same time are rejected.
        draft.at = 0.0;
        let once = add(&source, 0, draft.to_cue().unwrap()).unwrap();
        assert!(add(&once, 0, draft.to_cue().unwrap())
            .unwrap_err()
            .contains("two screen cues"));
    }
}
