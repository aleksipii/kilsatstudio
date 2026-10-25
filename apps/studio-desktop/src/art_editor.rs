//! Artwork edits expose one transaction to the shared project history.
use eframe::egui;
use motion_art::{
    BlendMode, ClipShape, Color, Content, Document, Layer, MaskKey, MaskMotionKey, MaskStroke,
    Plane, Point, Rect, TimeRange,
};
use std::collections::BTreeMap;

fn blend_label(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Normal => "Normaali",
        BlendMode::Multiply => "Kertova (Multiply)",
        BlendMode::Screen => "Vaalentava (Screen)",
    }
}

fn plane_label(plane: Plane) -> &'static str {
    match plane {
        Plane::Background => "Tausta",
        Plane::Foreground => "Etuala",
    }
}

fn mask_channel_label(channel: motion_art::MaskChannel) -> &'static str {
    match channel {
        motion_art::MaskChannel::Alpha => "Läpinäkyvyys (alfa)",
        motion_art::MaskChannel::Luminance => "Kirkkaus",
    }
}

pub struct SceneContext<'a> {
    pub id: &'a str,
    pub duration: f64,
    pub width: u32,
    pub height: u32,
}
#[derive(Default, PartialEq, Clone, Copy)]
pub enum Tool {
    #[default]
    Select,
    Pen,
    Rectangle,
    Ellipse,
    MaskPaint,
    MaskErase,
}
struct MaskGesture {
    layer: Layer,
    scene: String,
    tool: Tool,
    camera: (f64, f64, f64, f64),
    viewport: egui::Rect,
    scene_size: (u32, u32),
    points: Vec<Point>,
    radius: f64,
    hardness: f64,
    pressure: PressureCapture,
}
#[derive(Default)]
struct PressureCapture {
    enabled: bool,
    contact: Option<(egui::TouchDeviceId, egui::TouchId)>,
    samples: Vec<f64>,
    last: Option<f64>,
    received_force: bool,
}
impl PressureCapture {
    fn begin(enabled: bool, events: &[egui::Event], position: Option<egui::Pos2>) -> Self {
        let position = events
            .iter()
            .find_map(|event| match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } => Some(*pos),
                _ => None,
            })
            .or(position);
        let contacts: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                egui::Event::Touch {
                    device_id,
                    id,
                    phase: egui::TouchPhase::Start,
                    pos,
                    ..
                } if Some(*pos) == position => Some((*device_id, *id)),
                _ => None,
            })
            .collect();
        Self {
            enabled,
            contact: (enabled && contacts.len() == 1).then(|| contacts[0]),
            ..Self::default()
        }
    }
    /// Only the initiating touch can supply force. Ambiguous multitouch cancels.
    fn update(&mut self, events: &[egui::Event], position: Option<egui::Pos2>) -> bool {
        let Some(contact) = self.contact else {
            return true;
        };
        for event in events {
            if let egui::Event::Touch {
                device_id,
                id,
                phase,
                pos,
                force,
            } = event
            {
                if (*device_id, *id) != contact || *phase == egui::TouchPhase::Cancel {
                    return false;
                }
                if Some(*pos) != position {
                    continue;
                }
                if let Some(force) =
                    force.filter(|force| force.is_finite() && (0.0..=1.0).contains(force))
                {
                    self.last = Some(f64::from(force));
                    self.received_force = true;
                } else if *phase != egui::TouchPhase::End {
                    self.last = None;
                }
            }
        }
        true
    }
    fn value(&self) -> f64 {
        self.last.unwrap_or(1.0)
    }
}
pub struct Editor {
    pub tool: Tool,
    pub selected: Option<String>,
    pub dirty: bool,
    revision: u64,
    points: Vec<Point>,
    gesture_scene: String,
    transaction_before: Option<Document>,
    pub error: Option<String>,
    context: String,
    group_draft: Option<motion_art::Group>,
    move_parent: String,
    move_index: usize,
    mask_radius: f64,
    mask_hardness: f64,
    mask_pressure: bool,
    mask_gesture: Option<MaskGesture>,
}
impl Default for Editor {
    fn default() -> Self {
        Self {
            tool: Tool::default(),
            selected: None,
            dirty: false,
            revision: 0,
            points: Vec::new(),
            gesture_scene: String::new(),
            transaction_before: None,
            error: None,
            context: String::new(),
            group_draft: None,
            move_parent: String::new(),
            move_index: 0,
            mask_radius: 12.0,
            mask_hardness: 1.0,
            mask_pressure: false,
            mask_gesture: None,
        }
    }
}
fn parse_mask_points(text: &str) -> Result<Vec<Point>, String> {
    if text.len() > 16_384 {
        return Err("Maskin kärkipisteet ylittävät 16 KiB".into());
    }
    let mut points = Vec::new();
    for pair in text.split_whitespace() {
        if points.len() >= motion_art::MAX_MASK_POINTS {
            return Err("Käytä enintään 256 kärkipistettä".into());
        }
        let (x, y) = pair
            .split_once(',')
            .ok_or("Anna x,y-kärkipisteet välilyönneillä erotettuina")?;
        points.push(Point {
            x: x.parse().map_err(|_| "Virheellinen x-koordinaatti")?,
            y: y.parse().map_err(|_| "Virheellinen y-koordinaatti")?,
        });
    }
    ClipShape::Polygon {
        motion: Vec::new(),
        points: points.clone(),
        feather: 0.0,
    }
    .validate()
    .map_err(|e| e.to_string())?;
    Ok(points)
}
fn parse_motion_keys(text: &str, duration: f64) -> Result<Vec<MaskMotionKey>, String> {
    if text.len() > 24_576 {
        return Err("Liikeavainruudut ylittävät 24 KiB".into());
    }
    let mut keys = Vec::new();
    for row in text.split_whitespace() {
        if keys.len() >= motion_art::MAX_MASK_KEYS {
            return Err("Käytä enintään 128 liikeavainruutua".into());
        }
        let values: Vec<f64> = row
            .split(',')
            .map(|value| {
                value
                    .parse()
                    .map_err(|_| "Virheellinen liikeavainruudun luku")
            })
            .collect::<Result<_, _>>()?;
        if values.len() != 5 {
            return Err("Anna jokaiselle avainruudulle aika,x,y,mittakaava,kierto".into());
        }
        keys.push(MaskMotionKey {
            time: values[0],
            x: values[1],
            y: values[2],
            scale: values[3],
            rotation: values[4],
        });
    }
    ClipShape::Polygon {
        points: vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 1.0, y: 0.0 },
            Point { x: 0.0, y: 1.0 },
        ],
        feather: 0.0,
        motion: keys.clone(),
    }
    .validate()
    .map_err(|error| error.to_string())?;
    if !duration.is_finite()
        || duration <= 0.0
        || keys.last().is_some_and(|key| key.time > duration)
    {
        return Err("Liikeavainruutujen pitää mahtua kohtauksen kestoon".into());
    }
    Ok(keys)
}
fn motion_key_controls(
    ui: &mut egui::Ui,
    layer_id: &str,
    motion: &mut Vec<MaskMotionKey>,
    duration: f64,
) {
    ui.label("Maskin liikeavainruudut: aika,x,y,mittakaava,kierto(rad). Muoto liikkuu oman keskipisteensä suhteen. Aika on kohtauksen sekunteja; liike on tasainen ja päätyasennot säilyvät. Tyhjä = liikkumaton.");
    let id = ui.id().with(("mask-motion-keys", layer_id));
    let error_id = id.with("error");
    let initial = motion
        .iter()
        .map(|k| format!("{},{},{},{},{}", k.time, k.x, k.y, k.scale, k.rotation))
        .collect::<Vec<_>>()
        .join("\n");
    let mut draft = ui
        .data_mut(|data| data.get_temp::<String>(id))
        .unwrap_or(initial);
    ui.add(
        egui::TextEdit::multiline(&mut draft)
            .desired_rows(3)
            .char_limit(24_576),
    );
    if ui.button("Käytä maskin liikettä").clicked() {
        match parse_motion_keys(&draft, duration) {
            Ok(next) => {
                *motion = next;
                ui.data_mut(|data| data.remove::<String>(error_id));
            }
            Err(error) => ui.data_mut(|data| data.insert_temp(error_id, error)),
        }
    }
    if let Some(error) = ui.data_mut(|data| data.get_temp::<String>(error_id)) {
        ui.colored_label(egui::Color32::LIGHT_RED, error);
    }
    ui.data_mut(|data| data.insert_temp(id, draft));
}
fn parse_mask_keys(text: &str, duration: f64) -> Result<Vec<MaskKey>, String> {
    if text.len() > 24_576 {
        return Err("Maskin avainruudut ylittävät 24 KiB".into());
    }
    let mut keys = Vec::new();
    for row in text.split_whitespace() {
        if keys.len() >= motion_art::MAX_MASK_KEYS {
            return Err("Käytä enintään 128 maskin avainruutua".into());
        }
        let values: Vec<f64> = row
            .split(',')
            .map(|value| {
                value
                    .parse()
                    .map_err(|_| "Virheellinen maskin avainruudun luku")
            })
            .collect::<Result<_, _>>()?;
        if values.len() != 6 {
            return Err("Anna jokaiselle avainruudulle aika,x,y,leveys,korkeus,pehmennys".into());
        }
        keys.push(MaskKey {
            time: values[0],
            rect: Rect {
                x: values[1],
                y: values[2],
                width: values[3],
                height: values[4],
            },
            feather: values[5],
        });
    }
    ClipShape::SoftEllipse {
        rect: Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        feather: 0.0,
        keys: keys.clone(),
    }
    .validate()
    .map_err(|error| error.to_string())?;
    if !duration.is_finite()
        || duration <= 0.0
        || keys.last().is_some_and(|key| key.time > duration)
    {
        return Err("Maskin avainruutujen pitää mahtua kohtauksen kestoon".into());
    }
    Ok(keys)
}
fn parse_image_mask_keys(
    text: &str,
    duration: f64,
) -> Result<Vec<motion_art::RectMaskKey>, String> {
    if text.len() > 24_576 {
        return Err("Maskin avainruudut ylittävät 24 KiB".into());
    }
    let mut keys = Vec::new();
    for row in text.split_whitespace() {
        if keys.len() >= motion_art::MAX_MASK_KEYS {
            return Err("Käytä enintään 128 maskin avainruutua".into());
        }
        let values: Vec<f64> = row
            .split(',')
            .map(|value| {
                value
                    .parse()
                    .map_err(|_| "Virheellinen maskin avainruudun luku")
            })
            .collect::<Result<_, _>>()?;
        if values.len() != 5 {
            return Err("Anna jokaiselle avainruudulle aika,x,y,leveys,korkeus".into());
        }
        keys.push(motion_art::RectMaskKey {
            time: values[0],
            rect: Rect {
                x: values[1],
                y: values[2],
                width: values[3],
                height: values[4],
            },
        });
    }
    let shape = ClipShape::Image {
        asset_id: "mask".into(),
        rect: Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        channel: Default::default(),
        keys: keys.clone(),
    };
    shape.validate().map_err(|error| error.to_string())?;
    shape
        .validate_scene_duration(duration)
        .map_err(|error| error.to_string())?;
    Ok(keys)
}
fn compositing_controls(ui: &mut egui::Ui, layer: &mut Layer, duration: f64, images: &[&str]) {
    egui::ComboBox::from_label("Sekoitustapa")
        .selected_text(blend_label(layer.blend))
        .show_ui(ui, |ui| {
            for mode in [BlendMode::Normal, BlendMode::Multiply, BlendMode::Screen] {
                ui.selectable_value(&mut layer.blend, mode, blend_label(mode));
            }
        });
    let mut kind = match &layer.clip_shape {
        Some(ClipShape::Ellipse { .. }) => 2,
        Some(ClipShape::Polygon { .. }) => 3,
        Some(ClipShape::SoftEllipse { .. }) => 4,
        Some(ClipShape::Image { .. }) => 5,
        Some(ClipShape::Painted { .. }) => 6,
        None => {
            if layer.clip.is_some() {
                1
            } else {
                0
            }
        }
    };
    let previous = kind;
    egui::ComboBox::from_label("Rajausmaski")
        .selected_text(
            [
                "Ei rajausta",
                "Suorakulmio",
                "Ellipsi",
                "Monikulmio",
                "Pehmennetty / animoitu ellipsi",
                "Kuva (läpinäkyvyys / kirkkaus)",
                "Maalattu (sivellin / pyyhin)",
            ][kind],
        )
        .show_ui(ui, |ui| {
            for (value, label) in [
                "Ei rajausta",
                "Suorakulmio",
                "Ellipsi",
                "Monikulmio",
                "Pehmennetty / animoitu ellipsi",
                "Kuva (läpinäkyvyys / kirkkaus)",
                "Maalattu (sivellin / pyyhin)",
            ]
            .into_iter()
            .enumerate()
            {
                if value == 5 && images.is_empty() {
                    ui.add_enabled(false, egui::Label::new("Kuvamaski (tuo ensin kuva)"));
                    continue;
                }
                ui.selectable_value(&mut kind, value, label);
            }
        });
    if kind != previous {
        let rect = layer
            .clip
            .or(match &layer.clip_shape {
                Some(
                    ClipShape::Ellipse { rect }
                    | ClipShape::SoftEllipse { rect, .. }
                    | ClipShape::Image { rect, .. },
                ) => Some(*rect),
                _ => None,
            })
            .unwrap_or(Rect {
                x: 0.0,
                y: 0.0,
                width: 300.0,
                height: 300.0,
            });
        layer.clip = None;
        layer.clip_shape = None;
        match kind {
            6 => {
                layer.clip_shape = Some(ClipShape::Painted {
                    motion: Vec::new(),
                    base_visible: true,
                    strokes: Vec::new(),
                })
            }
            1 => layer.clip = Some(rect),
            2 => layer.clip_shape = Some(ClipShape::Ellipse { rect }),
            5 => {
                layer.clip_shape = Some(ClipShape::Image {
                    asset_id: images[0].to_string(),
                    rect,
                    channel: motion_art::MaskChannel::Alpha,
                    keys: Vec::new(),
                })
            }
            4 => {
                layer.clip_shape = Some(ClipShape::SoftEllipse {
                    rect,
                    feather: 0.25,
                    keys: Vec::new(),
                })
            }
            3 => {
                layer.clip_shape = Some(ClipShape::Polygon {
                    motion: Vec::new(),
                    feather: 0.0,
                    points: vec![
                        Point {
                            x: rect.x,
                            y: rect.y,
                        },
                        Point {
                            x: rect.x + rect.width,
                            y: rect.y,
                        },
                        Point {
                            x: rect.x + rect.width,
                            y: rect.y + rect.height,
                        },
                        Point {
                            x: rect.x,
                            y: rect.y + rect.height,
                        },
                    ],
                })
            }
            _ => {}
        }
    }
    let rect = match &mut layer.clip_shape {
        Some(
            ClipShape::Ellipse { rect }
            | ClipShape::SoftEllipse { rect, .. }
            | ClipShape::Image { rect, .. },
        ) => Some(rect),
        _ => layer.clip.as_mut(),
    };
    if let Some(rect) = rect {
        ui.horizontal(|ui| {
            for (name, value) in [
                ("X", &mut rect.x),
                ("Y", &mut rect.y),
                ("Leveys", &mut rect.width),
                ("Korkeus", &mut rect.height),
            ] {
                ui.label(name);
                ui.add(egui::DragValue::new(value).speed(1.0));
            }
        });
    }
    if let Some(ClipShape::Painted {
        base_visible,
        strokes,
        motion,
    }) = &mut layer.clip_shape
    {
        motion_key_controls(ui, &layer.id, motion, duration);
        ui.checkbox(
            base_visible,
            "Näytä aluksi koko sisältö (pois päältä: piilota kaikki)",
        );
        ui.label(format!(
            "{} järjestettyä siveltimen / pyyhkimen vetoa",
            strokes.len()
        ));
        ui.weak("Valitse Maskisivellin tai Maskipyyhin ja vedä tai napsauta näyttämöllä. Sivellin paljastaa, pyyhin piilottaa. Jokainen valmis veto on yksi kumottava muokkaus. Maskityypin vaihtaminen korvaa vedot. Esikatselu ja vienti käyttävät tässä CPU:ta.");
    }
    if let Some(ClipShape::Image {
        asset_id, channel, ..
    }) = &mut layer.clip_shape
    {
        egui::ComboBox::from_label("Maskin kuva")
            .selected_text(asset_id.as_str())
            .show_ui(ui, |ui| {
                for id in images {
                    ui.selectable_value(asset_id, id.to_string(), *id);
                }
            });
        egui::ComboBox::from_label("Maskin kanava")
            .selected_text(mask_channel_label(*channel))
            .show_ui(ui, |ui| {
                for value in [
                    motion_art::MaskChannel::Alpha,
                    motion_art::MaskChannel::Luminance,
                ] {
                    ui.selectable_value(channel, value, mask_channel_label(value));
                }
            });
        ui.weak("Kuva venytetään X/Y/leveys/korkeus-alueelle. Alfa: peittävät pikselit näyttävät sisällön. Kirkkaus: vaaleat pikselit näyttävät sisällön. Tavallinen esikatselu ja vienti käyttävät CPU:ta; kuvamaskeja voi käyttää myös erikseen valittavassa GPU-kokeessa.");
    }
    if let Some(ClipShape::Image { keys, .. }) = &mut layer.clip_shape {
        ui.label("Kuvamaskin avainruudut: aika,x,y,leveys,korkeus (kohtauksen sekuntia). Liike on tasainen ja päätyasennot säilyvät.");
        let initial = keys
            .iter()
            .map(|key| {
                format!(
                    "{},{},{},{},{}",
                    key.time, key.rect.x, key.rect.y, key.rect.width, key.rect.height
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let id = ui.id().with(("image-mask-keys", &layer.id, &initial));
        let error_id = id.with("error");
        let mut draft = ui
            .data_mut(|data| data.get_temp::<String>(id))
            .unwrap_or(initial);
        ui.add(
            egui::TextEdit::multiline(&mut draft)
                .desired_rows(4)
                .char_limit(24_576),
        );
        if ui.button("Käytä kuvamaskin avainruutuja").clicked() {
            match parse_image_mask_keys(&draft, duration) {
                Ok(next) => {
                    *keys = next;
                    ui.data_mut(|data| data.remove::<String>(error_id));
                }
                Err(error) => ui.data_mut(|data| data.insert_temp(error_id, error)),
            }
        }
        if let Some(error) = ui.data(|data| data.get_temp::<String>(error_id)) {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        ui.data_mut(|data| data.insert_temp(id, draft));
    }
    if let Some(ClipShape::Polygon {
        points,
        feather,
        motion,
    }) = &mut layer.clip_shape
    {
        motion_key_controls(ui, &layer.id, motion, duration);
        let id = ui.id().with(("mask-vertices", &layer.id));
        let initial = points
            .iter()
            .map(|p| format!("{},{}", p.x, p.y))
            .collect::<Vec<_>>()
            .join(" ");
        let mut draft = ui
            .data_mut(|d| d.get_temp::<String>(id))
            .unwrap_or(initial.clone());
        ui.label(
            "Kärkipisteet: 3–256 x,y-paria välilyönneillä erotettuina. Älä toista ensimmäistä pistettä.",
        );
        ui.add(
            egui::TextEdit::multiline(&mut draft)
                .desired_rows(3)
                .char_limit(16_384),
        );
        if ui.button("Käytä monikulmion kärkipisteitä").clicked() {
            match parse_mask_points(&draft) {
                Ok(next) => {
                    *points = next;
                }
                Err(error) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
            }
        }
        ui.data_mut(|d| d.insert_temp(id, draft));
        ui.add(
            egui::Slider::new(feather, 0.0..=motion_art::MAX_POLYGON_FEATHER)
                .logarithmic(true)
                .text("Reunan pehmennys (sumennus σ, kohtauksen yksikköä)"),
        );
        ui.weak("0 = terävä reuna. Positiivinen arvo sumentaa rajaa; reunalle jää noin puolet peittävyydestä. Tämä käyttää CPU-esikatselua ja -vientiä; tavallinen esikatselu ja vienti käyttävät CPU:ta. Kokeellinen GPU-polku tukee pehmennystä, mutta kulmat ja kapeat muodot voivat poiketa CPU-tuloksesta.");
    }
    if let Some(ClipShape::SoftEllipse { feather, keys, .. }) = &mut layer.clip_shape {
        ui.add(egui::Slider::new(feather, 0.0..=1.0).text("Pehmennys sisäänpäin"));
        ui.weak("0 = terävä reuna, 1 = häivytys keskipisteestä. Ilman avainruutuja käytetään perusmuotoa.");
        ui.label("Avainruudut: aika,x,y,leveys,korkeus,pehmennys (kohtauksen sekuntia). Väliarvot lasketaan tasaisesti, ja ensimmäinen sekä viimeinen asento säilyvät.");
        let id = ui.id().with(("soft-mask-keys", &layer.id));
        let error_id = id.with("error");
        let initial = keys
            .iter()
            .map(|key| {
                format!(
                    "{},{},{},{},{},{}",
                    key.time, key.rect.x, key.rect.y, key.rect.width, key.rect.height, key.feather
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut draft = ui
            .data_mut(|data| data.get_temp::<String>(id))
            .unwrap_or(initial);
        ui.add(
            egui::TextEdit::multiline(&mut draft)
                .desired_rows(4)
                .char_limit(24_576),
        );
        if ui.button("Käytä maskin avainruutuja").clicked() {
            match parse_mask_keys(&draft, duration) {
                Ok(next) => {
                    *keys = next;
                    ui.data_mut(|data| data.remove::<String>(error_id));
                }
                Err(error) => ui.data_mut(|data| data.insert_temp(error_id, error)),
            }
        }
        if let Some(error) = ui.data_mut(|data| data.get_temp::<String>(error_id)) {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        ui.data_mut(|data| data.insert_temp(id, draft));
    }
    if layer.clip_shape.is_some() || layer.blend != BlendMode::Normal {
        ui.weak("Tarkista maski ja sekoitustapa kohtauksen tai koko projektin esikatselusta tai PNG-/MP4-viennistä.");
    }
}

impl Editor {
    /// Increments only when document content changes, including history restoration.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn clear_history_for_context(&mut self, context: String) {
        if self.context != context {
            self.transaction_before = None;
            self.points.clear();
            self.mask_gesture = None;
            self.selected = None;
            self.group_draft = None;
            self.move_parent.clear();
            self.context = context;
        }
    }
    pub fn take_transaction_before(&mut self) -> Option<Document> {
        self.transaction_before.take()
    }
    pub fn reset(&mut self) {
        let revision = self.revision.wrapping_add(1);
        *self = Self::default();
        self.revision = revision;
    }
    pub(crate) fn commit(&mut self, document: &mut Document, mut candidate: Document) {
        if !candidate.groups.is_empty() || !candidate.order.is_empty() {
            for layer in &candidate.layers {
                if !document.layers.iter().any(|old| old.id == layer.id) {
                    let unit = motion_art::ArtUnit::Layer(layer.id.clone());
                    if !candidate.order.contains(&unit)
                        && !candidate.groups.iter().any(|g| g.children.contains(&unit))
                    {
                        candidate.order.push(unit);
                    }
                }
            }
        }
        if *document == candidate {
            return;
        }
        if let Err(error) = candidate.validate() {
            self.error = Some(error.to_string());
            return;
        }
        self.error = None;
        if self.transaction_before.is_none() {
            self.transaction_before = Some(document.clone());
        }
        *document = candidate;
        self.dirty = true;
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        document: &mut Document,
        scene: SceneContext<'_>,
        assets: &[motion_project::AssetReference],
        actors: &[String],
        textures: &BTreeMap<String, egui::TextureHandle>,
    ) {
        let scene_id = scene.id;
        egui::CollapsingHeader::new("Kuvitus ja tasot").show(ui,|ui| {
   ui.horizontal_wrapped(|ui| { for (tool,name) in [(Tool::Select,"Valitse"),(Tool::Pen,"Kynä"),(Tool::Rectangle,"Suorakulmio"),(Tool::Ellipse,"Ellipsi"),(Tool::MaskPaint,"Maskisivellin"),(Tool::MaskErase,"Maskipyyhin")] {if ui.selectable_value(&mut self.tool,tool,name).changed(){self.points.clear();self.mask_gesture=None;}} });
   if matches!(self.tool, Tool::MaskPaint | Tool::MaskErase) {
    if self.mask_radius <= 0.0 { self.mask_radius = 12.0; }
    ui.add(egui::Slider::new(&mut self.mask_radius, 0.5..=1000.0).logarithmic(true).text("Maskisiveltimen säde (kohtauksen yksikköä)"));
    ui.add(egui::Slider::new(&mut self.mask_hardness, 0.0..=1.0).text("Maskin kovuus (1 = terävä reuna)"));
    ui.checkbox(&mut self.mask_pressure, "Ohjaa maskin sädettä laitteen painetiedolla");
    ui.weak("Painetieto edellyttää laitteen ja sen ajurin tukea. Hiirellä tai ilman painetietoa käytetään täyttä sädettä. Piirtopöydän toimintaa tällä Macilla ei ole varmistettu.");
    ui.weak("Valitse ensin taso ja sille Maalattu-rajausmaski. Esc peruu keskeneräisen vedon.");
   }
   ui.weak("Piirrä vetämällä näyttämöllä. Taustatasot näkyvät hahmojen takana ja etualan tasot niiden edessä. Tallenna .kilsat-muodossa, jotta kuvitus säilyy.");
   self.group_controls(ui, document, scene_id);
   let mut next=document.clone();
   for layer in next.layers.iter_mut().filter(|l|l.scene_id==scene_id) { ui.horizontal(|ui| {ui.checkbox(&mut layer.visible,"");if ui.selectable_label(self.selected.as_deref()==Some(&layer.id),&layer.name).clicked(){self.selected=Some(layer.id.clone());}}); }
   if let Some(index)=next.layers.iter().position(|l|l.scene_id==scene_id && Some(&l.id)==self.selected.as_ref()) {
    let layer=&mut next.layers[index];
    ui.text_edit_singleline(&mut layer.name);
    egui::ComboBox::from_label("Sijoitus").selected_text(match layer.plane {Plane::Background=>"Tausta",Plane::Foreground=>"Etuala"}).show_ui(ui,|ui|{ui.selectable_value(&mut layer.plane,Plane::Background,"Tausta");ui.selectable_value(&mut layer.plane,Plane::Foreground,"Etuala");});
    let mut timed=layer.active.is_some();
    if ui.checkbox(&mut timed,"Näkyvissä aikavälillä").changed(){layer.active=timed.then_some(TimeRange{start:0.0,end:scene.duration});}
    if let Some(active)=&mut layer.active {ui.horizontal(|ui|{ui.label("Alku (s)");ui.add(egui::DragValue::new(&mut active.start).speed(0.1));ui.label("Loppu (s)");ui.add(egui::DragValue::new(&mut active.end).speed(0.1));});ui.weak(format!("Kohtaus: {:.2} s. Alkuhetki kuuluu aikaväliin, loppuhetki ei.",scene.duration));if !active.start.is_finite() || !active.end.is_finite() || active.start<0.0 || active.end<=active.start || active.end>scene.duration {ui.colored_label(egui::Color32::LIGHT_RED,"Käytä aikoja 0 ≤ alku < loppu ≤ kohtauksen kesto.");}}

    ui.add(egui::Slider::new(&mut layer.opacity,0.0..=1.0).text("Peittävyys"));
    if let Content::Image{rect,..}|Content::Rectangle{rect,..}|Content::Ellipse{rect,..}=&mut layer.content {ui.horizontal(|ui|{for(name,value)in[("X",&mut rect.x),("Y",&mut rect.y),("Leveys",&mut rect.width),("Korkeus",&mut rect.height)]{ui.label(name);ui.add(egui::DragValue::new(value).speed(1.0));}});rect.width=rect.width.max(1.0);rect.height=rect.height.max(1.0);}
    egui::ComboBox::from_label("Seuraa hahmon siirtoa").selected_text(layer.attachment.as_deref().unwrap_or("Kiinteä sijainti")).show_ui(ui,|ui| {ui.selectable_value(&mut layer.attachment,None,"Kiinteä sijainti");for actor in actors {ui.selectable_value(&mut layer.attachment,Some(actor.clone()),actor);}});
    let image_ids: Vec<&str> = assets.iter().filter(|a| a.kind == motion_project::AssetKind::Image).map(|a| a.id.as_str()).collect();
    compositing_controls(ui, layer, scene.duration, &image_ids);
    ui.horizontal(|ui| {if ui.add_enabled(index>0 && next.groups.is_empty() && next.order.is_empty(),egui::Button::new("Vie taakse")).clicked() { next.layers.swap(index,index-1); }
   if ui.add_enabled(index+1<next.layers.len() && next.groups.is_empty() && next.order.is_empty(),egui::Button::new("Tuo eteen")).clicked(){ next.layers.swap(index,index+1); }
   if ui.button("Poista taso").clicked(){let id=next.layers[index].id.clone(); if let Err(error)=next.apply(motion_art::Command::Remove{id}) {self.error=Some(error.to_string());} self.selected=None;}});
   }
   let images: Vec<_> = assets.iter().filter(|a| a.kind == motion_project::AssetKind::Image).collect();
   if !images.is_empty() {
    ui.label(format!("Kuvaresurssit ({})", images.len()));
    egui::ScrollArea::vertical().id_salt("art-image-inventory").max_height(256.0).show_rows(ui, 76.0, images.len(), |ui, rows| {
     for row in rows {let asset = images[row];
      ui.push_id(&asset.id, |ui| {ui.horizontal(|ui| {
       asset_thumbnail(ui, textures.get(&asset.id));
       ui.vertical(|ui| {ui.add(egui::Label::new(&asset.id).truncate()).on_hover_text(&asset.path);
        ui.vertical(|ui| {for (plane,label) in [(Plane::Foreground,"Lisää etualalle"),(Plane::Background,"Lisää taustalle")] {if ui.button(label).clicked(){let id=self.id(&next);self.selected=Some(id.clone());next.layers.push(Layer{id,name:asset.id.clone(),scene_id:scene_id.into(),visible:true,plane,active:None,opacity:1.0,attachment:None,clip_shape: None, blend: motion_art::BlendMode::Normal, clip:None,content:Content::Image{asset_id:asset.id.clone(),rect:if plane==Plane::Background {Rect{x:0.0,y:0.0,width:scene.width as f64,height:scene.height as f64}}else{Rect{x:50.0,y:50.0,width:250.0,height:250.0}}}});}}});
       });
      });});
     }
    });
   }
   if next.layers.iter().any(|l| l.scene_id==scene_id && l.active.as_ref().is_some_and(|a|a.end>scene.duration)) {self.error=Some("Kuvituksen aikaväli ylittää kohtauksen keston".into());} else {
   self.commit(document,next);
   }
   ui.weak("Kumoa / Tee uudelleen käyttää yläpalkin yhteistä projektihistoriaa.");
   if self.dirty {ui.label("Kuvituksessa on tallentamattomia muutoksia");}
   if let Some(error)=&self.error{ui.colored_label(egui::Color32::LIGHT_RED,error);}
  });
    }
    fn group_controls(&mut self, ui: &mut egui::Ui, document: &mut Document, scene_id: &str) {
        use motion_art::{ArtUnit, Command, Group};
        ui.collapsing("Ryhmät ja hierarkia", |ui| {
            if ui.button("Luo etualan ryhmä").clicked() {
                let id = (1..).map(|n| format!("group-{n}")).find(|id| !document.layers.iter().any(|l| &l.id == id) && !document.groups.iter().any(|g| &g.id == id)).unwrap();
                let mut candidate = document.clone();
                let group = Group { id: id.clone(), scene_id: scene_id.into(), plane: Plane::Foreground, name: "Ryhmä".into(), visible: true, opacity: 1.0, blend: BlendMode::Normal, children: Vec::new() };
                match candidate.apply(Command::CreateGroup(group.clone())) {
                    Ok(()) => { self.commit(document, candidate); self.selected = Some(id); self.group_draft = Some(group); }
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
            for plane in [Plane::Background, Plane::Foreground] {
                if let Ok(steps) = document.composition(scene_id, plane, 0.0) {
                    let mut depth: usize = 0;
                    for step in steps {
                        match step {
                            motion_art::CompositionStep::EnterGroup(group) => {
                                if ui.selectable_label(self.selected.as_deref() == Some(&group.id), format!("{}▣ {}", "  ".repeat(depth), group.name)).clicked() {
                                    self.selected = Some(group.id.clone()); self.group_draft = Some(group.clone());
                                }
                                depth += 1;
                            }
                            motion_art::CompositionStep::LeaveGroup => depth = depth.saturating_sub(1),
                            motion_art::CompositionStep::Layer(layer) => {
                                if ui.selectable_label(self.selected.as_deref() == Some(&layer.id), format!("{}{}", "  ".repeat(depth), layer.name)).clicked() { self.selected = Some(layer.id.clone()); self.group_draft = None; }
                            }
                        }
                    }
                }
            }
            // Hidden groups remain selectable so visibility can always be restored.
            for group in document.groups.iter().filter(|g| g.scene_id == scene_id && !g.visible) {
                if ui.selectable_label(self.selected.as_deref() == Some(&group.id), format!("Piilotettu ryhmä: {}", group.name)).clicked() { self.selected = Some(group.id.clone()); self.group_draft = Some(group.clone()); }
            }
            let mut command = None;
            if let Some(draft) = &mut self.group_draft {
                ui.text_edit_singleline(&mut draft.name);
                ui.checkbox(&mut draft.visible, "Näytä ryhmä sisältöineen");
                ui.add(egui::Slider::new(&mut draft.opacity, 0.0..=1.0).text("Ryhmän peittävyys"));
                egui::ComboBox::from_label("Ryhmän sijoitus").selected_text(plane_label(draft.plane)).show_ui(ui, |ui| {
                    ui.selectable_value(&mut draft.plane, Plane::Background, "Tausta");
                    ui.selectable_value(&mut draft.plane, Plane::Foreground, "Etuala");
                });
                egui::ComboBox::from_label("Ryhmän sekoitustapa").selected_text(blend_label(draft.blend)).show_ui(ui, |ui| {
                    for mode in [BlendMode::Normal, BlendMode::Multiply, BlendMode::Screen] { ui.selectable_value(&mut draft.blend, mode, blend_label(mode)); }
                });
                if ui.button("Käytä ryhmän asetuksia").clicked() { command = Some(Command::ReplaceGroup(draft.clone())); }
                ui.weak("Ryhmän purkaminen säilyttää sisällön järjestyksen. Peittävyyden tai sekoitustavan poistuminen voi muuttaa ulkoasua.");
                if ui.button("Pura ryhmä, säilytä sisältö").clicked() { command = Some(Command::Ungroup { id: draft.id.clone() }); }
            }
            if let Some(id) = &self.selected {
                let unit = if document.groups.iter().any(|g| &g.id == id) { ArtUnit::Group(id.clone()) } else { ArtUnit::Layer(id.clone()) };
                ui.label("Siirrä valinta: emoryhmän tunnus (tyhjä = juuritaso), paikan indeksi (alkaen 0)");
                ui.text_edit_singleline(&mut self.move_parent);
                ui.add(egui::DragValue::new(&mut self.move_index).range(0..=motion_art::MAX_LAYERS));
                if ui.button("Käytä hierarkiasiirtoa").clicked() {
                    command = Some(Command::Move { unit, parent: (!self.move_parent.is_empty()).then(|| self.move_parent.clone()), index: self.move_index });
                }
            }
            if let Some(command) = command {
                let ungroup = matches!(&command, Command::Ungroup { .. });
                let mut candidate = document.clone();
                match candidate.apply(command) {
                    Ok(()) => { self.commit(document, candidate); if ungroup { self.selected = None; self.group_draft = None; } }
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        });
    }
    fn id(&self, document: &Document) -> String {
        (1..)
            .map(|i| format!("art-{i}"))
            .find(|id| !document.layers.iter().any(|l| &l.id == id))
            .unwrap()
    }
    fn finish_mask_gesture(&mut self, document: &mut Document) {
        let Some(draft) = self.mask_gesture.take() else {
            return;
        };
        let Some(index) = document
            .layers
            .iter()
            .position(|layer| layer == &draft.layer)
        else {
            return;
        };
        if draft.points.is_empty() {
            return;
        }
        let mut next = document.clone();
        let Some(ClipShape::Painted { strokes, .. }) = &mut next.layers[index].clip_shape else {
            return;
        };
        strokes.push(MaskStroke {
            points: draft.points,
            radius: draft.radius,
            hardness: draft.hardness,
            pressures: if draft.pressure.received_force {
                draft.pressure.samples
            } else {
                Vec::new()
            },
            erase: draft.tool == Tool::MaskErase,
        });
        self.commit(document, next);
    }
    fn mask_gesture(
        &mut self,
        response: &egui::Response,
        rect: egui::Rect,
        scene: &motion_scene::EvaluatedScene,
        scene_id: &str,
        document: &mut Document,
    ) {
        let camera = (
            scene.camera.x,
            scene.camera.y,
            scene.camera.zoom,
            scene.time,
        );
        let selected = document
            .layers
            .iter()
            .find(|layer| Some(&layer.id) == self.selected.as_ref() && layer.scene_id == scene_id);
        let cancelled = response
            .ctx
            .input(|input| input.key_pressed(egui::Key::Escape));
        let incompatible = self.mask_gesture.as_ref().is_some_and(|draft| {
            selected != Some(&draft.layer)
                || draft.scene != scene_id
                || draft.tool != self.tool
                || draft.camera != camera
                || draft.viewport != rect
                || draft.scene_size != (scene.width, scene.height)
                || draft.radius != self.mask_radius
                || draft.hardness != self.mask_hardness
                || draft.pressure.enabled != self.mask_pressure
        });
        if cancelled || incompatible || !response.enabled() {
            self.mask_gesture = None;
            return;
        }
        let Some(layer) = selected else {
            self.mask_gesture = None;
            return;
        };
        if !matches!(layer.clip_shape, Some(ClipShape::Painted { .. })) {
            self.mask_gesture = None;
            return;
        }
        let (pressed, down, released, position, events) = response.ctx.input(|input| {
            (
                input.pointer.primary_pressed(),
                input.pointer.primary_down(),
                input.pointer.primary_released(),
                input.pointer.interact_pos(),
                input.events.clone(),
            )
        });
        if pressed && response.hovered() {
            self.mask_gesture = Some(MaskGesture {
                layer: layer.clone(),
                scene: scene_id.into(),
                tool: self.tool,
                camera,
                viewport: rect,
                scene_size: (scene.width, scene.height),
                points: Vec::new(),
                radius: self.mask_radius,
                hardness: self.mask_hardness,
                pressure: PressureCapture::begin(self.mask_pressure, &events, position),
            });
        }
        if self
            .mask_gesture
            .as_mut()
            .is_some_and(|draft| !draft.pressure.update(&events, position))
        {
            self.mask_gesture = None;
            return;
        }
        let scale = (rect.width() / scene.width as f32).min(rect.height() / scene.height as f32);
        if !scale.is_finite()
            || scale <= 0.0
            || !scene.camera.zoom.is_finite()
            || scene.camera.zoom <= 0.0
        {
            self.mask_gesture = None;
            return;
        }
        let origin =
            rect.center() - egui::vec2(scene.width as f32, scene.height as f32) * scale * 0.5;
        let offset = match &layer.attachment {
            Some(id) => match scene.actors.iter().find(|actor| &actor.id == id) {
                Some(actor) => (actor.x, actor.y),
                None => {
                    self.mask_gesture = None;
                    return;
                }
            },
            None => (0.0, 0.0),
        };
        if let Some(position) = position {
            if let Some(draft) = self.mask_gesture.as_mut().filter(|_| down || released) {
                let x = (position.x - origin.x) as f64 / scale as f64;
                let y = (position.y - origin.y) as f64 / scale as f64;
                let point = Point {
                    x: (x - scene.width as f64 / 2.0) / scene.camera.zoom
                        + scene.width as f64 / 2.0
                        + scene.camera.x
                        - offset.0,
                    y: (y - scene.height as f64 / 2.0) / scene.camera.zoom
                        + scene.height as f64 / 2.0
                        + scene.camera.y
                        - offset.1,
                };
                if draft.points.last() != Some(&point)
                    || draft.pressure.samples.last() != Some(&draft.pressure.value())
                {
                    if draft.points.len() >= motion_art::MAX_MASK_STROKE_POINTS {
                        self.mask_gesture = None;
                        self.error = Some(
                            "Maskiveto ylittää 2048 pistettä. Piirrä lyhyempiä vetoja.".into(),
                        );
                        return;
                    }
                    draft.points.push(point);
                    draft.pressure.samples.push(draft.pressure.value());
                }
            }
            if response.hovered() || self.mask_gesture.is_some() {
                let painter = response
                    .ctx
                    .layer_painter(response.layer_id)
                    .with_clip_rect(rect);
                let color = if self.tool == Tool::MaskErase {
                    egui::Color32::LIGHT_RED
                } else {
                    egui::Color32::LIGHT_GREEN
                };
                painter.circle_stroke(
                    position,
                    (self.mask_radius
                        * scene.camera.zoom
                        * self
                            .mask_gesture
                            .as_ref()
                            .map_or(1.0, |draft| draft.pressure.value()))
                        as f32
                        * scale,
                    egui::Stroke::new(1.5_f32, color),
                );
                if let Some(draft) = &self.mask_gesture {
                    for point in &draft.points {
                        let (x, y) = scene.camera.project(
                            point.x + offset.0,
                            point.y + offset.1,
                            scene.width,
                            scene.height,
                        );
                        painter.circle_filled(
                            origin + egui::vec2(x as f32, y as f32) * scale,
                            2.0,
                            color,
                        );
                    }
                }
            }
        }
        if released {
            self.finish_mask_gesture(document);
        } else if !down {
            // Focus loss / pointer cancellation must not leave a draft for a later gesture.
            self.mask_gesture = None;
        }
    }
    pub fn gesture(
        &mut self,
        response: &egui::Response,
        rect: egui::Rect,
        scene: &motion_scene::EvaluatedScene,
        scene_id: &str,
        document: &mut Document,
    ) {
        if matches!(self.tool, Tool::MaskPaint | Tool::MaskErase) {
            self.mask_gesture(response, rect, scene, scene_id, document);
            return;
        }
        self.mask_gesture = None;
        if self.tool == Tool::Select {
            return;
        }
        if response.drag_started() {
            self.points.clear();
            self.gesture_scene = scene_id.into();
        }
        if self.gesture_scene != scene_id {
            self.points.clear();
            return;
        }
        if let Some(pos) = response
            .interact_pointer_pos()
            .filter(|_| response.dragged() || response.drag_stopped())
        {
            let scale =
                (rect.width() / scene.width as f32).min(rect.height() / scene.height as f32);
            let origin =
                rect.center() - egui::vec2(scene.width as f32, scene.height as f32) * scale * 0.5;
            let x = (pos.x - origin.x) as f64 / scale as f64;
            let y = (pos.y - origin.y) as f64 / scale as f64;
            let point = Point {
                x: (x - scene.width as f64 / 2.0) / scene.camera.zoom
                    + scene.width as f64 / 2.0
                    + scene.camera.x,
                y: (y - scene.height as f64 / 2.0) / scene.camera.zoom
                    + scene.height as f64 / 2.0
                    + scene.camera.y,
            };
            if self.points.len() < 2048 {
                self.points.push(point);
            }
        }
        if response.drag_stopped() && self.points.len() >= 2 {
            let first = &self.points[0];
            let last = self.points.last().unwrap();
            let bounds = Rect {
                x: first.x.min(last.x),
                y: first.y.min(last.y),
                width: (first.x - last.x).abs().max(1.0),
                height: (first.y - last.y).abs().max(1.0),
            };
            let color = Color {
                r: 90,
                g: 200,
                b: 160,
                a: 255,
            };
            let content = match self.tool {
                Tool::Pen => Content::Stroke {
                    points: self.points.clone(),
                    color,
                    width: 4.0,
                },
                Tool::Rectangle => Content::Rectangle {
                    rect: bounds,
                    color,
                },
                Tool::Ellipse => Content::Ellipse {
                    rect: bounds,
                    color,
                },
                Tool::Select | Tool::MaskPaint | Tool::MaskErase => return,
            };
            let mut next = document.clone();
            let id = self.id(&next);
            self.selected = Some(id.clone());
            next.layers.push(Layer {
                id,
                name: "Piirros".into(),
                scene_id: scene_id.into(),
                visible: true,
                plane: Plane::Foreground,
                active: None,
                opacity: 1.0,
                attachment: None,
                clip_shape: None,
                blend: motion_art::BlendMode::Normal,
                clip: None,
                content,
            });
            self.commit(document, next);
            self.points.clear();
        }
    }
}

/// Display only an already decoded native texture; never read files from a frame.
fn asset_thumbnail(ui: &mut egui::Ui, texture: Option<&egui::TextureHandle>) {
    let bounds = egui::vec2(64.0, 64.0);
    let (rect, response) = ui.allocate_exact_size(bounds, egui::Sense::hover());
    if let Some(texture) = texture {
        let size = thumbnail_size(texture.size(), bounds);
        let image_rect = egui::Rect::from_center_size(rect.center(), size);
        ui.painter().image(
            texture.id(),
            image_rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        response.on_hover_text("Ladatun kuvan esikatselu");
    } else {
        ui.painter()
            .rect_filled(rect, 3.0, ui.visuals().faint_bg_color);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Esikatselu puuttuu",
            egui::FontId::proportional(10.0),
            ui.visuals().weak_text_color(),
        );
        response.on_hover_text("Esikatselu on kesken tai puuttuu. Tarkista resurssien latausvirheet; kuvan latautumista ei ole vielä vahvistettu.");
    }
}

fn thumbnail_size(size: [usize; 2], bounds: egui::Vec2) -> egui::Vec2 {
    let source = egui::vec2(size[0].max(1) as f32, size[1].max(1) as f32);
    source * (bounds.x / source.x).min(bounds.y / source.y)
}

pub fn paint(
    painter: &egui::Painter,
    canvas: egui::Rect,
    scene: &motion_scene::EvaluatedScene,
    scene_id: &str,
    document: &Document,
    resources: (
        &std::collections::BTreeMap<String, egui::TextureHandle>,
        Plane,
    ),
) {
    let (textures, plane) = resources;
    // Grouped scenes must use the shared compositor, never a flattened painter fallback.
    if !document.order.is_empty()
        || document.groups.iter().any(|g| g.scene_id == scene_id)
        || document.lights.iter().any(|l| l.scene_id == scene_id)
    {
        return;
    }

    let scale = (canvas.width() / scene.width as f32).min(canvas.height() / scene.height as f32);
    let origin =
        canvas.center() - egui::vec2(scene.width as f32, scene.height as f32) * scale * 0.5;
    for layer in document
        .layers
        .iter()
        .filter(|l| l.is_active(scene.time) && l.scene_id == scene_id && l.plane == plane)
    {
        let offset = if let Some(id) = &layer.attachment {
            let Some(actor) = scene.actors.iter().find(|a| &a.id == id) else {
                continue;
            };
            (actor.x, actor.y)
        } else {
            (0.0, 0.0)
        };
        let point = |x: f64, y: f64| {
            let (x, y) =
                scene
                    .camera
                    .project(x + offset.0, y + offset.1, scene.width, scene.height);
            origin + egui::vec2(x as f32, y as f32) * scale
        };
        let bounds = |r: &Rect| {
            egui::Rect::from_two_pos(point(r.x, r.y), point(r.x + r.width, r.y + r.height))
        };
        let mut painter = painter.clone();
        painter.set_opacity(layer.opacity as f32);
        if let Some(clip) = &layer.clip {
            painter = painter.with_clip_rect(bounds(clip).intersect(canvas));
        }
        let color = |c: &Color| egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
        match &layer.content {
            Content::Image { asset_id, rect } => {
                if let Some(texture) = textures.get(asset_id) {
                    painter.image(
                        texture.id(),
                        bounds(rect),
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                } else {
                    painter.rect_filled(bounds(rect), 0.0, egui::Color32::DARK_RED);
                }
            }
            Content::AnimatedImage { rect, .. } => {
                match layer
                    .animated_asset(scene.time)
                    .and_then(|asset| textures.get(asset))
                {
                    Some(texture) => {
                        painter.image(
                            texture.id(),
                            bounds(rect),
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                    None => {
                        painter.rect_filled(bounds(rect), 0.0, egui::Color32::DARK_RED);
                    }
                }
            }
            Content::Rectangle { rect, color: c } => {
                painter.rect_filled(bounds(rect), 0.0, color(c));
            }
            Content::Ellipse { rect, color: c } => {
                let r = bounds(rect);
                painter.add(egui::epaint::EllipseShape::filled(
                    r.center(),
                    r.size() * 0.5,
                    color(c),
                ));
            }
            Content::Stroke {
                points,
                color: c,
                width,
            } => {
                painter.add(egui::Shape::line(
                    points.iter().map(|p| point(p.x, p.y)).collect(),
                    egui::Stroke::new(
                        (*width as f32 * scale * scene.camera.zoom as f32).max(0.1),
                        color(c),
                    ),
                ));
            }
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn image_mask_key_drafts_validate_before_commit() {
        assert!(super::parse_image_mask_keys("0,0,0,10,10 1,20,0,10,10", 1.0).is_ok());
        for text in [
            "0,0,0,10,10 0,20,0,10,10",
            "2,0,0,10,10",
            "0,0,0,0,10",
            "NaN,0,0,10,10",
            "0,0,0,10",
        ] {
            assert!(super::parse_image_mask_keys(text, 1.0).is_err(), "{text}");
        }
    }
    fn mask_document() -> Document {
        Document {
            layers: vec![Layer {
                id: "mask".into(),
                scene_id: "scene".into(),
                name: "Mask".into(),
                visible: true,
                plane: Plane::Foreground,
                active: None,
                opacity: 1.0,
                attachment: None,
                clip: None,
                clip_shape: Some(ClipShape::Painted {
                    motion: Vec::new(),
                    base_visible: false,
                    strokes: Vec::new(),
                }),
                blend: BlendMode::Normal,
                content: Content::Rectangle {
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 300.0,
                        height: 200.0,
                    },
                    color: Color {
                        r: 255,
                        g: 0,
                        b: 0,
                        a: 255,
                    },
                },
            }],
            ..Document::default()
        }
    }
    fn mask_scene() -> motion_scene::EvaluatedScene {
        motion_scene::EvaluatedScene {
            time: 0.0,
            camera: motion_scene::CameraPose {
                x: 20.0,
                y: 10.0,
                zoom: 2.0,
            },
            background_color: "#000000".into(),
            opacity: 1.0,
            width: 300,
            height: 200,
            actors: vec![],
        }
    }
    fn mask_frame(
        ctx: &egui::Context,
        editor: &mut Editor,
        document: &mut Document,
        scene: &motion_scene::EvaluatedScene,
        events: Vec<egui::Event>,
    ) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 300.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let rect =
                        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 200.0));
                    let response =
                        ui.interact(rect, egui::Id::new("mask-gesture"), egui::Sense::drag());
                    editor.gesture(&response, rect, scene, "scene", document);
                });
            },
        );
    }
    fn mask_button(x: f32, y: f32, pressed: bool) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(egui::pos2(x, y)),
            egui::Event::PointerButton {
                pos: egui::pos2(x, y),
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            },
        ]
    }
    fn mask_touch(id: u64, phase: egui::TouchPhase, force: Option<f32>) -> egui::Event {
        egui::Event::Touch {
            device_id: egui::TouchDeviceId(7),
            id: egui::TouchId(id),
            phase,
            pos: egui::pos2(60.0, 60.0),
            force,
        }
    }
    #[test]
    fn device_force_and_softness_persist_once_without_mutating_a_draft() {
        let ctx = egui::Context::default();
        let mut editor = Editor {
            tool: Tool::MaskPaint,
            selected: Some("mask".into()),
            mask_hardness: 0.25,
            mask_pressure: true,
            ..Default::default()
        };
        let mut document = mask_document();
        let before = document.clone();
        let scene = mask_scene();
        mask_frame(&ctx, &mut editor, &mut document, &scene, vec![]);
        let mut events = mask_button(60.0, 60.0, true);
        events.push(mask_touch(1, egui::TouchPhase::Start, Some(0.25)));
        mask_frame(&ctx, &mut editor, &mut document, &scene, events);
        assert_eq!(document, before);
        // A pressure-only move at the same position is a distinct sample.
        mask_frame(
            &ctx,
            &mut editor,
            &mut document,
            &scene,
            vec![mask_touch(1, egui::TouchPhase::Move, Some(0.0))],
        );
        assert_eq!(document, before);
        let mut events = mask_button(60.0, 60.0, false);
        events.push(mask_touch(1, egui::TouchPhase::End, None));
        mask_frame(&ctx, &mut editor, &mut document, &scene, events);
        let Some(ClipShape::Painted { strokes, .. }) = &document.layers[0].clip_shape else {
            panic!("painted mask")
        };
        assert_eq!(strokes.len(), 1);
        assert_eq!(strokes[0].pressures, vec![0.25, 0.0]);
        assert_eq!(strokes[0].hardness, 0.25);
        assert_eq!(strokes[0].points.len(), 2);
        let encoded = serde_json::to_string(&document).unwrap();
        assert_eq!(
            serde_json::from_str::<Document>(&encoded).unwrap(),
            document
        );
        assert_eq!(editor.revision(), 1);
        assert_eq!(editor.take_transaction_before(), Some(before));
        assert!(editor.take_transaction_before().is_none());
    }
    #[test]
    fn pressure_option_does_not_invent_force_for_mouse_or_missing_device_values() {
        for enabled in [false, true] {
            let ctx = egui::Context::default();
            let mut editor = Editor {
                tool: Tool::MaskErase,
                selected: Some("mask".into()),
                mask_pressure: enabled,
                ..Default::default()
            };
            let mut document = mask_document();
            let scene = mask_scene();
            mask_frame(&ctx, &mut editor, &mut document, &scene, vec![]);
            mask_frame(
                &ctx,
                &mut editor,
                &mut document,
                &scene,
                mask_button(60.0, 60.0, true),
            );
            mask_frame(
                &ctx,
                &mut editor,
                &mut document,
                &scene,
                mask_button(60.0, 60.0, false),
            );
            let Some(ClipShape::Painted { strokes, .. }) = &document.layers[0].clip_shape else {
                panic!("painted mask")
            };
            assert!(strokes[0].pressures.is_empty());
            assert_eq!(strokes[0].hardness, 1.0);
            assert!(strokes[0].erase);
        }
        let pos = Some(egui::pos2(60.0, 60.0));
        let start = vec![mask_touch(1, egui::TouchPhase::Start, None)];
        let mut pressure = PressureCapture::begin(true, &start, pos);
        assert!(pressure.update(&start, pos));
        assert_eq!(pressure.value(), 1.0);
        for force in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert!(pressure.update(&[mask_touch(1, egui::TouchPhase::Move, Some(force))], pos));
            assert_eq!(pressure.value(), 1.0);
        }
        assert!(!pressure.received_force);
        let mut disabled = PressureCapture::begin(false, &start, pos);
        assert!(disabled.update(&[mask_touch(1, egui::TouchPhase::Move, Some(0.25))], pos));
        assert_eq!(disabled.value(), 1.0);
        assert!(!disabled.received_force);
    }
    #[test]
    fn foreign_touch_and_touch_cancel_discard_the_entire_pressure_gesture() {
        for event in [
            mask_touch(2, egui::TouchPhase::Move, Some(0.9)),
            mask_touch(1, egui::TouchPhase::Cancel, None),
        ] {
            let ctx = egui::Context::default();
            let mut editor = Editor {
                tool: Tool::MaskPaint,
                selected: Some("mask".into()),
                mask_pressure: true,
                ..Default::default()
            };
            let mut document = mask_document();
            let before = document.clone();
            let scene = mask_scene();
            mask_frame(&ctx, &mut editor, &mut document, &scene, vec![]);
            let mut events = mask_button(60.0, 60.0, true);
            events.push(mask_touch(1, egui::TouchPhase::Start, Some(0.25)));
            mask_frame(&ctx, &mut editor, &mut document, &scene, events);
            mask_frame(&ctx, &mut editor, &mut document, &scene, vec![event]);
            mask_frame(
                &ctx,
                &mut editor,
                &mut document,
                &scene,
                mask_button(60.0, 60.0, false),
            );
            assert_eq!(document, before);
            assert!(editor.take_transaction_before().is_none());
            assert_eq!(editor.revision(), 0);
        }
    }
    #[test]
    fn painted_mask_gestures_are_detached_and_single_dabs_commit_once() {
        let ctx = egui::Context::default();
        let mut editor = Editor {
            tool: Tool::MaskPaint,
            selected: Some("mask".into()),
            mask_radius: 7.0,
            ..Default::default()
        };
        let mut document = mask_document();
        let before = document.clone();
        let scene = mask_scene();
        mask_frame(&ctx, &mut editor, &mut document, &scene, vec![]);
        mask_frame(
            &ctx,
            &mut editor,
            &mut document,
            &scene,
            mask_button(60.0, 60.0, true),
        );
        assert_eq!(document, before);
        assert_eq!(editor.revision(), 0);
        mask_frame(
            &ctx,
            &mut editor,
            &mut document,
            &scene,
            mask_button(60.0, 60.0, false),
        );
        let Some(ClipShape::Painted { strokes, .. }) = &document.layers[0].clip_shape else {
            panic!("painted mask")
        };
        assert_eq!(strokes.len(), 1);
        assert_eq!(strokes[0].points, vec![Point { x: 125.0, y: 90.0 }]);
        assert_eq!(strokes[0].radius, 7.0);
        assert!(!strokes[0].erase);
        assert_eq!(editor.revision(), 1);
        assert_eq!(editor.take_transaction_before(), Some(before));
        let painted = document.clone();
        editor.tool = Tool::MaskErase;
        mask_frame(
            &ctx,
            &mut editor,
            &mut document,
            &scene,
            mask_button(80.0, 80.0, true),
        );
        mask_frame(
            &ctx,
            &mut editor,
            &mut document,
            &scene,
            vec![egui::Event::PointerMoved(egui::pos2(100.0, 100.0))],
        );
        assert_eq!(document, painted);
        mask_frame(
            &ctx,
            &mut editor,
            &mut document,
            &scene,
            mask_button(110.0, 110.0, false),
        );
        let Some(ClipShape::Painted { strokes, .. }) = &document.layers[0].clip_shape else {
            panic!("painted mask")
        };
        assert_eq!(strokes.len(), 2);
        assert!(strokes[1].erase);
        assert!(strokes[1].points.len() >= 2);
        assert_eq!(editor.take_transaction_before(), Some(painted));
        assert_eq!(editor.revision(), 2);
    }
    #[test]
    fn painted_dab_uses_attachment_local_coordinates() {
        let ctx = egui::Context::default();
        let mut editor = Editor {
            tool: Tool::MaskPaint,
            selected: Some("mask".into()),
            mask_radius: 7.0,
            ..Default::default()
        };
        let mut document = mask_document();
        document.layers[0].attachment = Some("actor".into());
        let mut scene = mask_scene();
        scene.actors.push(motion_scene::ActorPose {
            id: "actor".into(),
            x: 40.0,
            y: 30.0,
            emotion: "neutral".into(),
            rotation: 0.0,
            scale: motion_scene::Vec2::new(1.0, 1.0),
            wave: 0.0,
            skeleton: [motion_scene::Vec2::new(0.0, 0.0); 7],
            rigid_parts: Vec::new(),
            mouth: None,
        });
        mask_frame(&ctx, &mut editor, &mut document, &scene, vec![]);
        mask_frame(
            &ctx,
            &mut editor,
            &mut document,
            &scene,
            mask_button(60.0, 60.0, true),
        );
        mask_frame(
            &ctx,
            &mut editor,
            &mut document,
            &scene,
            mask_button(60.0, 60.0, false),
        );
        let Some(ClipShape::Painted { strokes, .. }) = &document.layers[0].clip_shape else {
            panic!("painted mask")
        };
        assert_eq!(strokes[0].points, vec![Point { x: 85.0, y: 60.0 }]);
    }
    #[test]
    fn painted_drafts_cancel_on_selection_scene_camera_and_document_changes() {
        for changed in 0..6 {
            let ctx = egui::Context::default();
            let mut editor = Editor {
                tool: Tool::MaskPaint,
                selected: Some("mask".into()),
                mask_radius: 7.0,
                ..Default::default()
            };
            let mut document = mask_document();
            let mut scene = mask_scene();
            mask_frame(&ctx, &mut editor, &mut document, &scene, vec![]);
            mask_frame(
                &ctx,
                &mut editor,
                &mut document,
                &scene,
                mask_button(60.0, 60.0, true),
            );
            assert!(editor.mask_gesture.is_some());
            match changed {
                0 => editor.selected = None,
                1 => scene.time = 0.5,
                2 => scene.camera.zoom = 1.0,
                3 => document.layers[0].attachment = Some("actor".into()),
                4 => editor.mask_hardness = 0.25,
                _ => editor.mask_pressure = true,
            }
            let unchanged = document.clone();
            mask_frame(
                &ctx,
                &mut editor,
                &mut document,
                &scene,
                mask_button(80.0, 80.0, false),
            );
            assert_eq!(document, unchanged);
            assert!(editor.mask_gesture.is_none());
            assert!(editor.take_transaction_before().is_none());
        }
    }
    #[test]
    fn painted_stroke_rejection_does_not_replace_other_masks_or_commit_invalid_data() {
        let mut editor = Editor::default();
        let mut document = mask_document();
        let before = document.clone();
        editor.mask_gesture = Some(MaskGesture {
            layer: document.layers[0].clone(),
            scene: "scene".into(),
            tool: Tool::MaskPaint,
            camera: (0.0, 0.0, 1.0, 0.0),
            viewport: egui::Rect::NOTHING,
            scene_size: (300, 200),
            points: vec![Point {
                x: f64::NAN,
                y: 0.0,
            }],
            radius: 5.0,
            hardness: 1.0,
            pressure: PressureCapture::default(),
        });
        editor.finish_mask_gesture(&mut document);
        assert_eq!(document, before);
        assert!(editor.take_transaction_before().is_none());
        assert!(editor.error.is_some());
        document.layers[0].clip_shape = Some(ClipShape::Ellipse {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            },
        });
        let unchanged = document.clone();
        editor.mask_gesture = Some(MaskGesture {
            layer: document.layers[0].clone(),
            scene: "scene".into(),
            tool: Tool::MaskErase,
            camera: (0.0, 0.0, 1.0, 0.0),
            viewport: egui::Rect::NOTHING,
            scene_size: (300, 200),
            points: vec![Point { x: 5.0, y: 5.0 }],
            radius: 5.0,
            hardness: 1.0,
            pressure: PressureCapture::default(),
        });
        editor.finish_mask_gesture(&mut document);
        assert_eq!(document, unchanged);
        assert!(editor.take_transaction_before().is_none());
    }
    #[test]
    fn grouped_transactions_have_one_before_snapshot_and_invalid_apply_is_atomic() {
        let mut editor = Editor::default();
        let mut document = Document::default();
        let before = document.clone();
        let mut candidate = document.clone();
        candidate
            .apply(motion_art::Command::CreateGroup(motion_art::Group {
                id: "group".into(),
                scene_id: "s".into(),
                plane: Plane::Foreground,
                name: "Group".into(),
                visible: true,
                opacity: 0.5,
                blend: BlendMode::Multiply,
                children: Vec::new(),
            }))
            .unwrap();
        editor.commit(&mut document, candidate);
        assert_eq!(editor.take_transaction_before(), Some(before));
        let committed = document.clone();
        let revision = editor.revision();
        let mut invalid = committed.clone();
        invalid.groups[0].opacity = f64::NAN;
        editor.commit(&mut document, invalid);
        assert_eq!(document, committed);
        assert_eq!(editor.revision(), revision);
        assert!(editor.take_transaction_before().is_none());
        assert!(editor.error.is_some());
    }
    #[test]
    fn soft_mask_key_drafts_validate_before_changing_document_history() {
        let keys = parse_mask_keys("0,0,0,20,20,0\n2,10,5,40,30,1", 2.0).unwrap();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[1].feather, 1.0);
        for invalid in [
            "0,0,0,20,20,NaN",
            "0,0,0,20,20,1.1",
            "3,0,0,20,20,0",
            "1,0,0,20,20,0 1,0,0,20,20,1",
            "0,0,0,0,20,0",
            "0,0,0,20,20",
        ] {
            assert!(parse_mask_keys(invalid, 2.0).is_err(), "{invalid}");
        }
        assert!(parse_mask_keys("", 2.0).unwrap().is_empty());
        let mut document = Document::default();
        let mut editor = Editor::default();
        let candidate = Document {
            layers: vec![Layer {
                id: "soft".into(),
                scene_id: "s".into(),
                name: "Soft mask".into(),
                visible: true,
                plane: Plane::Foreground,
                active: None,
                opacity: 1.0,
                attachment: None,
                clip: None,
                blend: BlendMode::Normal,
                clip_shape: Some(ClipShape::SoftEllipse {
                    rect: keys[0].rect,
                    feather: 0.0,
                    keys,
                }),
                content: Content::Rectangle {
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 100.0,
                        height: 100.0,
                    },
                    color: Color {
                        r: 255,
                        g: 0,
                        b: 0,
                        a: 255,
                    },
                },
            }],
            ..Document::default()
        };
        editor.commit(&mut document, candidate.clone());
        assert_eq!(document, candidate);
        assert_eq!(editor.take_transaction_before(), Some(Document::default()));
        let mut invalid = candidate.clone();
        let Some(ClipShape::SoftEllipse { keys, .. }) = &mut invalid.layers[0].clip_shape else {
            panic!("soft mask")
        };
        keys[1].time = 0.0;
        editor.commit(&mut document, invalid);
        assert_eq!(document, candidate);
        assert!(editor.take_transaction_before().is_none());
        assert!(editor.error.is_some());
    }
    #[test]
    fn thumbnail_preserves_landscape_and_portrait_aspect() {
        assert_eq!(
            thumbnail_size([400, 200], egui::vec2(64.0, 64.0)),
            egui::vec2(64.0, 32.0)
        );
        assert_eq!(
            thumbnail_size([200, 400], egui::vec2(64.0, 64.0)),
            egui::vec2(32.0, 64.0)
        );
    }

    #[test]
    fn asset_inventory_rendering_preserves_document_with_present_or_missing_preview() {
        let context = egui::Context::default();
        let texture = context.load_texture(
            "synthetic-asset",
            egui::ColorImage::new([2, 1], egui::Color32::RED),
            Default::default(),
        );
        let mut textures = BTreeMap::new();
        textures.insert("present".into(), texture);
        let assets: Vec<_> = ["present", "missing"]
            .into_iter()
            .map(|id| motion_project::AssetReference {
                id: id.into(),
                path: format!("{id}.png"),
                kind: motion_project::AssetKind::Image,
            })
            .collect();
        let mut editor = Editor::default();
        let mut document = Document::default();
        let before = document.clone();
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                // Force the collapsible inventory open without simulating a click.
                ui.memory_mut(|memory| memory.set_everything_is_visible(true));
                editor.ui(
                    ui,
                    &mut document,
                    SceneContext {
                        id: "scene",
                        duration: 2.0,
                        width: 100,
                        height: 100,
                    },
                    &assets,
                    &[],
                    &textures,
                );
            });
        });
        assert_eq!(document, before);
        assert!(!editor.dirty);
        assert!(editor.transaction_before.is_none());
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Mesh(mesh) if mesh.texture_id == textures["present"].id())));
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Esikatselu puuttuu")));
    }

    use super::*;
    #[test]
    fn rectangle_gesture_commits_once_in_camera_world_coordinates() {
        let ctx = egui::Context::default();
        let mut editor = Editor {
            tool: Tool::Rectangle,
            ..Default::default()
        };
        let mut document = Document::default();
        let scene = motion_scene::EvaluatedScene {
            time: 0.0,
            camera: motion_scene::CameraPose {
                x: 20.0,
                y: 10.0,
                zoom: 2.0,
            },
            background_color: "#000000".into(),
            opacity: 1.0,
            width: 300,
            height: 200,
            actors: vec![],
        };
        for events in [
            vec![],
            vec![
                egui::Event::PointerMoved(egui::pos2(20.0, 20.0)),
                egui::Event::PointerButton {
                    pos: egui::pos2(20.0, 20.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
            vec![egui::Event::PointerMoved(egui::pos2(60.0, 60.0))],
            vec![
                egui::Event::PointerMoved(egui::pos2(100.0, 100.0)),
                egui::Event::PointerButton {
                    pos: egui::pos2(100.0, 100.0),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ],
        ] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(400.0, 300.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let rect =
                            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 200.0));
                        let response =
                            ui.interact(rect, egui::Id::new("gesture"), egui::Sense::drag());
                        editor.gesture(&response, rect, &scene, "scene", &mut document);
                    });
                },
            );
        }
        assert_eq!(document.layers.len(), 1);
        assert!(editor.transaction_before.is_some());
        let Content::Rectangle { rect, .. } = document.layers[0].content else {
            panic!("rectangle expected")
        };
        assert_eq!(rect.x, 105.0);
        assert_eq!(rect.y, 70.0);
        assert_eq!(rect.width, 40.0);
        assert_eq!(rect.height, 40.0);
    }
    #[test]
    fn timed_artwork_changes_are_atomic_and_expose_the_before_state() {
        let mut editor = Editor::default();
        let mut document = Document::default();
        let layer = Layer {
            id: "timed".into(),
            scene_id: "scene".into(),
            name: "Background".into(),
            visible: true,
            plane: Plane::Background,
            active: Some(TimeRange {
                start: 1.0,
                end: 2.0,
            }),
            opacity: 1.0,
            attachment: None,
            clip_shape: None,
            blend: motion_art::BlendMode::Normal,
            clip: None,
            content: Content::Rectangle {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 20.0,
                    height: 20.0,
                },
                color: Color {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 255,
                },
            },
        };
        editor.commit(
            &mut document,
            Document {
                layers: vec![layer],
                ..Document::default()
            },
        );
        let before = document.clone();
        let mut invalid = before.clone();
        invalid.layers[0].active.as_mut().unwrap().end = 1.0;
        editor.commit(&mut document, invalid);
        assert_eq!(document, before);
        assert!(editor.transaction_before.is_some());
        assert!(editor.error.is_some());
        let mut next = before.clone();
        next.layers[0].plane = Plane::Foreground;
        next.layers[0].active = None;
        editor.commit(&mut document, next);
        // The host consumes the initial transaction before a later edit.
        assert!(editor.take_transaction_before().is_some());
        assert_ne!(document, before);
    }

    #[test]
    fn advanced_polygon_edits_are_detached_validated_and_undoable() {
        let points = parse_mask_points("0,0 20,0 20,20 0,20").unwrap();
        assert!(parse_mask_points("0,0 20,20 0,20 20,0").is_err());
        assert!(parse_mask_points("NaN,0 1,0 0,1").is_err());
        let mut document = Document {
            layers: vec![Layer {
                id: "mask".into(),
                scene_id: "s".into(),
                name: "Mask".into(),
                visible: true,
                plane: Plane::Foreground,
                active: None,
                opacity: 1.0,
                attachment: None,
                clip: None,
                clip_shape: None,
                blend: BlendMode::Normal,
                content: Content::Rectangle {
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 20.0,
                        height: 20.0,
                    },
                    color: Color {
                        r: 1,
                        g: 2,
                        b: 3,
                        a: 255,
                    },
                },
            }],
            ..Document::default()
        };
        let before = document.clone();
        let mut editor = Editor::default();
        let mut next = before.clone();
        next.layers[0].clip_shape = Some(ClipShape::Polygon {
            motion: Vec::new(),
            points,
            feather: 0.0,
        });
        next.layers[0].blend = BlendMode::Screen;
        editor.commit(&mut document, next);
        assert_ne!(document, before);
        let valid = document.clone();
        let mut invalid = valid.clone();
        invalid.layers[0].clip = Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        });
        editor.commit(&mut document, invalid);
        assert_eq!(document, valid);
        // Rejected edits preserve the one before-state delivered to the shared
        // project history; the artwork editor no longer owns an undo stack.
        assert_eq!(editor.take_transaction_before(), Some(before));
        assert!(editor.take_transaction_before().is_none());
        assert!(editor.error.is_some());
    }

    #[test]
    fn native_painter_filters_plane_and_half_open_interval() {
        let mut document = Document::default();
        for (id, plane, r) in [
            ("back", Plane::Background, 13),
            ("front", Plane::Foreground, 27),
        ] {
            document.layers.push(Layer {
                id: id.into(),
                scene_id: "s".into(),
                name: id.into(),
                visible: true,
                plane,
                active: Some(TimeRange {
                    start: 1.0,
                    end: 2.0,
                }),
                opacity: 1.0,
                attachment: None,
                clip_shape: None,
                blend: motion_art::BlendMode::Normal,
                clip: None,
                content: Content::Rectangle {
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 10.0,
                        height: 10.0,
                    },
                    color: Color {
                        r,
                        g: 0,
                        b: 0,
                        a: 255,
                    },
                },
            });
        }
        let ctx = egui::Context::default();
        for (time, plane, expected) in [
            (0.999, Plane::Background, vec![]),
            (1.0, Plane::Background, vec![13]),
            (1.5, Plane::Foreground, vec![27]),
            (2.0, Plane::Background, vec![]),
        ] {
            let scene = motion_scene::EvaluatedScene {
                time,
                camera: motion_scene::CameraPose {
                    x: 0.0,
                    y: 0.0,
                    zoom: 1.0,
                },
                background_color: "#000000".into(),
                opacity: 0.25,
                width: 100,
                height: 100,
                actors: vec![],
            };
            let output = ctx.run(Default::default(), |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Middle,
                    egui::Id::new("paint"),
                ));
                paint(
                    &painter,
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0)),
                    &scene,
                    "s",
                    &document,
                    (&Default::default(), plane),
                );
            });
            let colors: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|s| {
                    if let egui::Shape::Rect(r) = &s.shape {
                        Some(r.fill.r())
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(colors, expected, "time {time}, plane {plane:?}");
        }
    }

    #[test]
    fn artwork_transactions_are_atomic_and_only_keep_the_first_before_state() {
        let mut e = Editor::default();
        let mut d = Document::default();
        for i in 0..30 {
            let mut next = d.clone();
            next.layers.push(Layer {
                id: format!("l{i}"),
                scene_id: "s".into(),
                name: "Layer".into(),
                visible: true,
                plane: Plane::Foreground,
                active: None,
                opacity: 1.0,
                attachment: None,
                clip_shape: None,
                blend: motion_art::BlendMode::Normal,
                clip: None,
                content: Content::Rectangle {
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 10.0,
                        height: 10.0,
                    },
                    color: Color {
                        r: 1,
                        g: 2,
                        b: 3,
                        a: 255,
                    },
                },
            });
            e.commit(&mut d, next);
        }
        assert_eq!(e.transaction_before.as_ref().unwrap(), &Document::default());
        assert_eq!(e.revision(), 30);
        let unchanged = d.clone();
        e.commit(&mut d, unchanged);
        assert_eq!(e.revision(), 30);
        let before = d.clone();
        let mut invalid = d.clone();
        invalid.layers[0].opacity = f64::NAN;
        e.commit(&mut d, invalid);
        assert_eq!(d, before);
        assert!(e.error.is_some());
        assert_eq!(e.revision(), 30);
        e.reset();
        assert_eq!(e.revision(), 31);
    }
}
