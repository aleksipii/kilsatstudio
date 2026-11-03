//! Desktop side of the high-level story format (`kilsat_story: 1`): the
//! embedded stick-studio catalog (six characters, thirteen painted sets) and the
//! assembly of a compiled story into a project with per-scene set artwork.
//! Everything is embedded; building a project performs no I/O.
use crate::project_history::MAX_SOURCE_BYTES;
use motion_script::story::{
    self, CatalogCharacter, CatalogSet, IssueKind, StoryCatalog, StoryCompilation, StoryIssue,
};
use std::collections::{BTreeMap, BTreeSet};

const MANIFEST: &str = include_str!("../../../examples/stick-studio/manifest.json");

/// `(id, description for the assistant, project)` for each painted set.
/// The first four are the original sets; the rest are the city (pack revision 3).
const SETS: [(&str, &str, &str); 13] = [
    (
        "toimisto",
        "office interior with desks, daytime",
        include_str!("../../../examples/stick-studio/backgrounds/toimisto.kilsat"),
    ),
    (
        "kahvila",
        "café interior with counter and tables",
        include_str!("../../../examples/stick-studio/backgrounds/kahvila.kilsat"),
    ),
    (
        "pysakki",
        "bus stop on a street at night, outdoors",
        include_str!("../../../examples/stick-studio/backgrounds/pysakki.kilsat"),
    ),
    (
        "hautausmaa",
        "cemetery under a crescent moon, outdoors",
        include_str!("../../../examples/stick-studio/backgrounds/hautausmaa.kilsat"),
    ),    (
        "keskusta",
        "city shopping street by day: bakery, boutique and pharmacy with a traffic light; reverse looks across the road at a bus, a taxi, a bank and a cinema, outdoors",
        include_str!("../../../examples/stick-studio/backgrounds/keskusta.kilsat"),
    ),
    (
        "asuinalue",
        "residential street by day: two family houses with picket fences, a car in the driveway and an autumn maple; reverse looks across the road at apartment blocks and a playground, outdoors",
        include_str!("../../../examples/stick-studio/backgrounds/asuinalue.kilsat"),
    ),
    (
        "tori",
        "market square by day: town hall with a clock tower and two market stalls; reverse shows a department store, a passing tram and a statue, outdoors",
        include_str!("../../../examples/stick-studio/backgrounds/tori.kilsat"),
    ),
    (
        "lahikauppa",
        "corner grocery shop interior: shelves of goods and a drinks fridge; reverse shows the checkout, sliding entrance doors and shopping carts",
        include_str!("../../../examples/stick-studio/backgrounds/lahikauppa.kilsat"),
    ),
    (
        "huoltoasema",
        "petrol station at sunset: canopy, fuel pumps, a car filling up and the station shop; reverse shows a car wash and the road, outdoors",
        include_str!("../../../examples/stick-studio/backgrounds/huoltoasema.kilsat"),
    ),
    (
        "puisto",
        "city park by day: path, duck pond, bench and trees with the skyline beyond; reverse shows a playground and an ice-cream stall, outdoors",
        include_str!("../../../examples/stick-studio/backgrounds/puisto.kilsat"),
    ),
    (
        "katto",
        "city rooftop at night: water tank, antenna and the lit skyline under the moon, good for hero moments; reverse shows the stairwell door, a billboard and a bridge, outdoors",
        include_str!("../../../examples/stick-studio/backgrounds/katto.kilsat"),
    ),
    (
        "kuja",
        "back alley at night: brick walls, a lit back door, fire escape, dumpster and an alley cat; reverse looks out of the alley to a lit street with a taxi, outdoors",
        include_str!("../../../examples/stick-studio/backgrounds/kuja.kilsat"),
    ),
    (
        "metroasema",
        "metro station platform: a train waiting with open doors and a station clock; reverse shows the route map, ticket machines and the escalator up",
        include_str!("../../../examples/stick-studio/backgrounds/metroasema.kilsat"),
    ),
];

/// `(id, personality for the assistant, English voice, Finnish voice, speaking rate)`.
/// The Finnish voices are pitch variants of the one Piper speaker
/// (`motion_tts::FINNISH_VOICES`); with the rate they tell the characters apart.
const CAST: [(&str, &str, &str, &str, f64); 6] = [
    ("NOUTAJA", "calm, laconic grim reaper with dry humour; black hood and a scythe", "bm_george", "piper_harri_jykea", 0.88),
    ("VEIKKO", "ordinary commuter everyone can relate to; blue cap and shoulder bag", "am_michael", "piper_harri", 1.0),
    ("PASMA", "pompous small-time boss who turns small things into big ones; red tie and clipboard", "am_adam", "piper_harri_matala", 0.96),
    ("KIPINA", "curious inventor whose fix always needs another fix; orange beanie and goggles", "af_bella", "piper_harri_kirkas", 1.08),
    ("KIIRE", "courier who reaches everywhere except a standstill; lime vest and a parcel", "am_eric", "piper_harri_korkea", 1.18),
    ("SISU", "resourceful, calm grandmother who does not step aside even for the reaper; glasses, violet scarf and a cane", "bf_emma", "piper_harri_korkea", 0.92),
];
const FINNISH_VOICE: &str = "piper_harri";
/// Furniture artwork the compiler may place, with the source canvas geometry:
/// horizontal centre, seat top and floor (wheel bottom) in source pixels.
const FURNITURE: [(&str, &str, f64, f64, f64); 1] = [(
    "toimistotuoli",
    include_str!("../../../examples/stick-studio/props/toimistotuoli.kilsat"),
    256.0,
    280.0,
    461.0,
)];

/// Scale and move one piece of furniture artwork into a scene, behind actors.
/// Whether any layer is drawn specifically for camera `angle`.
fn drawn_for(layers: &[motion_art::Layer], angle: &str) -> bool {
    layers.iter().any(|l| l.angle.as_deref() == Some(angle))
}

/// Camera angles a set can be filmed from: front, every angle some layer is
/// drawn for, and reverse when every shared layer can be mirrored.
fn set_angles(layers: &[motion_art::Layer]) -> Vec<String> {
    let mut angles: Vec<String> = motion_scene::CAMERA_ANGLES
        .iter()
        .filter(|a| **a == "front" || drawn_for(layers, a))
        .map(|a| a.to_string())
        .collect();
    let mirrorable = layers
        .iter()
        .filter(|l| l.angle.is_none())
        .all(|l| l.mirrored_x(0.0).is_ok());
    if mirrorable && !angles.iter().any(|a| a == "reverse") {
        angles.insert(1, "reverse".into());
    }
    angles
}

/// Layers for a scene that cuts to the reverse angle: every shared layer is
/// kept for the front and a mirrored copy (about the canvas centre `axis`) is
/// added for the reverse. Layers already drawn for an angle stay as they are.
fn with_reverse(
    layers: Vec<motion_art::Layer>,
    axis: f64,
) -> Result<Vec<motion_art::Layer>, String> {
    let mut front = Vec::with_capacity(layers.len());
    let mut reverse = Vec::new();
    for mut layer in layers {
        if layer.angle.is_none() {
            let mut mirrored = layer
                .mirrored_x(axis)
                .map_err(|e| format!("layer {}: {e}", layer.id))?;
            mirrored.id = format!("{}_rev", layer.id);
            mirrored.angle = Some("reverse".into());
            reverse.push(mirrored);
            layer.angle = Some("front".into());
        }
        front.push(layer);
    }
    front.extend(reverse);
    Ok(front)
}

fn furniture_layers(
    item: &story::Furniture,
    index: usize,
) -> Result<Vec<motion_art::Layer>, String> {
    let (_, project, centre, seat, floor) = FURNITURE
        .iter()
        .find(|(id, ..)| *id == item.prop)
        .ok_or_else(|| format!("unknown furniture {}", item.prop))?;
    let scale = (item.floor - item.seat) / (floor - seat);
    if !scale.is_finite() || scale <= 0.0 {
        return Err(format!(
            "furniture {} has no room under the seat",
            item.prop
        ));
    }
    let map = |x: f64, y: f64| {
        (
            item.x + (x - centre) * scale,
            item.floor - (floor - y) * scale,
        )
    };
    let rect = |r: &motion_art::Rect| {
        let (x, y) = map(r.x, r.y);
        motion_art::Rect {
            x,
            y,
            width: r.width * scale,
            height: r.height * scale,
        }
    };
    let document = set_document(project)?;
    document
        .art
        .layers
        .into_iter()
        .map(|mut layer| {
            layer.content = match layer.content {
                motion_art::Content::Rectangle { rect: r, color } => {
                    motion_art::Content::Rectangle {
                        rect: rect(&r),
                        color,
                    }
                }
                motion_art::Content::Ellipse { rect: r, color } => motion_art::Content::Ellipse {
                    rect: rect(&r),
                    color,
                },
                motion_art::Content::Stroke {
                    points,
                    color,
                    width,
                } => motion_art::Content::Stroke {
                    points: points
                        .iter()
                        .map(|p| {
                            let (x, y) = map(p.x, p.y);
                            motion_art::Point { x, y }
                        })
                        .collect(),
                    color,
                    width: width * scale,
                },
                other => return Err(format!("furniture art {other:?} is not supported")),
            };
            let suffix = layer
                .id
                .strip_prefix(&format!("{}_", layer.scene_id))
                .unwrap_or(&layer.id)
                .to_string();
            layer.id = format!("{}_furniture{index}_{suffix}", item.scene);
            layer.scene_id = item.scene.clone();
            layer.plane = motion_art::Plane::Background;
            layer.clip = None;
            Ok(layer)
        })
        .collect()
}

/// The user-facing Finnish text for an automatic editing check.
pub(crate) fn review_text(note: &story::ReviewNote) -> String {
    use story::ReviewNote::*;
    match note {
        SpeakerOffScreen { scene, actor, at } => format!(
            "{scene} {at:.1} s: {actor} puhuu kuvan ulkopuolella. Rajaa puhujaan (on: {actor}) tai käytä two_shot-kuvaa."
        ),
        FlashCut { scene, at, seconds } => format!(
            "{scene} {at:.1} s: otos kestää vain {seconds:.2} s (välähdys). Pidennä beatia tai poista kuvanvaihto."
        ),
        LongStaticShot { scene, at, seconds } => format!(
            "{scene} {at:.1} s: liikkumaton otos {seconds:.1} s. Lisää leikkaus, reaktio tai camera: push_in."
        ),
        FastSpeech { scene, actor, at, chars_per_second } => format!(
            "{scene} {at:.1} s: {actor} puhuu nopeasti ({chars_per_second:.0} merkkiä/s). Lyhennä repliikkiä tai hidasta speed-arvoa."
        ),
        LongSilence { scene, at, seconds } => format!(
            "{scene} {at:.1} s: {seconds:.1} s hiljaisuutta repliikkien välissä. Tarkista, onko tauko tarkoituksellinen."
        ),
    }
}

pub(crate) struct Built {
    pub document: motion_project::ProjectDocument,
    pub compilation: StoryCompilation,
}

fn catalog_error(message: String) -> Vec<StoryIssue> {
    vec![StoryIssue {
        kind: IssueKind::Invalid,
        path: "catalog".into(),
        message,
        line: None,
        column: None,
    }]
}

fn set_document(project: &str) -> Result<motion_project::ProjectDocument, String> {
    motion_project::decode(project.as_bytes()).map_err(|e| e.to_string())
}

/// The open project's own characters and painted scenes, offered to the
/// story beside the stick-studio pack. Unreadable input simply offers nothing.
#[derive(Clone, Default)]
pub(crate) struct ProjectContent {
    script: Option<motion_script::Script>,
    art: Vec<motion_art::Layer>,
}
impl ProjectContent {
    pub(crate) fn new(source: &str, art: Option<&motion_art::Document>) -> Self {
        Self {
            script: motion_script::parse(source).ok(),
            art: art.map(|a| a.layers.clone()).unwrap_or_default(),
        }
    }
}

/// A catalog plus the artwork of each set.
struct Library {
    catalog: StoryCatalog,
    set_layers: BTreeMap<String, Vec<motion_art::Layer>>,
}

fn pack_library() -> Result<Library, String> {
    let starter = crate::stick_studio::load()?;
    let script = motion_script::parse(&starter.source).map_err(|e| e.to_string())?;
    let manifest: serde_json::Value = serde_json::from_str(MANIFEST).map_err(|e| e.to_string())?;
    let mut characters = BTreeMap::new();
    for (id, description, english_voice, finnish_voice, speed) in CAST {
        let character = script
            .characters
            .get(id)
            .ok_or_else(|| format!("starter project has no character {id}"))?;
        let entry = manifest["cast"]
            .as_array()
            .and_then(|cast| cast.iter().find(|c| c["id"] == id))
            .ok_or_else(|| format!("manifest has no cast entry {id}"))?;
        let mut character = character.clone();
        character.voice = None;
        characters.insert(
            id.to_string(),
            CatalogCharacter {
                name: entry["name"].as_str().unwrap_or(id).to_string(),
                description: description.into(),
                character,
                accent: entry["accent"].as_str().unwrap_or("#ffffff").to_lowercase(),
                english_voice: english_voice.into(),
                finnish_voice: finnish_voice.into(),
                speed,
            },
        );
    }
    let mut sets = BTreeMap::new();
    let mut set_layers = BTreeMap::new();
    for (id, description, project) in SETS {
        let document = set_document(project)?;
        let set_script = motion_script::parse(&document.source).map_err(|e| e.to_string())?;
        let scene = set_script
            .scenes
            .first()
            .ok_or_else(|| format!("set {id} has no scene"))?;
        sets.insert(
            id.to_string(),
            CatalogSet {
                name: set_script.project.title.clone(),
                description: description.into(),
                background_color: scene.composition.background_color.clone(),
                angles: set_angles(&document.art.layers),
            },
        );
        set_layers.insert(id.to_string(), document.art.layers);
    }
    Ok(Library {
        catalog: StoryCatalog { characters, sets },
        set_layers,
    })
}

/// A story set id for a project scene: lower-case `a-z0-9_`, prefixed with
/// `oma_` when it would hide a pack set.
fn project_set_id(scene: &str, taken: &BTreeMap<String, CatalogSet>) -> Option<String> {
    let mut id: String = scene
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if !id.starts_with(|c: char| c.is_ascii_lowercase()) {
        id.insert_str(0, "oma_");
    }
    if taken.contains_key(&id) {
        id.insert_str(0, "oma_");
    }
    (id.len() <= 40 && !taken.contains_key(&id)).then_some(id)
}

fn library(project: &ProjectContent) -> Result<Library, String> {
    let mut library = pack_library()?;
    let Some(script) = &project.script else {
        return Ok(library);
    };
    for (id, character) in &script.characters {
        let rig_ok = character.rig.is_some() && character.resolved_rig().is_ok();
        if !rig_ok || library.catalog.characters.contains_key(id) {
            continue;
        }
        let voice = character.voice.as_ref();
        let preset = |provider: &str, fallback: &str| {
            voice
                .filter(|v| v.provider == provider)
                .map_or(fallback.to_string(), |v| v.preset.clone())
        };
        let acting = character.rig.as_ref().map_or(String::new(), |rig| {
            let mut parts = Vec::new();
            if rig.visemes.is_some() {
                parts.push("talking mouth");
            }
            if rig.eyes.is_some() {
                parts.push("eyes");
            }
            if !rig.expressions.is_empty() {
                parts.push("expressions");
            }
            if parts.is_empty() {
                String::new()
            } else {
                format!(" with {}", parts.join(", "))
            }
        });
        let mut own = character.clone();
        own.voice = None;
        library.catalog.characters.insert(
            id.clone(),
            CatalogCharacter {
                name: id.clone(),
                description: format!("the user's own character from this project{acting}"),
                character: own,
                accent: script
                    .project
                    .captions
                    .as_ref()
                    .and_then(|c| c.speaker_fill.get(id).cloned())
                    .unwrap_or_else(|| "#ffffff".into()),
                english_voice: preset("kokoro", "am_michael"),
                finnish_voice: preset("piper", FINNISH_VOICE),
                speed: voice.map_or(1.0, |v| f64::from(v.speed)).clamp(0.7, 1.4),
            },
        );
    }
    for scene in &script.scenes {
        let layers: Vec<_> = project
            .art
            .iter()
            .filter(|layer| layer.scene_id == scene.id && layer.attachment.is_none())
            .cloned()
            .collect();
        if layers.is_empty() {
            continue;
        }
        let Some(id) = project_set_id(&scene.id, &library.catalog.sets) else {
            continue;
        };
        library.catalog.sets.insert(
            id.clone(),
            CatalogSet {
                name: scene.id.clone(),
                description: format!("set painted in the user's project (scene {})", scene.id),
                background_color: scene.composition.background_color.clone(),
                angles: set_angles(&layers),
            },
        );
        library.set_layers.insert(id, layers);
    }
    Ok(library)
}

#[cfg(test)]
pub(crate) fn catalog() -> Result<StoryCatalog, String> {
    Ok(pack_library()?.catalog)
}

/// Instructions to paste into Claude or ChatGPT before the story idea,
/// listing the pack and the open project's own characters and sets.
pub(crate) fn guide(project: &ProjectContent) -> Result<String, String> {
    Ok(story::authoring_guide(&library(project)?.catalog))
}

/// Compile with the pack only (tests and the bundled example).
#[cfg(test)]
pub(crate) fn build(
    text: &str,
    measured: Option<&[story::MeasuredLine]>,
) -> Result<Built, Vec<StoryIssue>> {
    build_with(text, measured, &ProjectContent::default())
}

/// Compile pasted story text and assemble a project with each scene's set artwork.
pub(crate) fn build_with(
    text: &str,
    measured: Option<&[story::MeasuredLine]>,
    project: &ProjectContent,
) -> Result<Built, Vec<StoryIssue>> {
    let library = library(project).map_err(catalog_error)?;
    let compilation = story::compile_story(text, &library.catalog, measured)?;
    let source =
        motion_script::to_source(&compilation.script).map_err(|e| catalog_error(e.to_string()))?;
    if source.len() > MAX_SOURCE_BYTES {
        return Err(vec![StoryIssue {
            kind: IssueKind::Bound,
            path: "cast".into(),
            message: format!(
                "the compiled screenplay is {} KiB; the editor limit is {} KiB — use fewer characters or scenes",
                source.len() / 1024,
                MAX_SOURCE_BYTES / 1024
            ),
            line: None,
            column: None,
        }]);
    }
    let mut document =
        motion_project::ProjectDocument::new(source).map_err(|e| catalog_error(e.to_string()))?;
    let axis = f64::from(compilation.script.project.width) / 2.0;
    // Camera angles each scene cuts to; front is always filmed.
    let angles = |scene_id: &str| -> BTreeSet<String> {
        let mut used: BTreeSet<String> = compilation
            .script
            .scenes
            .iter()
            .find(|s| s.id == scene_id)
            .map(|s| {
                s.composition
                    .camera
                    .iter()
                    .map(|k| k.angle().to_string())
                    .collect()
            })
            .unwrap_or_default();
        used.insert("front".into());
        used
    };
    let reversed = |scene_id: &str| angles(scene_id).contains("reverse");
    for (scene_id, set_id) in &compilation.scene_sets {
        let layers = library
            .set_layers
            .get(set_id.as_str())
            .ok_or_else(|| catalog_error(format!("set {set_id} has no artwork")))?;
        // A set without its own reverse drawing is mirrored for the reverse angle.
        let mirror = reversed(scene_id) && !drawn_for(layers, "reverse");
        // Only the angles this scene uses are copied: four drawn angles of a
        // set in every scene would soon exceed the 1024-layer artwork limit.
        let used = angles(scene_id);
        let mut placed = Vec::new();
        for (index, layer) in layers.iter().enumerate() {
            if layer.angle.as_ref().is_some_and(|a| !used.contains(a)) {
                continue;
            }
            let mut layer = layer.clone();
            let suffix = layer
                .id
                .strip_prefix(&format!("{}_", layer.scene_id))
                .unwrap_or(&layer.id)
                .to_string();
            layer.id = format!("{scene_id}_{index}_{suffix}");
            layer.scene_id = scene_id.clone();
            placed.push(layer);
        }
        let placed = if mirror {
            with_reverse(placed, axis).map_err(catalog_error)?
        } else {
            placed
        };
        document.art.layers.extend(placed);
    }
    for (index, item) in compilation.furniture.iter().enumerate() {
        let layers = furniture_layers(item, index).map_err(catalog_error)?;
        // Furniture is placed per story, so it is always mirrored for reverse.
        let layers = if reversed(&item.scene) {
            with_reverse(layers, axis).map_err(catalog_error)?
        } else {
            layers
        };
        document.art.layers.extend(layers);
    }
    let layers = document.art.layers.len();
    if layers > motion_art::MAX_LAYERS {
        return Err(vec![StoryIssue {
            kind: IssueKind::Bound,
            path: "scenes".into(),
            message: format!(
                "the sets of this story need {layers} artwork layers; the limit is {} — use fewer scenes, fewer camera angles per scene or simpler sets",
                motion_art::MAX_LAYERS
            ),
            line: None,
            column: None,
        }]);
    }
    // Morning, evening and night light the whole picture, cast included.
    for (scene_id, time) in &compilation.scene_times {
        let time = motion_art::TimeOfDay::parse(time)
            .ok_or_else(|| catalog_error(format!("unknown time of day {time}")))?;
        document.art = motion_art::with_time_of_day(&document.art, scene_id, time)
            .map_err(|e| catalog_error(e.to_string()))?;
    }
    document
        .validate()
        .map_err(|e| catalog_error(e.to_string()))?;
    Ok(Built {
        document,
        compilation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const EXAMPLE: &str = include_str!("../../../examples/story/kahvitauko.yaml");

    #[test]
    fn story_times_of_day_light_their_scenes() {
        assert_eq!(story::TIMES_OF_DAY, motion_art::TIMES_OF_DAY);
        let text = EXAMPLE.replacen(
            "    set: kahvila\n",
            "    set: kahvila\n    time: night\n",
            1,
        );
        assert_ne!(text, EXAMPLE, "the example names set kahvila");
        let built = build(&text, None).unwrap();
        let scene = &built.compilation.scene_times[0].0;
        assert_eq!(
            motion_art::scene_time_of_day(&built.document.art, scene),
            motion_art::TimeOfDay::Night
        );
        let second = &built.compilation.scene_times[1].0;
        assert_eq!(
            motion_art::scene_time_of_day(&built.document.art, second),
            motion_art::TimeOfDay::Day
        );
    }

    #[test]
    fn the_guide_lists_every_finnish_voice_and_the_cast_uses_them() {
        let guide = story::authoring_guide(&catalog().unwrap());
        for voice in motion_tts::FINNISH_VOICES {
            assert!(guide.contains(voice.preset), "{}", voice.preset);
        }
        let catalog = catalog().unwrap();
        for (id, entry) in &catalog.characters {
            assert!(
                motion_tts::finnish_voice(&entry.finnish_voice).is_some(),
                "{id}: {}",
                entry.finnish_voice
            );
        }
        let distinct: std::collections::BTreeSet<_> = catalog
            .characters
            .values()
            .map(|e| e.finnish_voice.as_str())
            .collect();
        assert_eq!(distinct.len(), 5);
    }

    #[test]
    fn catalog_matches_the_embedded_pack() {
        let catalog = catalog().unwrap();
        assert_eq!(catalog.characters.len(), 6);
        assert_eq!(catalog.sets.len(), 13);
        assert!(catalog.characters.values().all(|c| c
            .character
            .rig
            .as_ref()
            .is_some_and(|r| r.eyes.is_some())));
        let guide = guide(&ProjectContent::default()).unwrap();
        assert!(guide.contains("SISU") && guide.contains("hautausmaa"));
        for city in [
            "keskusta",
            "asuinalue",
            "tori",
            "lahikauppa",
            "huoltoasema",
            "puisto",
            "katto",
            "kuja",
            "metroasema",
        ] {
            assert!(catalog.sets.contains_key(city), "{city}");
            assert!(guide.contains(city), "{city} is offered to the assistant");
        }
    }

    /// Every pack set has its own reverse drawing, and twelve scenes filmed
    /// only from the front stay inside the 1024-layer artwork limit.
    #[test]
    fn every_pack_set_draws_its_reverse_and_fits_a_long_story() {
        let library = pack_library().unwrap();
        for (set, layers) in &library.set_layers {
            let front = layers
                .iter()
                .filter(|l| l.angle.as_deref() == Some("front"))
                .count();
            assert!(drawn_for(layers, "reverse"), "{set}");
            assert!(front > 20, "{set} has {front} front layers");
            assert!(
                front * motion_script::story::MAX_STORY_SCENES <= motion_art::MAX_LAYERS,
                "{set}: {front} front layers"
            );
        }
        let scenes: String = (0..12)
            .map(|i| format!("  - id: s{i}\n    set: keskusta\n    positions: {{VEIKKO: left}}\n    beats:\n      - say: VEIKKO\n        line: Hi.\n"))
            .collect();
        let text = format!(
            "kilsat_story: 1\ntitle: T\nlanguage: en\ncast:\n  VEIKKO: {{}}\nscenes:\n{scenes}"
        );
        assert!(build(&text, None).unwrap().document.art.layers.len() <= 1024);
    }

    #[test]
    fn a_story_over_the_layer_limit_is_reported_as_a_bound() {
        // Twelve scenes cutting to the reverse need both drawings of the street.
        let scenes: String = (0..12)
            .map(|i| format!("  - id: s{i}\n    set: keskusta\n    positions: {{VEIKKO: left}}\n    beats:\n      - say: VEIKKO\n        line: Hi.\n        angle: reverse\n"))
            .collect();
        let text = format!(
            "kilsat_story: 1\ntitle: T\nlanguage: en\ncast:\n  VEIKKO: {{}}\nscenes:\n{scenes}"
        );
        let issues = build(&text, None).err().unwrap();
        assert_eq!(issues[0].kind, IssueKind::Bound);
        assert!(issues[0].message.contains("1024"), "{}", issues[0].message);
    }

    #[test]
    fn example_story_becomes_a_project_with_set_artwork_per_scene() {
        let built = build(EXAMPLE, None).unwrap();
        let script = motion_script::parse(&built.document.source).unwrap();
        assert_eq!(script.scenes.len(), 2);
        for scene in &script.scenes {
            let layers = built
                .document
                .art
                .layers
                .iter()
                .filter(|l| l.scene_id == scene.id)
                .count();
            assert!(layers > 10, "scene {} has {layers} set layers", scene.id);
        }
        let mut ids: Vec<_> = built.document.art.layers.iter().map(|l| &l.id).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), count);
        // The document round-trips through the project format.
        let bytes = motion_project::encode(&built.document).unwrap();
        motion_project::decode(&bytes).unwrap();
    }

    #[test]
    fn reverse_angle_uses_the_drawn_set_mirrors_furniture_and_swaps_actor_sides() {
        let text = "kilsat_story: 1\ntitle: T\nlanguage: en\ncast:\n  VEIKKO: {}\n  SISU: {}\nscenes:\n  - id: s\n    set: kahvila\n    positions: {VEIKKO: left, SISU: right}\n    beats:\n      - do: VEIKKO\n        pose: sit\n      - say: SISU\n        line: Hello there.\n      - say: VEIKKO\n        line: Over here.\n        angle: reverse\n";
        let catalog = catalog().unwrap();
        assert!(catalog
            .sets
            .values()
            .all(|set| set.angles == ["front", "reverse", "high", "low"]));
        assert!(guide(&ProjectContent::default())
            .unwrap()
            .contains("angles: front, reverse, high, low"));
        let built = build(text, None).unwrap();
        let art = &built.document.art;
        let tagged = |angle: &str| {
            art.layers
                .iter()
                .filter(|l| l.angle.as_deref() == Some(angle))
                .count()
        };
        // The café's own reverse drawing is used; unused angles are not copied.
        assert!(art.layers.iter().any(|l| l.id.contains("kahvila_rev_door")));
        assert_eq!(tagged("high") + tagged("low"), 0);
        assert!(tagged("front") > 40 && tagged("reverse") > 30);
        assert!(art.layers.iter().all(|l| l.angle.is_some()));
        // The chair has no drawn reverse, so it is mirrored about the centre.
        let width = 1920.0;
        let chairs: Vec<_> = art
            .layers
            .iter()
            .filter(|l| l.angle.as_deref() == Some("front") && !l.id.contains("kahvila"))
            .collect();
        assert!(!chairs.is_empty());
        for layer in chairs {
            let copy = art
                .layers
                .iter()
                .find(|l| l.id == format!("{}_rev", layer.id))
                .unwrap();
            assert_eq!(copy.content, layer.mirrored_x(width / 2.0).unwrap().content);
        }
        let script = motion_script::parse(&built.document.source).unwrap();
        let line = &built.compilation.lines[1];
        let x_at = |t: f64| {
            let scene = motion_scene::evaluate(&script, 0, t).unwrap();
            let veikko = scene.actors.iter().find(|a| a.id == "VEIKKO").unwrap().x;
            (scene.camera.angle, veikko)
        };
        let (front, before) = x_at(line.at - 0.1);
        let (reverse, after) = x_at(line.at + 0.1);
        assert_eq!(front, motion_scene::CameraAngle::Front);
        assert_eq!(reverse, motion_scene::CameraAngle::Reverse);
        assert!(before < width / 2.0 && after > width / 2.0);
        assert!((before + after - width).abs() < 1e-6, "mirrored, not moved");
        // Both shots render through the normal pipeline; set
        // STORY_ANGLE_OUT=<dir> to keep the frames for a visual check.
        for (name, t) in [("front", line.at - 0.1), ("reverse", line.at + 0.1)] {
            let scene = motion_scene::evaluate(&script, 0, t).unwrap();
            let svg = motion_render::render_svg_with_art(
                &scene,
                960,
                540,
                &[],
                "s",
                art,
                &BTreeMap::new(),
            )
            .unwrap();
            if let Ok(dir) = std::env::var("STORY_ANGLE_OUT") {
                let png = motion_export::rasterize_svg_png(&svg).unwrap();
                std::fs::write(format!("{dir}/{name}.png"), png).unwrap();
            }
        }
    }

    #[test]
    fn sets_without_drawn_angles_are_mirrored_for_reverse() {
        let mut layers = pack_library().unwrap().set_layers["toimisto"].clone();
        layers.retain(|l| l.angle.as_deref() == Some("front"));
        for layer in &mut layers {
            layer.angle = None;
        }
        assert_eq!(set_angles(&layers), ["front", "reverse"]);
        let doubled = with_reverse(layers.clone(), 960.0).unwrap();
        assert_eq!(doubled.len(), 2 * layers.len());
        let mut image = layers[0].clone();
        image.content = motion_art::Content::Image {
            asset_id: "photo".into(),
            rect: motion_art::Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
        };
        layers.push(image);
        assert_eq!(
            set_angles(&layers),
            ["front"],
            "an image cannot be mirrored"
        );
    }

    #[test]
    fn a_long_story_copies_only_the_angles_it_films() {
        let scenes: String = (0..12)
            .map(|i| format!("  - id: s{i}\n    set: kahvila\n    positions: {{VEIKKO: left}}\n    beats:\n      - say: VEIKKO\n        line: Hi.\n        angle: {}\n", ["front", "high", "low"][i % 3]))
            .collect();
        let text = format!(
            "kilsat_story: 1\ntitle: T\nlanguage: en\ncast:\n  VEIKKO: {{}}\nscenes:\n{scenes}"
        );
        let built = build(&text, None).unwrap();
        assert!(built.document.art.layers.len() <= 1024);
        for scene in ["s1", "s2"] {
            let angles: BTreeSet<_> = built
                .document
                .art
                .layers
                .iter()
                .filter(|l| l.scene_id == scene)
                .filter_map(|l| l.angle.clone())
                .collect();
            assert_eq!(angles.len(), 2, "{scene}: front plus its own angle");
        }
    }

    /// Every set renders from every angle through the normal pipeline; set
    /// STORY_ANGLE_OUT=<dir> to keep the frames for a visual check.
    #[test]
    fn every_set_renders_from_every_angle() {
        for (set, _, _) in SETS {
            for angle in motion_scene::CAMERA_ANGLES {
                let text = format!("kilsat_story: 1\ntitle: T\nlanguage: en\ncast:\n  VEIKKO: {{}}\nscenes:\n  - id: s\n    set: {set}\n    positions: {{VEIKKO: center}}\n    beats:\n      - say: VEIKKO\n        line: Hello there.\n        angle: {angle}\n");
                let built = build(&text, None).unwrap();
                let script = motion_script::parse(&built.document.source).unwrap();
                let at = built.compilation.lines[0].at + 0.2;
                let scene = motion_scene::evaluate(&script, 0, at).unwrap();
                assert_eq!(scene.camera.angle.name(), angle);
                let svg = motion_render::render_svg_with_art(
                    &scene,
                    960,
                    540,
                    &[],
                    "s",
                    &built.document.art,
                    &BTreeMap::new(),
                )
                .unwrap();
                if let Ok(dir) = std::env::var("STORY_ANGLE_OUT") {
                    let png = motion_export::rasterize_svg_png(&svg).unwrap();
                    std::fs::write(format!("{dir}/{set}-{angle}.png"), png).unwrap();
                }
            }
        }
    }

    #[test]
    fn every_set_and_character_compiles() {
        for (set, _, _) in SETS {
            for chunk in CAST.chunks(3) {
                let cast: String = chunk
                    .iter()
                    .map(|(id, _, _, _, _)| format!("  {id}: {{}}\n"))
                    .collect();
                let positions: Vec<String> = chunk
                    .iter()
                    .zip(["left", "center", "right"])
                    .map(|((id, _, _, _, _), p)| format!("{id}: {p}"))
                    .collect();
                let beats: String = chunk
                    .iter()
                    .map(|(id, _, _, _, _)| format!("      - say: {id}\n        line: Hello there.\n        gesture: wave\n"))
                    .collect();
                let text = format!(
                    "kilsat_story: 1\ntitle: T\nlanguage: en\ncast:\n{cast}scenes:\n  - id: s\n    set: {set}\n    positions: {{{}}}\n    beats:\n{beats}",
                    positions.join(", ")
                );
                build(&text, None).unwrap_or_else(|e| panic!("{set}: {e:#?}"));
            }
        }
    }

    #[test]
    fn a_seated_actor_gets_a_chair_behind_them() {
        let text = "kilsat_story: 1\ntitle: T\nlanguage: en\ncast:\n  VEIKKO: {}\nscenes:\n  - id: s\n    set: kahvila\n    positions: {VEIKKO: center}\n    beats:\n      - do: VEIKKO\n        pose: sit\n";
        let built = build(text, None).unwrap();
        let chair: Vec<_> = built
            .document
            .art
            .layers
            .iter()
            .filter(|l| l.id.contains("furniture0"))
            .collect();
        assert!(chair.len() > 5);
        assert!(chair
            .iter()
            .all(|l| l.plane == motion_art::Plane::Background && l.scene_id == "s"));
    }

    #[test]
    fn errors_are_returned_not_panicked() {
        let issues = build("kilsat_story: 1\ntitle: x\n", None).err().unwrap();
        assert!(!issues.is_empty());
    }
}

#[cfg(test)]
mod project_content_tests {
    use super::*;

    #[test]
    fn own_characters_and_painted_scenes_become_story_material() {
        // A project made from a story offers its scenes as sets; the pack's
        // starter offers its six painted scenes.
        let starter = crate::stick_studio::load().unwrap();
        let mut script = motion_script::parse(&starter.source).unwrap();
        let mut own = script.characters["VEIKKO"].clone();
        own.voice = Some(motion_script::VoiceConfig {
            provider: "kokoro".into(),
            preset: "bm_lewis".into(),
            language: "en-gb".into(),
            speed: 1.1,
        });
        script.characters.insert("ONNI".into(), own);
        let source = motion_script::to_source(&script).unwrap();
        let project = ProjectContent::new(&source, Some(&starter.art));
        let library = library(&project).unwrap();
        let onni = &library.catalog.characters["ONNI"];
        assert_eq!(onni.english_voice, "bm_lewis");
        assert!(onni.description.contains("own character"));
        assert!(library.catalog.sets.contains_key("vuoron_alku"));
        assert!(!library.set_layers["vuoron_alku"].is_empty());
        let guide = story::authoring_guide(&library.catalog);
        assert!(guide.contains("ONNI") && guide.contains("vuoron_alku"));
        let text = "kilsat_story: 1\ntitle: T\nlanguage: en\ncast:\n  ONNI: {}\nscenes:\n  - id: s\n    set: vuoron_alku\n    positions: {ONNI: center}\n    beats:\n      - say: ONNI\n        line: My own set.\n";
        let built = build_with(text, None, &project).unwrap();
        assert!(built.document.art.layers.iter().any(|l| l.scene_id == "s"));
        assert!(
            build(text, None).is_err(),
            "not offered without the project"
        );
    }

    #[test]
    fn set_ids_never_hide_pack_sets() {
        let sets = catalog().unwrap().sets;
        assert_eq!(
            project_set_id("kahvila", &sets).as_deref(),
            Some("oma_kahvila")
        );
        assert_eq!(project_set_id("Scene 1", &sets).as_deref(), Some("scene_1"));
        assert_eq!(project_set_id("1st", &sets).as_deref(), Some("oma_1st"));
    }
}
