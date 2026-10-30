//! Regenerate the stick-studio library backgrounds, locations and sets.
//!
//! Usage (from the workspace root):
//! `cargo run -p motion-library --example generate_stick_library -- examples/stick-studio`
//!
//! * Migrates `backgrounds/*.kilsat` (art layers) to background manifests.
//! * Writes three new hand-authored backgrounds (yard, street, stairs).
//! * Writes room-generated and kit-bashed locations and one set per location.
//!
//! Prop placement reads each referenced prop manifest from `library/props/`
//! (owned by A05) for its bounds and seat anchor; a missing prop falls back to
//! an estimated size and is listed on stderr, so re-run after props change.
//! Output is deterministic.
use motion_art::{Color, Content, Point, Rect};
use motion_library::manifest::{
    Anchor, ArtLayer, CameraSpec, Depth, Geometry, Kind, License, Manifest, Mark, MarkKind, Origin,
    Part, Redistribution, SCHEMA_VERSION,
};
use motion_library::room::{self, DoorSide, RoomSpec, TimeOfDay};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const W: f64 = 1920.0;
const H: f64 = 1080.0;
const INK: Color = rgb(43, 49, 48);
/// `2 × STICK_LIMB_WIDTH` at actor scale 1.
const MAX_LINE: f64 = 18.0;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}
fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}
fn pt(x: f64, y: f64) -> Point {
    Point { x, y }
}
fn canvas() -> Rect {
    rect(0.0, 0.0, W, H)
}
fn license() -> License {
    License {
        license: "MIT".into(),
        author: "Kilsat Studio".into(),
        origin: Origin::Original,
        source_url: None,
        redistribution: Redistribution::Allowed,
        notice: None,
    }
}
fn tags(t: &[&str]) -> Vec<String> {
    t.iter().map(|s| (*s).to_owned()).collect()
}
fn base(id: &str, kind: Kind, name: &str, bounds: Rect, depth: Depth, t: &[&str]) -> Manifest {
    Manifest {
        schema_version: SCHEMA_VERSION,
        id: id.into(),
        kind,
        name: name.into(),
        bounds,
        depth,
        tags: tags(t),
        license: license(),
        palette: BTreeMap::new(),
        geometry: Geometry::Composite,
        anchors: Vec::new(),
        variants: Vec::new(),
        states: Vec::new(),
        room: None,
        parts: Vec::new(),
        location: None,
        cameras: Vec::new(),
        marks: Vec::new(),
        atmosphere: None,
    }
}

// ---------------------------------------------------------------- backgrounds

struct Migration {
    file: &'static str,
    id: &'static str,
    tags: &'static [&'static str],
    /// Layer-id prefixes that are furniture/mid-ground; everything else is far.
    mid: &'static [&'static str],
    anchors: fn() -> Vec<Anchor>,
}

fn ground(y: f64) -> Anchor {
    Anchor::Ground {
        id: "ground".into(),
        at: pt(0.0, y),
        width: W,
    }
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        file: "pysakki",
        id: "background.outdoor.bus_stop_night_v1",
        tags: &["outdoor", "night", "bus_stop", "street", "pysäkki"],
        mid: &[
            "lamp_post",
            "lamp_shade",
            "lamp_light",
            "bench_",
            "sign_",
            "bus_",
            "timetable_",
        ],
        anchors: || {
            vec![
                ground(950.0),
                Anchor::Seat {
                    id: "bench".into(),
                    at: pt(221.0, 826.0),
                },
                Anchor::Stand {
                    id: "sign".into(),
                    at: pt(1560.0, 950.0),
                },
            ]
        },
    },
    Migration {
        file: "hautausmaa",
        id: "background.outdoor.cemetery_night_v1",
        tags: &["outdoor", "night", "cemetery", "hautausmaa"],
        mid: &["tree_", "stone_", "grass_", "moss_"],
        anchors: || {
            vec![
                ground(950.0),
                Anchor::Stand {
                    id: "grave".into(),
                    at: pt(470.0, 950.0),
                },
            ]
        },
    },
    Migration {
        file: "kahvila",
        id: "background.interior.cafe_v1",
        tags: &["interior", "cafe", "kahvila", "day"],
        mid: &["counter_", "machine_", "table_", "chair_"],
        anchors: || {
            vec![
                ground(950.0),
                Anchor::Seat {
                    id: "chair".into(),
                    at: pt(1849.0, 829.0),
                },
                Anchor::Stand {
                    id: "counter".into(),
                    at: pt(440.0, 950.0),
                },
            ]
        },
    },
    Migration {
        file: "toimisto",
        id: "background.interior.office_v1",
        tags: &["interior", "office", "toimisto", "work"],
        mid: &["door_", "side_desk", "desk_", "plant_"],
        anchors: || {
            vec![
                ground(950.0),
                Anchor::Doorway {
                    id: "door".into(),
                    rect: rect(1670.0, 500.0, 160.0, 438.0),
                },
                Anchor::Stand {
                    id: "desk".into(),
                    at: pt(440.0, 950.0),
                },
            ]
        },
    },
];

fn migrate(pack: &Path, m: &Migration) -> Result<Manifest, String> {
    let path = pack.join("backgrounds").join(format!("{}.kilsat", m.file));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let doc: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let source: serde_json::Value = doc["source"]
        .as_str()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    let title = source["project"]["title"]
        .as_str()
        .unwrap_or(m.file)
        .to_owned();
    let prefix = format!("tausta_tausta_{}_", m.file);
    let mut layers = Vec::new();
    for layer in doc["art"]["layers"]
        .as_array()
        .ok_or("art.layers missing")?
    {
        let id = layer["id"].as_str().ok_or("layer id")?;
        let short = id.strip_prefix(&prefix).unwrap_or(id).to_owned();
        let mut content: Content =
            serde_json::from_value(layer["content"].clone()).map_err(|e| e.to_string())?;
        // The library style gate uses actor scale 1 (limb 9 px, lines <= 18 px);
        // the original pack was tuned for actor scale 1.6, so heavier strokes
        // are clamped and reported.
        if let Content::Stroke { width, .. } = &mut content {
            if *width > MAX_LINE {
                eprintln!(
                    "note: {} {short}: stroke width {width} clamped to {MAX_LINE}",
                    m.id
                );
                *width = MAX_LINE;
            }
        }
        let depth = if m.mid.iter().any(|p| short.starts_with(p)) {
            Depth::Mid
        } else {
            Depth::Far
        };
        layers.push(ArtLayer {
            id: short,
            depth: Some(depth),
            parallax: None,
            paint: None,
            content,
        });
    }
    let mut out = base(m.id, Kind::Background, &title, canvas(), Depth::Far, m.tags);
    out.geometry = Geometry::Art { layers };
    out.anchors = (m.anchors)();
    Ok(out)
}

fn layer(id: &str, depth: Depth, content: Content) -> ArtLayer {
    ArtLayer {
        id: id.into(),
        depth: Some(depth),
        parallax: None,
        paint: None,
        content,
    }
}
fn fill(id: &str, depth: Depth, r: Rect, color: Color) -> ArtLayer {
    layer(id, depth, Content::Rectangle { rect: r, color })
}
fn oval(id: &str, depth: Depth, r: Rect, color: Color) -> ArtLayer {
    layer(id, depth, Content::Ellipse { rect: r, color })
}
fn stroke(id: &str, depth: Depth, pts: &[(f64, f64)], color: Color, width: f64) -> ArtLayer {
    layer(
        id,
        depth,
        Content::Stroke {
            points: pts.iter().map(|&(x, y)| pt(x, y)).collect(),
            color,
            width,
        },
    )
}

fn yard() -> Manifest {
    use Depth::{Far, Mid};
    let mut l = vec![
        fill("sky", Far, canvas(), rgb(176, 200, 206)),
        oval(
            "treeline_0",
            Far,
            rect(-80.0, 600.0, 520.0, 220.0),
            rgb(128, 156, 136),
        ),
        oval(
            "treeline_1",
            Far,
            rect(360.0, 620.0, 460.0, 190.0),
            rgb(140, 166, 144),
        ),
        oval(
            "treeline_2",
            Far,
            rect(760.0, 610.0, 420.0, 200.0),
            rgb(128, 156, 136),
        ),
        fill("grass", Far, rect(0.0, 760.0, W, 320.0), rgb(166, 186, 152)),
        fill(
            "path",
            Far,
            rect(1320.0, 760.0, 220.0, 320.0),
            rgb(206, 198, 178),
        ),
        fill(
            "house",
            Far,
            rect(1180.0, 380.0, 560.0, 390.0),
            rgb(214, 200, 180),
        ),
        stroke(
            "roof",
            Far,
            &[(1150.0, 395.0), (1460.0, 230.0), (1770.0, 395.0)],
            INK,
            14.0,
        ),
        fill(
            "house_door_frame",
            Far,
            rect(1370.0, 560.0, 140.0, 210.0),
            INK,
        ),
        fill(
            "house_door",
            Far,
            rect(1380.0, 570.0, 120.0, 200.0),
            rgb(161, 125, 86),
        ),
        fill(
            "house_window_0",
            Far,
            rect(1220.0, 470.0, 110.0, 90.0),
            rgb(176, 206, 214),
        ),
        fill(
            "house_window_1",
            Far,
            rect(1590.0, 470.0, 110.0, 90.0),
            rgb(176, 206, 214),
        ),
    ];
    for i in 0..9 {
        let x = 40.0 + 120.0 * f64::from(i);
        l.push(stroke(
            &format!("fence_post_{i}"),
            Mid,
            &[(x, 690.0), (x, 800.0)],
            rgb(150, 130, 110),
            8.0,
        ));
    }
    l.push(stroke(
        "fence_rail_top",
        Mid,
        &[(20.0, 715.0), (1020.0, 715.0)],
        rgb(150, 130, 110),
        6.0,
    ));
    l.push(stroke(
        "fence_rail_low",
        Mid,
        &[(20.0, 765.0), (1020.0, 765.0)],
        rgb(150, 130, 110),
        6.0,
    ));
    let mut m = base(
        "background.outdoor.yard_v1",
        Kind::Background,
        "Piha",
        canvas(),
        Depth::Far,
        &["outdoor", "day", "yard", "piha", "garden"],
    );
    m.geometry = Geometry::Art { layers: l };
    m.anchors = vec![
        ground(945.0),
        Anchor::Doorway {
            id: "house_door".into(),
            rect: rect(1380.0, 570.0, 120.0, 200.0),
        },
    ];
    m
}

fn street() -> Manifest {
    use Depth::Far;
    let mut l = vec![fill("sky", Far, canvas(), rgb(168, 186, 198))];
    let buildings = [
        (0.0, 260.0, 380.0, rgb(196, 190, 180)),
        (400.0, 200.0, 340.0, rgb(180, 186, 190)),
        (760.0, 300.0, 400.0, rgb(204, 196, 184)),
        (1180.0, 230.0, 330.0, rgb(184, 190, 192)),
        (1530.0, 280.0, 390.0, rgb(198, 192, 182)),
    ];
    for (i, (x, y, w, c)) in buildings.into_iter().enumerate() {
        l.push(fill(
            &format!("building_{i}"),
            Far,
            rect(x, y, w, 820.0 - y),
            c,
        ));
        for row in 0..3 {
            for col in 0..3 {
                let wx = x + 40.0 + f64::from(col) * (w - 80.0) / 3.0;
                let wy = y + 50.0 + f64::from(row) * 130.0;
                l.push(fill(
                    &format!("building_{i}_window_{row}_{col}"),
                    Far,
                    rect(wx, wy, (w - 80.0) / 3.0 - 30.0, 80.0),
                    rgb(150, 172, 180),
                ));
            }
        }
    }
    l.push(fill(
        "sidewalk",
        Far,
        rect(0.0, 800.0, W, 200.0),
        rgb(176, 180, 176),
    ));
    l.push(stroke(
        "curb",
        Far,
        &[(0.0, 1000.0), (W, 1000.0)],
        rgb(110, 124, 128),
        6.0,
    ));
    l.push(fill(
        "road",
        Far,
        rect(0.0, 1003.0, W, 77.0),
        rgb(104, 126, 132),
    ));
    for i in 0..4 {
        l.push(fill(
            &format!("road_dash_{i}"),
            Far,
            rect(120.0 + 480.0 * f64::from(i), 1040.0, 220.0, 8.0),
            rgb(218, 220, 198),
        ));
    }
    for i in 0..6 {
        let x = 160.0 + 320.0 * f64::from(i);
        l.push(stroke(
            &format!("paving_seam_{i}"),
            Far,
            &[(x, 800.0), (x - 40.0, 1000.0)],
            rgb(156, 160, 156),
            2.0,
        ));
    }
    let mut m = base(
        "background.outdoor.street_v1",
        Kind::Background,
        "Katu",
        canvas(),
        Depth::Far,
        &["outdoor", "day", "street", "katu", "city"],
    );
    m.geometry = Geometry::Art { layers: l };
    m.anchors = vec![ground(940.0)];
    m
}

fn stairs() -> Manifest {
    use Depth::Mid;
    let (w, h) = (600.0, 420.0);
    let mut l = Vec::new();
    let steps = 6;
    let step_w = w / f64::from(steps);
    let step_h = h / f64::from(steps);
    for i in 0..steps {
        let x = step_w * f64::from(i);
        let top = h - step_h * f64::from(i + 1);
        l.push(fill(
            &format!("step_{i}"),
            Mid,
            rect(x, top, w - x, step_h),
            rgb(190, 178, 160),
        ));
        l.push(stroke(
            &format!("step_{i}_edge"),
            Mid,
            &[(x, top), (w, top)],
            rgb(140, 128, 112),
            4.0,
        ));
    }
    l.push(stroke(
        "handrail",
        Mid,
        &[(10.0, h - 160.0), (w - 10.0, 10.0)],
        INK,
        8.0,
    ));
    for i in 0..4 {
        let x = 40.0 + 150.0 * f64::from(i);
        let rail_y = h - 160.0 - (x - 10.0) * (h - 170.0) / (w - 20.0);
        let step = ((x / step_w).floor() + 1.0).min(f64::from(steps));
        l.push(stroke(
            &format!("baluster_{i}"),
            Mid,
            &[(x, rail_y), (x, h - step_h * step)],
            INK,
            4.0,
        ));
    }
    let mut m = base(
        "background.interior.stairs_v1",
        Kind::Background,
        "Portaat",
        rect(0.0, 0.0, w, h),
        Depth::Mid,
        &["interior", "stairs", "portaat", "architecture"],
    );
    m.geometry = Geometry::Art { layers: l };
    m.anchors = vec![
        Anchor::Ground {
            id: "ground".into(),
            at: pt(0.0, h),
            width: w,
        },
        Anchor::Stand {
            id: "top_step".into(),
            at: pt(w - step_w / 2.0, step_h),
        },
    ];
    m
}

// ---------------------------------------------------------------- props (A05)

struct PropInfo {
    width: f64,
    height: f64,
    seat: Option<String>,
    found: bool,
}

/// Estimated sizes used until A05's manifest exists.
fn fallback_size(id: &str) -> (f64, f64) {
    match id {
        "prop.furniture.chair_wood_v1" => (110.0, 220.0),
        "prop.furniture.office_chair_v1" => (120.0, 230.0),
        "prop.furniture.stool_v1" => (80.0, 150.0),
        "prop.furniture.sofa_v1" => (420.0, 220.0),
        "prop.furniture.armchair_v1" => (200.0, 210.0),
        "prop.furniture.bench_v1" => (380.0, 140.0),
        "prop.furniture.table_dining_v1" => (360.0, 170.0),
        "prop.furniture.desk_v1" => (320.0, 170.0),
        "prop.furniture.coffee_table_v1" => (240.0, 90.0),
        "prop.furniture.bed_v1" => (480.0, 200.0),
        "prop.furniture.bookshelf_v1" => (220.0, 420.0),
        "prop.furniture.cabinet_v1" => (180.0, 240.0),
        "prop.furniture.counter_v1" => (400.0, 190.0),
        "prop.kitchen.fridge_v1" => (180.0, 380.0),
        "prop.kitchen.stove_v1" => (170.0, 190.0),
        "prop.kitchen.sink_v1" => (200.0, 190.0),
        "prop.electronics.tv_v1" => (260.0, 170.0),
        "prop.electronics.monitor_v1" => (110.0, 90.0),
        "prop.decor.plant_pot_v1" => (90.0, 160.0),
        "prop.decor.floor_lamp_v1" => (90.0, 380.0),
        "prop.decor.picture_frame_v1" => (120.0, 90.0),
        "prop.decor.rug_v1" => (420.0, 30.0),
        "prop.decor.wall_clock_v1" => (90.0, 90.0),
        "prop.decor.coat_rack_v1" => (90.0, 360.0),
        "prop.decor.trash_bin_v1" => (70.0, 100.0),
        "prop.architecture.door_v1" => (180.0, 450.0),
        "prop.architecture.window_v1" => (240.0, 300.0),
        "prop.sauna.bench_v1" => (500.0, 200.0),
        "prop.sauna.stove_v1" => (160.0, 220.0),
        "prop.outdoor.street_lamp_v1" => (90.0, 600.0),
        "prop.outdoor.tree_v1" => (360.0, 620.0),
        "prop.outdoor.bus_stop_shelter_v1" => (520.0, 420.0),
        "prop.classroom.blackboard_v1" => (520.0, 300.0),
        "prop.shop.shelf_v1" => (300.0, 380.0),
        _ => (100.0, 100.0),
    }
}

struct Ctx {
    props_dir: PathBuf,
    cache: BTreeMap<String, PropInfo>,
}
impl Ctx {
    fn prop(&mut self, id: &str) -> &PropInfo {
        if !self.cache.contains_key(id) {
            let path = self.props_dir.join(format!("{id}.json"));
            let parsed = std::fs::read(&path)
                .ok()
                .and_then(|b| Manifest::from_json(&b).ok());
            let info = match parsed {
                Some(m) => PropInfo {
                    width: m.bounds.width,
                    height: m.bounds.height,
                    seat: m.anchors.iter().find_map(|a| match a {
                        Anchor::Seat { id, .. } => Some(id.clone()),
                        _ => None,
                    }),
                    found: true,
                },
                None => {
                    let (width, height) = fallback_size(id);
                    PropInfo {
                        width,
                        height,
                        seat: None,
                        found: false,
                    }
                }
            };
            self.cache.insert(id.to_owned(), info);
        }
        &self.cache[id]
    }
}

/// How a part is placed vertically.
#[derive(Clone, Copy)]
enum Place {
    /// Bottom edge on the floor contact line, plus an offset.
    Floor(f64),
    /// Top edge at a wall height.
    Wall(f64),
    /// Bottom edge on the top of an earlier instance.
    On(&'static str),
}
struct Spot {
    instance: &'static str,
    asset: &'static str,
    /// Horizontal centre in world pixels.
    x: f64,
    place: Place,
    flip: bool,
}
const fn floor(instance: &'static str, asset: &'static str, x: f64) -> Spot {
    Spot {
        instance,
        asset,
        x,
        place: Place::Floor(0.0),
        flip: false,
    }
}
const fn wall(instance: &'static str, asset: &'static str, x: f64, y: f64) -> Spot {
    Spot {
        instance,
        asset,
        x,
        place: Place::Wall(y),
        flip: false,
    }
}
const fn flipped(mut s: Spot) -> Spot {
    s.flip = true;
    s
}
const fn back(mut s: Spot, dy: f64) -> Spot {
    s.place = Place::Floor(dy);
    s
}
const fn on(instance: &'static str, asset: &'static str, x: f64, under: &'static str) -> Spot {
    Spot {
        instance,
        asset,
        x,
        place: Place::On(under),
        flip: false,
    }
}

fn place_parts(ctx: &mut Ctx, ground_y: f64, spots: &[Spot]) -> Vec<Part> {
    let mut tops: BTreeMap<&str, f64> = BTreeMap::new();
    let mut parts = Vec::new();
    for s in spots {
        let info = ctx.prop(s.asset);
        let (w, h) = (info.width, info.height);
        let top = match s.place {
            Place::Floor(dy) => ground_y + dy - h,
            Place::Wall(y) => y,
            Place::On(under) => tops.get(under).copied().unwrap_or(ground_y) - h,
        };
        tops.insert(s.instance, top);
        parts.push(Part {
            instance: s.instance.into(),
            asset: s.asset.into(),
            at: pt(round1(s.x - w / 2.0), round1(top)),
            scale: None,
            flip_x: s.flip,
            depth: None,
            parallax: None,
            variant: None,
        });
    }
    parts
}
fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

// ---------------------------------------------------------------- locations

struct RoomLocation {
    id: &'static str,
    name: &'static str,
    tags: &'static [&'static str],
    wall: Color,
    floor: Color,
    windows: u8,
    time: TimeOfDay,
    door: DoorSide,
    spots: &'static [Spot],
    /// (mark id, kind, instance or "" for a floor point, x for floor points)
    marks: &'static [(&'static str, MarkKind, &'static str, f64)],
}

use MarkKind::{Sit, Stand};
const ROOMS: &[RoomLocation] = &[
    RoomLocation {
        id: "location.home.kitchen_v1",
        name: "Keittiö",
        tags: &["interior", "home", "kitchen", "keittiö"],
        wall: rgb(226, 220, 204),
        floor: rgb(190, 172, 146),
        windows: 1,
        time: TimeOfDay::Day,
        door: DoorSide::Right,
        spots: &[
            back(floor("fridge", "prop.kitchen.fridge_v1", 160.0), -40.0),
            back(floor("counter", "prop.furniture.counter_v1", 410.0), -40.0),
            back(floor("sink", "prop.kitchen.sink_v1", 660.0), -40.0),
            back(floor("stove", "prop.kitchen.stove_v1", 830.0), -40.0),
            wall("clock", "prop.decor.wall_clock_v1", 1240.0, 220.0),
            floor("table", "prop.furniture.table_dining_v1", 1250.0),
            floor("chair_left", "prop.furniture.chair_wood_v1", 1080.0),
            flipped(floor("chair_right", "prop.furniture.chair_wood_v1", 1420.0)),
            floor("bin", "prop.decor.trash_bin_v1", 1560.0),
        ],
        marks: &[
            ("cook", Stand, "", 740.0),
            ("sit_left", Sit, "chair_left", 0.0),
            ("sit_right", Sit, "chair_right", 0.0),
            ("door", Stand, "room", 0.0),
        ],
    },
    RoomLocation {
        id: "location.home.living_room_v1",
        name: "Olohuone",
        tags: &["interior", "home", "living_room", "olohuone"],
        wall: rgb(222, 216, 206),
        floor: rgb(186, 164, 136),
        windows: 2,
        time: TimeOfDay::Evening,
        door: DoorSide::Left,
        spots: &[
            back(floor("rug", "prop.decor.rug_v1", 1000.0), 20.0),
            wall("picture", "prop.decor.picture_frame_v1", 1000.0, 260.0),
            back(floor("lamp", "prop.decor.floor_lamp_v1", 640.0), -30.0),
            back(floor("sofa", "prop.furniture.sofa_v1", 1000.0), -30.0),
            floor("armchair", "prop.furniture.armchair_v1", 1420.0),
            floor("coffee_table", "prop.furniture.coffee_table_v1", 1010.0),
            back(floor("tv", "prop.electronics.tv_v1", 1720.0), -20.0),
            floor("plant", "prop.decor.plant_pot_v1", 1860.0),
        ],
        marks: &[
            ("sit_sofa", Sit, "sofa", 0.0),
            ("sit_armchair", Sit, "armchair", 0.0),
            ("stand_centre", Stand, "", 760.0),
            ("door", Stand, "room", 0.0),
        ],
    },
    RoomLocation {
        id: "location.home.bedroom_v1",
        name: "Makuuhuone",
        tags: &["interior", "home", "bedroom", "makuuhuone", "night"],
        wall: rgb(208, 210, 214),
        floor: rgb(178, 160, 140),
        windows: 1,
        time: TimeOfDay::Night,
        door: DoorSide::Right,
        spots: &[
            back(floor("rug", "prop.decor.rug_v1", 820.0), 20.0),
            wall("picture", "prop.decor.picture_frame_v1", 400.0, 300.0),
            back(floor("bed", "prop.furniture.bed_v1", 760.0), -30.0),
            floor("cabinet", "prop.furniture.cabinet_v1", 380.0),
            floor("lamp", "prop.decor.floor_lamp_v1", 1110.0),
        ],
        marks: &[
            ("sit_bed", Sit, "bed", 0.0),
            ("stand_window", Stand, "", 1300.0),
            ("door", Stand, "room", 0.0),
        ],
    },
    RoomLocation {
        id: "location.home.sauna_v1",
        name: "Sauna",
        tags: &["interior", "home", "sauna", "suomi", "finland"],
        wall: rgb(204, 180, 146),
        floor: rgb(172, 162, 148),
        windows: 1,
        time: TimeOfDay::Evening,
        door: DoorSide::Left,
        spots: &[
            back(floor("stove", "prop.sauna.stove_v1", 520.0), -30.0),
            floor("bench", "prop.sauna.bench_v1", 1180.0),
        ],
        marks: &[
            ("sit_bench", Sit, "bench", 0.0),
            ("stove", Stand, "", 700.0),
            ("door", Stand, "room", 0.0),
        ],
    },
    RoomLocation {
        id: "location.school.classroom_v1",
        name: "Luokkahuone",
        tags: &["interior", "school", "classroom", "luokka"],
        wall: rgb(220, 222, 210),
        floor: rgb(184, 170, 150),
        windows: 1,
        time: TimeOfDay::Day,
        door: DoorSide::Right,
        spots: &[
            wall("blackboard", "prop.classroom.blackboard_v1", 380.0, 200.0),
            wall("clock", "prop.decor.wall_clock_v1", 1460.0, 160.0),
            back(
                floor("teacher_desk", "prop.furniture.desk_v1", 380.0),
                -30.0,
            ),
            floor("desk_a", "prop.furniture.desk_v1", 860.0),
            floor("chair_a", "prop.furniture.chair_wood_v1", 800.0),
            floor("desk_b", "prop.furniture.desk_v1", 1240.0),
            floor("chair_b", "prop.furniture.chair_wood_v1", 1180.0),
            back(floor("shelf", "prop.furniture.bookshelf_v1", 1500.0), -40.0),
        ],
        marks: &[
            ("teacher", Stand, "", 620.0),
            ("pupil_a", Sit, "chair_a", 0.0),
            ("pupil_b", Sit, "chair_b", 0.0),
            ("door", Stand, "room", 0.0),
        ],
    },
    RoomLocation {
        id: "location.public.corridor_v1",
        name: "Käytävä",
        tags: &["interior", "public", "corridor", "käytävä", "hallway"],
        wall: rgb(214, 216, 210),
        floor: rgb(176, 170, 158),
        windows: 0,
        time: TimeOfDay::Day,
        door: DoorSide::Left,
        spots: &[
            back(floor("door_b", "prop.architecture.door_v1", 1500.0), -50.0),
            wall("picture", "prop.decor.picture_frame_v1", 900.0, 300.0),
            floor("coat_rack", "prop.decor.coat_rack_v1", 560.0),
            floor("bench", "prop.furniture.bench_v1", 940.0),
            floor("plant", "prop.decor.plant_pot_v1", 1250.0),
            floor("bin", "prop.decor.trash_bin_v1", 1760.0),
        ],
        marks: &[
            ("sit_bench", Sit, "bench", 0.0),
            ("door_a", Stand, "room", 0.0),
            ("walk_mid", Stand, "", 1150.0),
        ],
    },
    RoomLocation {
        id: "location.public.waiting_room_v1",
        name: "Odotushuone",
        tags: &["interior", "public", "waiting_room", "odotushuone"],
        wall: rgb(218, 222, 216),
        floor: rgb(180, 176, 164),
        windows: 1,
        time: TimeOfDay::Day,
        door: DoorSide::Right,
        spots: &[
            wall("clock", "prop.decor.wall_clock_v1", 1300.0, 170.0),
            back(floor("cabinet", "prop.furniture.cabinet_v1", 300.0), -40.0),
            on("tv", "prop.electronics.tv_v1", 300.0, "cabinet"),
            floor("chair_a", "prop.furniture.chair_wood_v1", 520.0),
            floor("chair_b", "prop.furniture.chair_wood_v1", 680.0),
            floor("chair_c", "prop.furniture.chair_wood_v1", 840.0),
            floor("coffee_table", "prop.furniture.coffee_table_v1", 1100.0),
            floor("plant", "prop.decor.plant_pot_v1", 1320.0),
        ],
        marks: &[
            ("sit_a", Sit, "chair_a", 0.0),
            ("sit_c", Sit, "chair_c", 0.0),
            ("door", Stand, "room", 0.0),
        ],
    },
    RoomLocation {
        id: "location.commercial.shop_v1",
        name: "Kauppa",
        tags: &["interior", "commercial", "shop", "kauppa", "store"],
        wall: rgb(224, 222, 212),
        floor: rgb(186, 182, 170),
        windows: 2,
        time: TimeOfDay::Day,
        door: DoorSide::Left,
        spots: &[
            back(floor("shelf_a", "prop.shop.shelf_v1", 900.0), -40.0),
            back(floor("shelf_b", "prop.shop.shelf_v1", 1240.0), -40.0),
            floor("counter", "prop.furniture.counter_v1", 1640.0),
            floor("bin", "prop.decor.trash_bin_v1", 620.0),
        ],
        marks: &[
            ("cashier", Stand, "", 1700.0),
            ("customer", Stand, "", 1080.0),
            ("door", Stand, "room", 0.0),
        ],
    },
    RoomLocation {
        id: "location.public.library_v1",
        name: "Kirjasto",
        tags: &["interior", "public", "library", "kirjasto", "books"],
        wall: rgb(222, 214, 198),
        floor: rgb(176, 156, 130),
        windows: 1,
        time: TimeOfDay::Day,
        door: DoorSide::Right,
        spots: &[
            back(
                floor("shelf_a", "prop.furniture.bookshelf_v1", 160.0),
                -40.0,
            ),
            back(
                floor("shelf_b", "prop.furniture.bookshelf_v1", 400.0),
                -40.0,
            ),
            back(
                floor("shelf_c", "prop.furniture.bookshelf_v1", 1450.0),
                -40.0,
            ),
            floor("lamp", "prop.decor.floor_lamp_v1", 640.0),
            floor("desk", "prop.furniture.desk_v1", 1100.0),
            floor("chair", "prop.furniture.chair_wood_v1", 1100.0),
        ],
        marks: &[
            ("reader", Sit, "chair", 0.0),
            ("browse", Stand, "", 300.0),
            ("door", Stand, "room", 0.0),
        ],
    },
    RoomLocation {
        id: "location.public.stairwell_v1",
        name: "Porraskäytävä",
        tags: &["interior", "public", "stairwell", "porraskäytävä", "stairs"],
        wall: rgb(212, 214, 206),
        floor: rgb(178, 172, 160),
        windows: 0,
        time: TimeOfDay::Day,
        door: DoorSide::Left,
        spots: &[
            back(floor("door_b", "prop.architecture.door_v1", 1020.0), -50.0),
            floor("bin", "prop.decor.trash_bin_v1", 760.0),
        ],
        marks: &[
            ("landing", Stand, "", 900.0),
            ("door", Stand, "room", 0.0),
            ("top", Stand, "stairs", 0.0),
        ],
    },
    RoomLocation {
        id: "location.work.home_office_v1",
        name: "Kotitoimisto",
        tags: &["interior", "home", "work", "office", "kotitoimisto"],
        wall: rgb(220, 218, 210),
        floor: rgb(182, 166, 144),
        windows: 1,
        time: TimeOfDay::Day,
        door: DoorSide::None,
        spots: &[
            back(floor("shelf", "prop.furniture.bookshelf_v1", 360.0), -40.0),
            floor("desk", "prop.furniture.desk_v1", 960.0),
            on("monitor", "prop.electronics.monitor_v1", 960.0, "desk"),
            floor("chair", "prop.furniture.office_chair_v1", 960.0),
            floor("plant", "prop.decor.plant_pot_v1", 1300.0),
            back(floor("lamp", "prop.decor.floor_lamp_v1", 1560.0), -30.0),
        ],
        marks: &[
            ("worker", Sit, "chair", 0.0),
            ("visitor", Stand, "", 1420.0),
        ],
    },
    RoomLocation {
        id: "location.work.meeting_room_v1",
        name: "Neuvotteluhuone",
        tags: &[
            "interior",
            "work",
            "office",
            "meeting_room",
            "neuvotteluhuone",
        ],
        wall: rgb(224, 226, 220),
        floor: rgb(176, 172, 162),
        windows: 1,
        time: TimeOfDay::Day,
        door: DoorSide::Right,
        spots: &[
            back(floor("cabinet", "prop.furniture.cabinet_v1", 300.0), -40.0),
            on("tv", "prop.electronics.tv_v1", 300.0, "cabinet"),
            floor("table", "prop.furniture.table_dining_v1", 960.0),
            floor("chair_a", "prop.furniture.office_chair_v1", 720.0),
            flipped(floor("chair_b", "prop.furniture.office_chair_v1", 1200.0)),
            floor("plant", "prop.decor.plant_pot_v1", 1420.0),
        ],
        marks: &[
            ("presenter", Stand, "", 520.0),
            ("sit_a", Sit, "chair_a", 0.0),
            ("sit_b", Sit, "chair_b", 0.0),
            ("door", Stand, "room", 0.0),
        ],
    },
];

fn room_location(ctx: &mut Ctx, r: &RoomLocation) -> (Manifest, Vec<Mark>, f64) {
    let spec = RoomSpec {
        width: W,
        height: H,
        floor_y: 810.0,
        wall: r.wall,
        floor: r.floor,
        windows: r.windows,
        time_of_day: r.time,
        door: r.door,
    };
    let ground_y = spec.ground_y();
    let mut m = base(r.id, Kind::Location, r.name, canvas(), Depth::Far, r.tags);
    m.variants = room::standard_variants(&spec);
    m.room = Some(spec);
    m.parts = place_parts(ctx, ground_y, r.spots);
    if r.id == "location.public.stairwell_v1" {
        // Kit-bash: stairs background piece on the right.
        m.parts.push(Part {
            instance: "stairs".into(),
            asset: "background.interior.stairs_v1".into(),
            at: pt(1260.0, ground_y - 420.0),
            scale: None,
            flip_x: false,
            depth: None,
            parallax: None,
            variant: None,
        });
    }
    let marks = r
        .marks
        .iter()
        .map(|&(id, kind, instance, x)| {
            let anchor = match (instance, kind) {
                ("", _) => None,
                ("room", _) => Some("room/doorway".to_owned()),
                ("stairs", _) => Some("stairs/top_step".to_owned()),
                (inst, Sit) => {
                    let asset = r
                        .spots
                        .iter()
                        .find(|s| s.instance == inst)
                        .map(|s| s.asset)
                        .unwrap_or_default();
                    let seat = ctx.prop(asset).seat.clone().unwrap_or("seat".into());
                    Some(format!("{inst}/{seat}"))
                }
                (inst, Stand) => Some(format!("{inst}/ground")),
            };
            Mark {
                id: id.into(),
                kind,
                at: anchor.is_none().then(|| pt(x, ground_y)),
                anchor,
            }
        })
        .collect();
    (m, marks, ground_y)
}

struct KitLocation {
    id: &'static str,
    name: &'static str,
    tags: &'static [&'static str],
    background: &'static str,
    ground_y: f64,
    spots: &'static [Spot],
    marks: &'static [(&'static str, MarkKind, Option<&'static str>, f64)],
}
const KITS: &[KitLocation] = &[
    KitLocation {
        id: "location.outdoor.bus_stop_night_v1",
        name: "Yövuoron pysäkki",
        tags: &["outdoor", "night", "bus_stop", "pysäkki"],
        background: "background.outdoor.bus_stop_night_v1",
        ground_y: 950.0,
        spots: &[],
        marks: &[
            ("sit_bench", Sit, Some("backdrop/bench"), 0.0),
            ("wait", Stand, None, 760.0),
            ("sign", Stand, Some("backdrop/sign"), 0.0),
        ],
    },
    KitLocation {
        id: "location.outdoor.cemetery_night_v1",
        name: "Hautausmaa",
        tags: &["outdoor", "night", "cemetery", "hautausmaa"],
        background: "background.outdoor.cemetery_night_v1",
        ground_y: 950.0,
        spots: &[],
        marks: &[
            ("grave", Stand, Some("backdrop/grave"), 0.0),
            ("path", Stand, None, 1100.0),
        ],
    },
    KitLocation {
        id: "location.commercial.cafe_v1",
        name: "Kahvila",
        tags: &["interior", "commercial", "cafe", "kahvila"],
        background: "background.interior.cafe_v1",
        ground_y: 950.0,
        spots: &[],
        marks: &[
            ("barista", Stand, Some("backdrop/counter"), 0.0),
            ("sit_chair", Sit, Some("backdrop/chair"), 0.0),
            ("customer", Stand, None, 900.0),
        ],
    },
    KitLocation {
        id: "location.work.office_v1",
        name: "Toimisto",
        tags: &["interior", "work", "office", "toimisto"],
        background: "background.interior.office_v1",
        ground_y: 950.0,
        spots: &[],
        marks: &[
            ("desk", Stand, Some("backdrop/desk"), 0.0),
            ("door", Stand, Some("backdrop/door"), 0.0),
            ("centre", Stand, None, 1000.0),
        ],
    },
    KitLocation {
        id: "location.outdoor.yard_v1",
        name: "Piha",
        tags: &["outdoor", "day", "yard", "piha"],
        background: "background.outdoor.yard_v1",
        ground_y: 945.0,
        spots: &[
            back(floor("tree", "prop.outdoor.tree_v1", 260.0), -60.0),
            floor("bench", "prop.furniture.bench_v1", 760.0),
            back(floor("lamp", "prop.outdoor.street_lamp_v1", 1100.0), -40.0),
        ],
        marks: &[
            ("sit_bench", Sit, Some("bench"), 0.0),
            ("house_door", Stand, Some("backdrop/house_door"), 0.0),
            ("lawn", Stand, None, 1700.0),
        ],
    },
    KitLocation {
        id: "location.outdoor.street_v1",
        name: "Katu",
        tags: &["outdoor", "day", "street", "katu"],
        background: "background.outdoor.street_v1",
        ground_y: 940.0,
        spots: &[
            back(floor("lamp_a", "prop.outdoor.street_lamp_v1", 200.0), -50.0),
            back(
                floor("shelter", "prop.outdoor.bus_stop_shelter_v1", 760.0),
                -40.0,
            ),
            back(floor("tree", "prop.outdoor.tree_v1", 1380.0), -60.0),
            floor("bin", "prop.decor.trash_bin_v1", 1120.0),
            back(
                floor("lamp_b", "prop.outdoor.street_lamp_v1", 1760.0),
                -50.0,
            ),
        ],
        marks: &[
            ("sit_shelter", Sit, Some("shelter"), 0.0),
            ("walk_left", Stand, None, 420.0),
            ("walk_right", Stand, None, 1560.0),
        ],
    },
];

fn kit_location(ctx: &mut Ctx, k: &KitLocation) -> (Manifest, Vec<Mark>, f64) {
    let mut m = base(k.id, Kind::Location, k.name, canvas(), Depth::Far, k.tags);
    m.parts.push(Part {
        instance: "backdrop".into(),
        asset: k.background.into(),
        at: pt(0.0, 0.0),
        scale: None,
        flip_x: false,
        depth: None,
        parallax: None,
        variant: None,
    });
    m.parts.extend(place_parts(ctx, k.ground_y, k.spots));
    let marks = k
        .marks
        .iter()
        .map(|&(id, kind, anchor, x)| {
            let anchor = anchor.map(|a| {
                if a.contains('/') {
                    a.to_owned()
                } else {
                    let asset = k
                        .spots
                        .iter()
                        .find(|s| s.instance == a)
                        .map(|s| s.asset)
                        .unwrap_or_default();
                    let seat = ctx.prop(asset).seat.clone().unwrap_or("seat".into());
                    format!("{a}/{seat}")
                }
            });
            Mark {
                id: id.into(),
                kind,
                at: anchor.is_none().then(|| pt(x, k.ground_y)),
                anchor,
            }
        })
        .collect();
    (m, marks, k.ground_y)
}

// ---------------------------------------------------------------- sets

/// Camera centred horizontally on `x`, with the frame kept inside the canvas.
fn camera(id: &str, x: f64, y: f64, zoom: f64) -> CameraSpec {
    let half_w = W / 2.0 / zoom;
    let half_h = H / 2.0 / zoom;
    let cx = x.clamp(half_w, W - half_w);
    let cy = y.clamp(half_h, H - half_h);
    CameraSpec {
        id: id.into(),
        x: round1(cx - W / 2.0),
        y: round1(cy - H / 2.0),
        zoom,
    }
}

/// Approximate world x of a mark for camera framing (exact for `at` marks).
fn mark_x(loc: &Manifest, mark: &Mark, ctx: &mut Ctx) -> f64 {
    if let Some(at) = mark.at {
        return at.x;
    }
    let Some((instance, _)) = mark.anchor_ref() else {
        return W / 2.0;
    };
    if instance == "room" {
        if let Some(art) = loc.room_art() {
            if let Some(d) = art.door {
                return d.x + d.width / 2.0;
            }
        }
    }
    if let Some(p) = loc.parts.iter().find(|p| p.instance == instance) {
        if p.asset.starts_with("prop.") {
            let w = ctx.prop(&p.asset).width;
            return p.at.x + w / 2.0;
        }
    }
    W / 2.0
}

fn make_set(ctx: &mut Ctx, loc: &Manifest, marks: Vec<Mark>, ground_y: f64) -> Manifest {
    let set_id = loc.id.replacen("location.", "set.", 1);
    let mut tags = loc.tags.clone();
    tags.push("set".into());
    let mut m = base(
        &set_id,
        Kind::Set,
        &format!("{} (lavaste)", loc.name),
        canvas(),
        Depth::Far,
        &[],
    );
    m.tags = tags;
    m.location = Some(loc.id.clone());
    let focus = marks.first().map_or(W / 2.0, |mk| mark_x(loc, mk, ctx));
    let second = marks.get(1).map_or(focus, |mk| mark_x(loc, mk, ctx));
    m.cameras = vec![
        camera("wide", W / 2.0, H / 2.0, 1.0),
        camera("two_shot", (focus + second) / 2.0, ground_y - 220.0, 1.4),
        camera("medium", focus, ground_y - 220.0, 1.8),
        camera("close", focus, ground_y - 300.0, 2.6),
    ];
    m.marks = marks;
    m
}

fn write(dir: &Path, m: &Manifest) -> Result<(), String> {
    let path = dir.join(format!("{}.json", m.id));
    let text = m.to_json_pretty().map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
}

fn run(pack: &Path) -> Result<(), String> {
    let lib = pack.join("library");
    let dirs = [Kind::Background, Kind::Location, Kind::Set].map(|k| lib.join(k.directory()));
    for d in &dirs {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    for m in MIGRATIONS {
        write(&dirs[0], &migrate(pack, m)?)?;
    }
    for m in [yard(), street(), stairs()] {
        write(&dirs[0], &m)?;
    }
    let mut ctx = Ctx {
        props_dir: lib.join("props"),
        cache: BTreeMap::new(),
    };
    let mut built = Vec::new();
    for r in ROOMS {
        built.push(room_location(&mut ctx, r));
    }
    for k in KITS {
        built.push(kit_location(&mut ctx, k));
    }
    for (loc, marks, ground_y) in built {
        let set = make_set(&mut ctx, &loc, marks, ground_y);
        write(&dirs[1], &loc)?;
        write(&dirs[2], &set)?;
    }
    let missing: Vec<&String> = ctx
        .cache
        .iter()
        .filter(|(_, i)| !i.found)
        .map(|(id, _)| id)
        .collect();
    if !missing.is_empty() {
        eprintln!(
            "warning: {} prop manifest(s) missing; placed with estimated sizes:",
            missing.len()
        );
        for id in missing {
            eprintln!("  {id}");
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let Some(pack) = std::env::args_os().nth(1) else {
        eprintln!("usage: generate_stick_library <pack_root>");
        return ExitCode::from(2);
    };
    match run(Path::new(&pack)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
