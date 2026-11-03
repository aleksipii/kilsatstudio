//! Set dressing: place stick-studio props into a scene's set, then select,
//! drag, resize, flip, move behind or in front of the characters and remove
//! them on the stage. The scene's time of day (lighting preset) is chosen here
//! too. A placed prop is ordinary artwork: its layers share the id prefix
//! `dress<N>_` and the prop's name, so every other artwork tool still works.
//! Each finished gesture is one undoable artwork transaction.
use crate::art_editor::Editor as ArtEditor;
use eframe::egui;
use motion_art::{ClipShape, Command, Content, Document, Layer, Plane, Point, Rect};
use std::sync::OnceLock;

/// `(id, name, artwork project)` of every stick-studio prop drawn as artwork, in
/// manifest order. Hand props (drawn as rig parts) are held with `holds:` instead.
/// A test keeps this list equal to `examples/stick-studio/manifest.json`.
pub(crate) const PROPS: [(&str, &str, &str); 33] = [
    (
        "viikate",
        "Noutajan viikate",
        include_str!("../../../examples/stick-studio/props/viikate.kilsat"),
    ),
    (
        "toimistotuoli",
        "Palaverituoli",
        include_str!("../../../examples/stick-studio/props/toimistotuoli.kilsat"),
    ),
    (
        "tyopoyta",
        "Työpöytä",
        include_str!("../../../examples/stick-studio/props/tyopoyta.kilsat"),
    ),
    (
        "kannettava",
        "Kannettava tietokone",
        include_str!("../../../examples/stick-studio/props/kannettava.kilsat"),
    ),
    (
        "ajopaivakirja",
        "Ajopäiväkirja",
        include_str!("../../../examples/stick-studio/props/ajopaivakirja.kilsat"),
    ),
    (
        "kello",
        "Myöhässä oleva kello",
        include_str!("../../../examples/stick-studio/props/kello.kilsat"),
    ),
    (
        "liikennemerkki",
        "Yövuoron pysäkkimerkki",
        include_str!("../../../examples/stick-studio/props/liikennemerkki.kilsat"),
    ),
    (
        "paketti",
        "Salaperäinen paketti",
        include_str!("../../../examples/stick-studio/props/paketti.kilsat"),
    ),
    (
        "sateenvarjo",
        "Yövuoron sateenvarjo",
        include_str!("../../../examples/stick-studio/props/sateenvarjo.kilsat"),
    ),
    (
        "auto",
        "Veikon pieni auto",
        include_str!("../../../examples/stick-studio/props/auto.kilsat"),
    ),
    (
        "vuoronumerolaite",
        "Ikuisen jonon vuoronumerolaite",
        include_str!("../../../examples/stick-studio/props/vuoronumerolaite.kilsat"),
    ),
    (
        "henkiloauto",
        "Henkilöauto",
        include_str!("../../../examples/stick-studio/props/henkiloauto.kilsat"),
    ),
    (
        "taksi",
        "Taksi",
        include_str!("../../../examples/stick-studio/props/taksi.kilsat"),
    ),
    (
        "poliisiauto",
        "Poliisiauto",
        include_str!("../../../examples/stick-studio/props/poliisiauto.kilsat"),
    ),
    (
        "pakettiauto",
        "Pakettiauto",
        include_str!("../../../examples/stick-studio/props/pakettiauto.kilsat"),
    ),
    (
        "ambulanssi",
        "Ambulanssi",
        include_str!("../../../examples/stick-studio/props/ambulanssi.kilsat"),
    ),
    (
        "linja_auto",
        "Kaupunkibussi",
        include_str!("../../../examples/stick-studio/props/linja_auto.kilsat"),
    ),
    (
        "paloauto",
        "Paloauto",
        include_str!("../../../examples/stick-studio/props/paloauto.kilsat"),
    ),
    (
        "skootteri",
        "Skootteri",
        include_str!("../../../examples/stick-studio/props/skootteri.kilsat"),
    ),
    (
        "liikennevalo",
        "Liikennevalo",
        include_str!("../../../examples/stick-studio/props/liikennevalo.kilsat"),
    ),
    (
        "paloposti",
        "Paloposti",
        include_str!("../../../examples/stick-studio/props/paloposti.kilsat"),
    ),
    (
        "pysakointimittari",
        "Pysäköintimittari",
        include_str!("../../../examples/stick-studio/props/pysakointimittari.kilsat"),
    ),
    (
        "pankkiautomaatti",
        "Pankkiautomaatti",
        include_str!("../../../examples/stick-studio/props/pankkiautomaatti.kilsat"),
    ),
    (
        "juoma_automaatti",
        "Juoma-automaatti",
        include_str!("../../../examples/stick-studio/props/juoma_automaatti.kilsat"),
    ),
    (
        "katupuu",
        "Katupuu",
        include_str!("../../../examples/stick-studio/props/katupuu.kilsat"),
    ),
    (
        "puistonpenkki",
        "Puistonpenkki",
        include_str!("../../../examples/stick-studio/props/puistonpenkki.kilsat"),
    ),
    (
        "suojatiemerkki",
        "Suojatiemerkki",
        include_str!("../../../examples/stick-studio/props/suojatiemerkki.kilsat"),
    ),
    (
        "omakotitalo",
        "Omakotitalo",
        include_str!("../../../examples/stick-studio/props/omakotitalo.kilsat"),
    ),
    (
        "kerrostalo",
        "Kerrostalo",
        include_str!("../../../examples/stick-studio/props/kerrostalo.kilsat"),
    ),
    (
        "kulmakauppa",
        "Kulmakauppa",
        include_str!("../../../examples/stick-studio/props/kulmakauppa.kilsat"),
    ),
    (
        "pilvenpiirtaja",
        "Pilvenpiirtäjä",
        include_str!("../../../examples/stick-studio/props/pilvenpiirtaja.kilsat"),
    ),
    (
        "polttoainemittari",
        "Polttoainemittari",
        include_str!("../../../examples/stick-studio/props/polttoainemittari.kilsat"),
    ),
    (
        "torikoju",
        "Torikoju",
        include_str!("../../../examples/stick-studio/props/torikoju.kilsat"),
    ),
];

const PREFIX: &str = "dress";

/// One prop's artwork, ready to place.
pub(crate) struct Prop {
    pub id: &'static str,
    pub name: &'static str,
    layers: Vec<Layer>,
    bounds: Rect,
}

fn content_rect(content: &Content) -> Option<Rect> {
    match content {
        Content::Rectangle { rect, .. } | Content::Ellipse { rect, .. } => Some(*rect),
        Content::Stroke { points, width, .. } => {
            let xs = points.iter().map(|p| p.x);
            let ys = points.iter().map(|p| p.y);
            let (x0, x1) = (
                xs.clone().fold(f64::INFINITY, f64::min),
                xs.fold(f64::NEG_INFINITY, f64::max),
            );
            let (y0, y1) = (
                ys.clone().fold(f64::INFINITY, f64::min),
                ys.fold(f64::NEG_INFINITY, f64::max),
            );
            let half = width / 2.0;
            x0.is_finite().then_some(Rect {
                x: x0 - half,
                y: y0 - half,
                width: x1 - x0 + 2.0 * half,
                height: y1 - y0 + 2.0 * half,
            })
        }
        Content::Image { rect, .. } | Content::AnimatedImage { rect, .. } => Some(*rect),
    }
}

fn union(a: Option<Rect>, b: Rect) -> Rect {
    match a {
        None => b,
        Some(a) => {
            let x = a.x.min(b.x);
            let y = a.y.min(b.y);
            Rect {
                x,
                y,
                width: (a.x + a.width).max(b.x + b.width) - x,
                height: (a.y + a.height).max(b.y + b.height) - y,
            }
        }
    }
}

fn bounds<'a>(layers: impl IntoIterator<Item = &'a Layer>) -> Option<Rect> {
    layers
        .into_iter()
        .filter_map(|l| content_rect(&l.content))
        .fold(None, |acc, r| Some(union(acc, r)))
}

/// Every placeable prop (decoded once).
pub(crate) fn props() -> &'static [Prop] {
    static PROPS_CACHE: OnceLock<Vec<Prop>> = OnceLock::new();
    PROPS_CACHE.get_or_init(|| {
        PROPS
            .iter()
            .filter_map(|(id, name, project)| {
                let document = motion_project::decode(project.as_bytes()).ok()?;
                let layers = document.art.layers;
                let bounds = bounds(&layers)?;
                Some(Prop {
                    id,
                    name,
                    layers,
                    bounds,
                })
            })
            .collect()
    })
}

/// `layer` scaled by `s` about the origin, then moved by `(tx, ty)`.
fn transformed(layer: &Layer, s: f64, tx: f64, ty: f64) -> Result<Layer, String> {
    let point = |p: &Point| Point {
        x: p.x * s + tx,
        y: p.y * s + ty,
    };
    let rect = |r: &Rect| Rect {
        x: r.x * s + tx,
        y: r.y * s + ty,
        width: r.width * s,
        height: r.height * s,
    };
    let content = match &layer.content {
        Content::Rectangle { rect: r, color } => Content::Rectangle {
            rect: rect(r),
            color: *color,
        },
        Content::Ellipse { rect: r, color } => Content::Ellipse {
            rect: rect(r),
            color: *color,
        },
        Content::Stroke {
            points,
            color,
            width,
        } => Content::Stroke {
            points: points.iter().map(point).collect(),
            color: *color,
            width: width * s,
        },
        Content::Image { asset_id, rect: r } => Content::Image {
            asset_id: asset_id.clone(),
            rect: rect(r),
        },
        Content::AnimatedImage {
            frames,
            rect: r,
            playback,
        } => Content::AnimatedImage {
            frames: frames.clone(),
            rect: rect(r),
            playback: *playback,
        },
    };
    let clip_shape = match &layer.clip_shape {
        None => None,
        Some(ClipShape::Ellipse { rect: r }) => Some(ClipShape::Ellipse { rect: rect(r) }),
        Some(ClipShape::Polygon {
            points,
            feather,
            motion,
        }) if motion.is_empty() => Some(ClipShape::Polygon {
            points: points.iter().map(point).collect(),
            feather: feather * s,
            motion: Vec::new(),
        }),
        Some(_) => return Err(format!("taso {}: tätä maskia ei voi siirtää", layer.id)),
    };
    Ok(Layer {
        content,
        clip: layer.clip.as_ref().map(rect),
        clip_shape,
        ..layer.clone()
    })
}

/// A placed prop in a scene.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Placed {
    /// `dress<N>`.
    pub unit: String,
    pub name: String,
    pub bounds: Rect,
    pub plane: Plane,
}

fn unit_of(id: &str) -> Option<&str> {
    let (unit, _) = id.split_once('_')?;
    let number = unit.strip_prefix(PREFIX)?;
    (!number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())).then_some(unit)
}

/// The props placed in `scene_id`, in drawing order (topmost last).
pub(crate) fn placed(document: &Document, scene_id: &str) -> Vec<Placed> {
    let mut out: Vec<Placed> = Vec::new();
    for layer in document.layers.iter().filter(|l| l.scene_id == scene_id) {
        let Some(unit) = unit_of(&layer.id) else {
            continue;
        };
        let Some(rect) = content_rect(&layer.content) else {
            continue;
        };
        match out.iter_mut().find(|p| p.unit == unit) {
            Some(p) => p.bounds = union(Some(p.bounds), rect),
            None => out.push(Placed {
                unit: unit.to_string(),
                name: layer.name.clone(),
                bounds: rect,
                plane: layer.plane,
            }),
        }
    }
    out
}

fn next_unit(document: &Document) -> String {
    let n = document
        .layers
        .iter()
        .filter_map(|l| unit_of(&l.id))
        .filter_map(|u| u[PREFIX.len()..].parse::<u64>().ok())
        .max()
        .map_or(1, |n| n + 1);
    format!("{PREFIX}{n}")
}

/// Place `prop` with its feet (bottom centre) at `foot`, `height` tall.
pub(crate) fn place(
    document: &Document,
    scene_id: &str,
    prop: &Prop,
    foot: Point,
    height: f64,
    plane: Plane,
) -> Result<(Document, String), String> {
    if !height.is_finite() || height <= 0.0 {
        return Err("Anna esineelle positiivinen korkeus.".into());
    }
    let s = height / prop.bounds.height;
    let tx = foot.x - (prop.bounds.x + prop.bounds.width / 2.0) * s;
    let ty = foot.y - (prop.bounds.y + prop.bounds.height) * s;
    let unit = next_unit(document);
    let mut next = document.clone();
    for (index, layer) in prop.layers.iter().enumerate() {
        let mut layer = transformed(layer, s, tx, ty)?;
        layer.id = format!("{unit}_{index}");
        layer.scene_id = scene_id.to_string();
        layer.name = prop.name.to_string();
        layer.plane = plane;
        layer.attachment = None;
        layer.angle = None;
        layer.active = None;
        next.apply(Command::Add(layer)).map_err(|e| e.to_string())?;
    }
    next.validate().map_err(|e| e.to_string())?;
    Ok((next, unit))
}

/// Apply `f` to every layer of `unit` and validate.
fn edit_unit(
    document: &Document,
    scene_id: &str,
    unit: &str,
    mut f: impl FnMut(&Layer) -> Result<Layer, String>,
) -> Result<Document, String> {
    let mut next = document.clone();
    let mut found = false;
    for layer in next.layers.iter_mut() {
        if layer.scene_id == scene_id && unit_of(&layer.id) == Some(unit) {
            *layer = f(layer)?;
            found = true;
        }
    }
    if !found {
        return Err(format!("Esinettä {unit} ei löydy tästä kohtauksesta."));
    }
    next.validate().map_err(|e| e.to_string())?;
    Ok(next)
}

fn unit_bounds(document: &Document, scene_id: &str, unit: &str) -> Result<Rect, String> {
    placed(document, scene_id)
        .into_iter()
        .find(|p| p.unit == unit)
        .map(|p| p.bounds)
        .ok_or_else(|| format!("Esinettä {unit} ei löydy tästä kohtauksesta."))
}

pub(crate) fn moved(
    document: &Document,
    scene_id: &str,
    unit: &str,
    dx: f64,
    dy: f64,
) -> Result<Document, String> {
    edit_unit(document, scene_id, unit, |l| transformed(l, 1.0, dx, dy))
}

/// Resize by `factor` about the prop's feet (bottom centre).
pub(crate) fn resized(
    document: &Document,
    scene_id: &str,
    unit: &str,
    factor: f64,
) -> Result<Document, String> {
    if !factor.is_finite() || factor <= 0.0 {
        return Err("Koon kertoimen on oltava positiivinen.".into());
    }
    let b = unit_bounds(document, scene_id, unit)?;
    let (fx, fy) = (b.x + b.width / 2.0, b.y + b.height);
    edit_unit(document, scene_id, unit, |l| {
        transformed(l, factor, fx - fx * factor, fy - fy * factor)
    })
}

/// Mirror left to right about the prop's centre.
pub(crate) fn flipped(document: &Document, scene_id: &str, unit: &str) -> Result<Document, String> {
    let b = unit_bounds(document, scene_id, unit)?;
    let axis = b.x + b.width / 2.0;
    edit_unit(document, scene_id, unit, |l| {
        l.mirrored_x(axis).map_err(|e| e.to_string())
    })
}

pub(crate) fn with_plane(
    document: &Document,
    scene_id: &str,
    unit: &str,
    plane: Plane,
) -> Result<Document, String> {
    edit_unit(document, scene_id, unit, |l| {
        Ok(Layer { plane, ..l.clone() })
    })
}

pub(crate) fn removed(document: &Document, scene_id: &str, unit: &str) -> Result<Document, String> {
    let mut next = document.clone();
    let ids: Vec<String> = next
        .layers
        .iter()
        .filter(|l| l.scene_id == scene_id && unit_of(&l.id) == Some(unit))
        .map(|l| l.id.clone())
        .collect();
    if ids.is_empty() {
        return Err(format!("Esinettä {unit} ei löydy tästä kohtauksesta."));
    }
    for id in ids {
        next.apply(Command::Remove { id })
            .map_err(|e| e.to_string())?;
    }
    Ok(next)
}

struct Drag {
    original: Document,
    start: [f64; 2],
}

pub(crate) struct SetDressing {
    /// Stage clicks and drags dress the set instead of drawing.
    pub active: bool,
    search: String,
    prop: usize,
    height: f64,
    plane: Plane,
    placing: bool,
    pub selected: Option<String>,
    drag: Option<Drag>,
    pub status: Option<String>,
}
impl Default for SetDressing {
    fn default() -> Self {
        Self {
            active: false,
            search: String::new(),
            prop: 0,
            height: 260.0,
            plane: Plane::Background,
            placing: false,
            selected: None,
            drag: None,
            status: None,
        }
    }
}

fn plane_label(plane: Plane) -> &'static str {
    match plane {
        Plane::Background => "Hahmojen takana",
        Plane::Foreground => "Hahmojen edessä",
    }
}

impl SetDressing {
    fn commit(
        &mut self,
        editor: &mut ArtEditor,
        art: &mut Document,
        result: Result<Document, String>,
    ) {
        match result {
            Ok(next) => {
                editor.commit(art, next);
                self.status = editor.error.clone();
            }
            Err(error) => self.status = Some(error),
        }
    }

    /// Side panel: time of day, prop picker and the selected prop's controls.
    pub(crate) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        art: &mut Document,
        scene_id: &str,
        width: u32,
        height: u32,
        editor: &mut ArtEditor,
    ) {
        egui::CollapsingHeader::new("Lavastus ja vuorokaudenaika")
            .default_open(true)
            .show(ui, |ui| {
                let time = motion_art::scene_time_of_day(art, scene_id);
                let mut chosen = time;
                egui::ComboBox::from_label("Vuorokaudenaika")
                    .selected_text(time.label())
                    .show_ui(ui, |ui| {
                        for option in motion_art::TimeOfDay::ALL {
                            ui.selectable_value(&mut chosen, option, option.label());
                        }
                    });
                if chosen != time {
                    let next = motion_art::with_time_of_day(art, scene_id, chosen)
                        .map_err(|e| e.to_string());
                    self.commit(editor, art, next);
                }
                ui.small("Aamu, ilta ja yö sävyttävät koko kuvan hahmoineen kohtauksen valoilla. Valot näkyvät myös Kuvitus-paneelin valoluettelossa.");
                ui.separator();
                ui.checkbox(&mut self.active, "Lavastustila: näyttämön napsautus ja veto siirtävät esineitä");
                ui.horizontal(|ui| {
                    ui.label("Hae");
                    ui.text_edit_singleline(&mut self.search);
                });
                let needle = self.search.to_lowercase();
                let props = props();
                let current = props.get(self.prop).map_or("", |p| p.name);
                egui::ComboBox::from_label("Esine")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for (index, prop) in props.iter().enumerate() {
                            if needle.is_empty()
                                || prop.name.to_lowercase().contains(&needle)
                                || prop.id.contains(&needle)
                            {
                                ui.selectable_value(&mut self.prop, index, prop.name);
                            }
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("Korkeus");
                    ui.add(egui::DragValue::new(&mut self.height).range(10.0..=2000.0).suffix(" px"));
                    for plane in [Plane::Background, Plane::Foreground] {
                        ui.selectable_value(&mut self.plane, plane, plane_label(plane));
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Lisää keskelle lattiaa").clicked() {
                        if let Some(prop) = props.get(self.prop) {
                            let foot = Point {
                                x: f64::from(width) / 2.0,
                                y: f64::from(height) * 0.82,
                            };
                            match place(art, scene_id, prop, foot, self.height, self.plane) {
                                Ok((next, unit)) => {
                                    editor.commit(art, next);
                                    self.selected = Some(unit);
                                    self.status = editor.error.clone();
                                }
                                Err(error) => self.status = Some(error),
                            }
                        }
                    }
                    if ui
                        .selectable_label(self.placing, "Sijoita napsauttamalla")
                        .clicked()
                    {
                        self.placing = !self.placing;
                        self.active |= self.placing;
                    }
                });
                if self.placing {
                    ui.small("Napsauta näyttämöä: esineen alareuna asettuu napsautuskohtaan.");
                }
                let placed = placed(art, scene_id);
                if self
                    .selected
                    .as_ref()
                    .is_some_and(|s| !placed.iter().any(|p| &p.unit == s))
                {
                    self.selected = None;
                }
                ui.label(format!("Kohtauksen lavastus: {} esinettä", placed.len()));
                for p in &placed {
                    let label = format!("{} · {}", p.name, plane_label(p.plane));
                    if ui
                        .selectable_label(self.selected.as_ref() == Some(&p.unit), label)
                        .clicked()
                    {
                        self.selected = Some(p.unit.clone());
                    }
                }
                if let Some(unit) = self.selected.clone() {
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Pienennä").clicked() {
                            let next = resized(art, scene_id, &unit, 1.0 / 1.1);
                            self.commit(editor, art, next);
                        }
                        if ui.button("Suurenna").clicked() {
                            let next = resized(art, scene_id, &unit, 1.1);
                            self.commit(editor, art, next);
                        }
                        if ui.button("Peilaa").clicked() {
                            let next = flipped(art, scene_id, &unit);
                            self.commit(editor, art, next);
                        }
                        let plane = placed
                            .iter()
                            .find(|p| p.unit == unit)
                            .map_or(Plane::Background, |p| p.plane);
                        let other = match plane {
                            Plane::Background => Plane::Foreground,
                            Plane::Foreground => Plane::Background,
                        };
                        if ui.button(format!("Siirrä: {}", plane_label(other))).clicked() {
                            let next = with_plane(art, scene_id, &unit, other);
                            self.commit(editor, art, next);
                        }
                        if ui.button("Poista").clicked() {
                            let next = removed(art, scene_id, &unit);
                            self.commit(editor, art, next);
                            self.selected = None;
                        }
                    });
                }
                if let Some(status) = &self.status {
                    ui.colored_label(ui.visuals().warn_fg_color, status);
                }
            });
    }

    /// Stage gesture while dressing: place, select and drag props.
    pub(crate) fn gesture(
        &mut self,
        response: &egui::Response,
        rect: egui::Rect,
        scene: &motion_scene::EvaluatedScene,
        scene_id: &str,
        art: &mut Document,
        editor: &mut ArtEditor,
    ) {
        let world = |pos: egui::Pos2| crate::direct_manipulation::canvas_to_world(rect, scene, pos);
        let hit = |art: &Document, p: [f64; 2]| {
            placed(art, scene_id)
                .into_iter()
                .rev()
                .find(|u| {
                    p[0] >= u.bounds.x
                        && p[0] <= u.bounds.x + u.bounds.width
                        && p[1] >= u.bounds.y
                        && p[1] <= u.bounds.y + u.bounds.height
                })
                .map(|u| u.unit)
        };
        if response.clicked() {
            let Some(p) = response.interact_pointer_pos().and_then(world) else {
                return;
            };
            if self.placing {
                self.placing = false;
                if let Some(prop) = props().get(self.prop) {
                    let foot = Point { x: p[0], y: p[1] };
                    match place(art, scene_id, prop, foot, self.height, self.plane) {
                        Ok((next, unit)) => {
                            editor.commit(art, next);
                            self.selected = Some(unit);
                            self.status = editor.error.clone();
                        }
                        Err(error) => self.status = Some(error),
                    }
                }
            } else {
                self.selected = hit(art, p);
            }
            return;
        }
        if response.drag_started() {
            if let Some(p) = response.interact_pointer_pos().and_then(world) {
                if let Some(unit) = hit(art, p) {
                    self.selected = Some(unit);
                    self.drag = Some(Drag {
                        original: art.clone(),
                        start: p,
                    });
                }
            }
        }
        if let (Some(drag), Some(unit)) = (&self.drag, &self.selected) {
            if response.dragged() {
                if let Some(p) = response.interact_pointer_pos().and_then(world) {
                    if let Ok(next) = moved(
                        &drag.original,
                        scene_id,
                        unit,
                        p[0] - drag.start[0],
                        p[1] - drag.start[1],
                    ) {
                        // Live preview without history; the drop is one transaction.
                        *art = next;
                        editor.touch();
                    }
                }
            }
        }
        if response.drag_stopped() {
            if let Some(drag) = self.drag.take() {
                let moved = std::mem::replace(art, drag.original);
                editor.commit(art, moved);
            }
        }
    }

    /// Outline the selected prop on the stage.
    pub(crate) fn paint(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        scene: &motion_scene::EvaluatedScene,
        art: &Document,
        scene_id: &str,
    ) {
        let Some(unit) = &self.selected else {
            return;
        };
        let Some(p) = placed(art, scene_id).into_iter().find(|p| &p.unit == unit) else {
            return;
        };
        let b = &p.bounds;
        let corner = |x: f64, y: f64| crate::direct_manipulation::screen_point(rect, scene, x, y);
        let outline =
            egui::Rect::from_two_pos(corner(b.x, b.y), corner(b.x + b.width, b.y + b.height));
        painter.rect_stroke(
            outline,
            2.0,
            egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(255, 196, 0)),
            egui::StrokeKind::Outside,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prop(id: &str) -> &'static Prop {
        props().iter().find(|p| p.id == id).unwrap()
    }

    #[test]
    fn the_embedded_list_matches_the_pack_manifest_and_every_prop_decodes() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../../examples/stick-studio/manifest.json"))
                .unwrap();
        let ids: Vec<&str> = manifest["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["kind"] == "prop")
            .filter(|i| {
                // Hand props are rig parts with no artwork layers.
                let path = format!(
                    "{}/../../examples/stick-studio/{}",
                    env!("CARGO_MANIFEST_DIR"),
                    i["project"].as_str().unwrap()
                );
                let project: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
                !project["art"]["layers"].as_array().unwrap().is_empty()
            })
            .map(|i| i["id"].as_str().unwrap())
            .collect();
        let listed: Vec<&str> = PROPS.iter().map(|p| p.0).collect();
        assert_eq!(listed, ids, "regenerate PROPS from the manifest");
        assert_eq!(props().len(), PROPS.len());
        for p in props() {
            assert!(p.bounds.width > 0.0 && p.bounds.height > 0.0, "{}", p.id);
        }
    }

    #[test]
    fn placing_scales_to_height_and_stands_on_the_foot_point() {
        let art = Document::default();
        let bench = prop("puistonpenkki");
        let (art, unit) = place(
            &art,
            "s",
            bench,
            Point { x: 500.0, y: 800.0 },
            120.0,
            Plane::Background,
        )
        .unwrap();
        assert_eq!(unit, "dress1");
        let p = &placed(&art, "s")[0];
        assert!((p.bounds.height - 120.0).abs() < 1e-6);
        assert!((p.bounds.y + p.bounds.height - 800.0).abs() < 1e-6);
        assert!((p.bounds.x + p.bounds.width / 2.0 - 500.0).abs() < 1e-6);
        assert_eq!(p.name, bench.name);
        assert!(placed(&art, "other").is_empty());
        let (art, second) = place(
            &art,
            "s",
            prop("katupuu"),
            Point { x: 100.0, y: 800.0 },
            300.0,
            Plane::Foreground,
        )
        .unwrap();
        assert_eq!(second, "dress2");
        assert_eq!(placed(&art, "s").len(), 2);
        assert!(place(
            &art,
            "s",
            bench,
            Point { x: 0.0, y: 0.0 },
            0.0,
            Plane::Background
        )
        .is_err());
    }

    #[test]
    fn move_resize_flip_plane_and_remove_keep_the_prop_whole() {
        let (art, unit) = place(
            &Document::default(),
            "s",
            prop("taksi"),
            Point { x: 600.0, y: 700.0 },
            200.0,
            Plane::Background,
        )
        .unwrap();
        let before = placed(&art, "s")[0].bounds;
        let art = moved(&art, "s", &unit, 50.0, -10.0).unwrap();
        let after = placed(&art, "s")[0].bounds;
        assert!(
            (after.x - before.x - 50.0).abs() < 1e-9 && (after.y - before.y + 10.0).abs() < 1e-9
        );
        let art = resized(&art, "s", &unit, 2.0).unwrap();
        let big = placed(&art, "s")[0].bounds;
        assert!((big.height - 2.0 * after.height).abs() < 1e-6);
        assert!(
            (big.y + big.height - (after.y + after.height)).abs() < 1e-6,
            "feet stay on the floor"
        );
        let art = flipped(&art, "s", &unit).unwrap();
        let mirrored = placed(&art, "s")[0].bounds;
        assert!((mirrored.x - big.x).abs() < 1e-6 && (mirrored.width - big.width).abs() < 1e-6);
        let art = with_plane(&art, "s", &unit, Plane::Foreground).unwrap();
        assert_eq!(placed(&art, "s")[0].plane, Plane::Foreground);
        let art = removed(&art, "s", &unit).unwrap();
        assert!(placed(&art, "s").is_empty());
        assert!(art.layers.is_empty());
        assert!(moved(&art, "s", &unit, 1.0, 1.0).is_err());
    }

    #[test]
    fn every_prop_places_into_a_valid_document() {
        let mut art = Document::default();
        for (i, p) in props().iter().enumerate() {
            let foot = Point {
                x: 100.0 + 40.0 * i as f64,
                y: 900.0,
            };
            art = place(&art, "s", p, foot, 150.0, Plane::Background)
                .unwrap()
                .0;
        }
        assert_eq!(placed(&art, "s").len(), props().len());
        art.validate().unwrap();
    }

    #[test]
    fn panel_draws_headlessly() {
        let mut dressing = SetDressing::default();
        let mut editor = ArtEditor::default();
        let mut art = Document::default();
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                dressing.ui(ui, &mut art, "s", 1920, 1080, &mut editor);
            });
        });
        assert!(art.layers.is_empty());
    }
}
