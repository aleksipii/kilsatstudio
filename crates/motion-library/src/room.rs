//! Deterministic parametric room generator.
//!
//! A [`RoomSpec`] produces back wall, floor, skirting, floor seams, an optional
//! door and 0–6 evenly spaced windows as palette-painted art layers, plus a
//! `ground` anchor (floor contact line) and a `doorway` anchor when a door
//! exists. Windows deliberately get **no** anchor: `screen` means a display
//! surface (TV, monitor, blackboard), and a window is not one. Window glass
//! rectangles are still returned in [`RoomArt::windows`] for generators.
//!
//! [`standard_variants`] derives `day`, `evening`, `rain` and `messy` variants
//! (palette overrides plus extra layers) that keep the stick-studio background
//! style rules.
use crate::manifest::{Anchor, ArtLayer, Depth, ManifestError, Variant};
use motion_art::{Color, Content, Point, Rect};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_WINDOWS: u8 = 6;
pub const MIN_ROOM_SIDE: f64 = 64.0;
pub const MAX_ROOM_SIDE: f64 = 8192.0;
/// Minimum horizontal space per window (frame and sill included).
pub const MIN_WINDOW_SLOT: f64 = 80.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeOfDay {
    Day,
    Evening,
    Night,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoorSide {
    None,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoomSpec {
    pub width: f64,
    pub height: f64,
    /// Back edge of the floor (base of the back wall), y down.
    pub floor_y: f64,
    pub wall: Color,
    pub floor: Color,
    pub windows: u8,
    pub time_of_day: TimeOfDay,
    pub door: DoorSide,
}

/// Generated room content in location-local coordinates (origin top-left).
#[derive(Debug, Clone, PartialEq)]
pub struct RoomArt {
    pub palette: BTreeMap<String, Color>,
    pub layers: Vec<ArtLayer>,
    pub anchors: Vec<Anchor>,
    /// Glass rectangle of each window, left to right.
    pub windows: Vec<Rect>,
    /// Door panel rectangle, if any.
    pub door: Option<Rect>,
}

const INK: Color = rgb(43, 49, 48);
const GLASS_DAY: Color = rgb(176, 206, 214);
const GLASS_EVENING: Color = rgb(222, 186, 146);
const GLASS_NIGHT: Color = rgb(70, 84, 110);
const GLASS_RAIN: Color = rgb(134, 150, 160);

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}
fn shade(c: Color, f: f64) -> Color {
    let s = |v: u8| (f64::from(v) * f).round().clamp(0.0, 255.0) as u8;
    Color {
        r: s(c.r),
        g: s(c.g),
        b: s(c.b),
        a: c.a,
    }
}
fn mix(a: Color, b: Color, t: f64) -> Color {
    let m = |x: u8, y: u8| (f64::from(x) * (1.0 - t) + f64::from(y) * t).round() as u8;
    Color {
        r: m(a.r, b.r),
        g: m(a.g, b.g),
        b: m(a.b, b.b),
        a: 255,
    }
}
fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}
fn painted(id: String, depth: Depth, slot: &str, content: Content) -> ArtLayer {
    ArtLayer {
        id,
        depth: Some(depth),
        paint: Some(slot.to_owned()),
        content,
    }
}
fn filled(id: String, depth: Depth, slot: &str, r: Rect) -> ArtLayer {
    painted(
        id,
        depth,
        slot,
        Content::Rectangle {
            rect: r,
            color: INK,
        },
    )
}
fn line(id: String, depth: Depth, slot: &str, a: Point, b: Point, width: f64) -> ArtLayer {
    painted(
        id,
        depth,
        slot,
        Content::Stroke {
            points: vec![a, b],
            color: INK,
            width,
        },
    )
}

impl RoomSpec {
    /// Floor contact line where actors stand (half-way into the floor band).
    pub fn ground_y(&self) -> f64 {
        self.floor_y + 0.5 * (self.height - self.floor_y)
    }
    fn door_rect(&self) -> Option<Rect> {
        let w = 0.094 * self.width;
        let h = 0.42 * self.height;
        let x = match self.door {
            DoorSide::None => return None,
            DoorSide::Left => 0.06 * self.width,
            DoorSide::Right => self.width - 0.06 * self.width - w,
        };
        Some(rect(x, self.floor_y - h, w, h))
    }
    fn window_span(&self) -> (f64, f64) {
        let mut x0 = 0.08 * self.width;
        let mut x1 = 0.92 * self.width;
        if let Some(d) = self.door_rect() {
            match self.door {
                DoorSide::Left => x0 = d.x + d.width + 0.05 * self.width,
                DoorSide::Right => x1 = d.x - 0.05 * self.width,
                DoorSide::None => {}
            }
        }
        (x0, x1)
    }
    pub fn validate(&self) -> Result<(), ManifestError> {
        let fail = |field: &str, reason: &str| ManifestError::InvalidField {
            field: format!("room.{field}"),
            reason: reason.to_owned(),
        };
        for (name, v) in [("width", self.width), ("height", self.height)] {
            if !v.is_finite() || !(MIN_ROOM_SIDE..=MAX_ROOM_SIDE).contains(&v) {
                return Err(fail(name, "must be finite and 64..=8192"));
            }
        }
        if !self.floor_y.is_finite()
            || self.floor_y < 0.5 * self.height
            || self.floor_y > 0.92 * self.height
        {
            return Err(fail("floor_y", "must lie in 0.5..=0.92 of the room height"));
        }
        if self.windows > MAX_WINDOWS {
            return Err(fail("windows", "must be 0..=6"));
        }
        if self.windows > 0 {
            let (x0, x1) = self.window_span();
            let slot = (x1 - x0) / f64::from(self.windows);
            if slot < 0.06 * self.width || slot < MIN_WINDOW_SLOT {
                return Err(fail("windows", "do not fit on the wall next to the door"));
            }
        }
        Ok(())
    }
}

/// Generate the room. Call [`RoomSpec::validate`] first; the output is a pure
/// function of the spec.
pub fn generate(spec: &RoomSpec) -> RoomArt {
    let (w, h, f) = (spec.width, spec.height, spec.floor_y);
    let mut palette = BTreeMap::new();
    let glass = match spec.time_of_day {
        TimeOfDay::Day => GLASS_DAY,
        TimeOfDay::Evening => GLASS_EVENING,
        TimeOfDay::Night => GLASS_NIGHT,
    };
    for (slot, color) in [
        ("wall", spec.wall),
        ("floor", spec.floor),
        ("trim", shade(spec.floor, 0.72)),
        ("floor_line", shade(spec.floor, 0.9)),
        ("frame", INK),
        ("glass", glass),
        ("sill", shade(spec.wall, 0.8)),
        ("door", shade(spec.wall, 0.82)),
        ("door_frame", INK),
        ("handle", rgb(185, 199, 197)),
        (
            "rain",
            Color {
                r: 214,
                g: 224,
                b: 230,
                a: 170,
            },
        ),
        ("clutter_a", rgb(170, 90, 61)),
        ("clutter_b", rgb(241, 236, 223)),
        ("clutter_c", rgb(100, 143, 122)),
    ] {
        palette.insert(slot.to_owned(), color);
    }

    let far = Depth::Far;
    let mut layers = vec![
        filled("wall".into(), far, "wall", rect(0.0, 0.0, w, f)),
        filled("floor".into(), far, "floor", rect(0.0, f, w, h - f)),
    ];
    for (k, t) in [0.18, 0.42, 0.72].into_iter().enumerate() {
        let y = f + (h - f) * t;
        layers.push(line(
            format!("floor_seam_{k}"),
            far,
            "floor_line",
            Point { x: 0.0, y },
            Point { x: w, y },
            2.0,
        ));
    }
    layers.push(filled(
        "skirting".into(),
        far,
        "trim",
        rect(0.0, f - 12.0, w, 12.0),
    ));

    let door = spec.door_rect();
    let mut anchors = vec![Anchor::Ground {
        id: "ground".into(),
        at: Point {
            x: 0.0,
            y: spec.ground_y(),
        },
        width: w,
    }];
    if let Some(d) = door {
        layers.push(filled(
            "door_frame".into(),
            far,
            "door_frame",
            rect(d.x - 10.0, d.y - 10.0, d.width + 20.0, d.height + 10.0),
        ));
        layers.push(filled("door".into(), far, "door", d));
        let hx = if spec.door == DoorSide::Left {
            d.x + d.width * 0.82
        } else {
            d.x + d.width * 0.18
        };
        let hy = d.y + d.height * 0.52;
        layers.push(line(
            "door_handle".into(),
            far,
            "handle",
            Point { x: hx, y: hy },
            Point {
                x: hx,
                y: hy + 0.05 * h,
            },
            8.0,
        ));
        anchors.push(Anchor::Doorway {
            id: "doorway".into(),
            rect: d,
        });
    }

    let mut windows = Vec::new();
    if spec.windows > 0 {
        let (x0, x1) = spec.window_span();
        let n = f64::from(spec.windows);
        let slot = (x1 - x0) / n;
        let ww = (0.135 * w).min(slot * 0.72);
        let wh = 0.3 * h;
        let wy = 0.16 * h;
        for i in 0..spec.windows {
            let wx = x0 + slot * (f64::from(i) + 0.5) - ww / 2.0;
            let g = rect(wx, wy, ww, wh);
            layers.push(filled(
                format!("window_{i}_frame"),
                far,
                "frame",
                rect(wx - 10.0, wy - 10.0, ww + 20.0, wh + 20.0),
            ));
            layers.push(filled(format!("window_{i}_glass"), far, "glass", g));
            layers.push(filled(
                format!("window_{i}_mullion"),
                far,
                "frame",
                rect(wx + ww / 2.0 - 5.0, wy, 10.0, wh),
            ));
            layers.push(filled(
                format!("window_{i}_sill"),
                far,
                "sill",
                rect(wx - 18.0, wy + wh + 10.0, ww + 36.0, 12.0),
            ));
            windows.push(g);
        }
    }
    RoomArt {
        palette,
        layers,
        anchors,
        windows,
        door,
    }
}

/// `day`, `evening`, `rain` and `messy` variants for a generated room.
pub fn standard_variants(spec: &RoomSpec) -> Vec<Variant> {
    let art = generate(spec);
    let (w, h, f) = (spec.width, spec.height, spec.floor_y);
    let mut rain_layers = Vec::new();
    for (i, g) in art.windows.iter().enumerate() {
        for k in 0..4 {
            let x = g.x + g.width * (0.2 + 0.2 * f64::from(k));
            rain_layers.push(line(
                format!("rain_{i}_{k}"),
                Depth::Far,
                "rain",
                Point {
                    x,
                    y: g.y + g.height * 0.12,
                },
                Point {
                    x: x - g.width * 0.1,
                    y: g.y + g.height * 0.88,
                },
                3.0,
            ));
        }
    }
    let floor_at = |t: f64| f + (h - f) * t;
    let messy_layers = vec![
        filled(
            "mess_paper".into(),
            Depth::Mid,
            "clutter_b",
            rect(0.22 * w, floor_at(0.62), 70.0, 10.0),
        ),
        painted(
            "mess_sock".into(),
            Depth::Mid,
            "clutter_a",
            Content::Ellipse {
                rect: rect(0.47 * w, floor_at(0.78), 46.0, 18.0),
                color: INK,
            },
        ),
        filled(
            "mess_book".into(),
            Depth::Mid,
            "clutter_c",
            rect(0.71 * w, floor_at(0.55), 60.0, 14.0),
        ),
        painted(
            "mess_crumple".into(),
            Depth::Mid,
            "clutter_b",
            Content::Ellipse {
                rect: rect(0.86 * w, floor_at(0.85), 30.0, 22.0),
                color: INK,
            },
        ),
    ];
    let palette = |entries: &[(&str, Color)]| {
        entries
            .iter()
            .map(|(k, v)| ((*k).to_owned(), *v))
            .collect::<BTreeMap<_, _>>()
    };
    vec![
        Variant {
            name: "day".into(),
            palette: palette(&[("glass", GLASS_DAY)]),
            layers: Vec::new(),
        },
        Variant {
            name: "evening".into(),
            palette: palette(&[
                ("glass", GLASS_EVENING),
                ("wall", mix(spec.wall, rgb(236, 206, 176), 0.25)),
            ]),
            layers: Vec::new(),
        },
        Variant {
            name: "rain".into(),
            palette: palette(&[("glass", GLASS_RAIN)]),
            layers: rain_layers,
        },
        Variant {
            name: "messy".into(),
            palette: BTreeMap::new(),
            layers: messy_layers,
        },
    ]
}
