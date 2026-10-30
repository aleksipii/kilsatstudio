//! High-level story screenplay (`kilsat_story: 1`).
//!
//! A compact, strict YAML format that a person or a chat assistant can write
//! from a story idea. It names characters and sets from a caller-supplied
//! [`StoryCatalog`], places actors on named stage positions and lists beats:
//! dialogue (`say`), actions (`do`) and pauses. [`compile_story`] turns it into
//! an ordinary validated [`Script`]: one MotionScript scene per story scene,
//! shot changes as hard camera cuts, dialogue as `speak` events, listener gaze,
//! gestures, walks, facing, hand props and speaker-coloured captions.
//!
//! The compiler adds no clock, timeline or evaluator; preview and export use
//! the existing paths. Dialogue durations are text estimates unless the caller
//! supplies measured ones, and the result says which ([`StoryTiming`]).
//! Every problem is reported with a path such as `scenes[1].beats[3].gesture`,
//! all at once, so the list can be handed back to the author for correction.
use crate::{
    ActorKey, CameraKey, CaptionStyle, Character, Composition, Event, GazeKey, HeldProp, Layer,
    Project, Scene, Script, VoiceConfig, FACINGS,
};
use motion_animation::MoveEase;
use motion_rig::HandSide;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const STORY_VERSION: u32 = 1;
pub const MAX_STORY_BYTES: usize = 64 * 1024;
pub const MAX_STORY_SCENES: usize = 12;
pub const MAX_SCENE_BEATS: usize = 48;
pub const MAX_STORY_BEATS: usize = 200;
pub const MAX_STORY_CAST: usize = 6;
pub const MAX_LINE_CHARS: usize = 400;
pub const MAX_TITLE_CHARS: usize = 120;
pub const MAX_LINE_SECONDS: f64 = 30.0;
pub const MAX_HOLD_SECONDS: f64 = 10.0;
pub const MAX_SCENE_SECONDS: f64 = 180.0;
pub const MAX_STORY_SECONDS: f64 = 900.0;
/// Output format of every compiled story.
pub const STORY_FPS: u32 = 24;
pub const STORY_WIDTH: u32 = 1920;
pub const STORY_HEIGHT: u32 = 1080;

/// Named stage positions as fractions of the frame width. The `offscreen_`
/// positions are just outside the frame: start there to enter, move there to
/// exit.
pub const POSITIONS: [(&str, f64); 7] = [
    ("far_left", 0.14),
    ("left", 0.30),
    ("center", 0.50),
    ("right", 0.70),
    ("far_right", 0.86),
    ("offscreen_left", -0.12),
    ("offscreen_right", 1.12),
];
fn offscreen(place: &str) -> bool {
    place.starts_with("offscreen_")
}
pub const SHOTS: [&str; 6] = [
    "auto",
    "wide",
    "medium",
    "close_up",
    "two_shot",
    "over_shoulder",
];
pub const CAMERA_MOVES: [&str; 6] = [
    "static",
    "push_in",
    "pull_out",
    "pan_left",
    "pan_right",
    "shake",
];
pub const EMOTIONS: [&str; 5] = ["neutral", "happy", "sad", "surprised", "angry"];
pub const LANGUAGES: [&str; 3] = ["fi", "en", "en-gb"];

/// Seconds before the first beat, between beats and after the last one.
const LEAD: f64 = 0.5;
const GAP: f64 = 0.3;
const TAIL: f64 = 0.8;
const GESTURE_SECONDS: f64 = 1.2;
const REACTION_SECONDS: f64 = 1.0;
const ACTION_SECONDS: f64 = 1.0;
/// Hard cut: the outgoing framing holds until this long before the cut frame.
const CUT_EPSILON: f64 = 0.001;
/// Listeners turn their eyes to a new speaker slightly after the line starts.
const LISTENER_DELAY: f64 = 0.2;
/// Actor scale on stage; the stick-studio rigs are drawn for this size at 1080p.
const ACTOR_SCALE: f64 = 1.6;
/// Walking speeds in project pixels per second.
const GAIT_SPEEDS: [(&str, f64); 11] = [
    ("walk", 260.0),
    ("jog", 450.0),
    ("run", 700.0),
    ("stroll", 180.0),
    ("sneak", 150.0),
    ("tiptoe", 170.0),
    ("limp", 150.0),
    ("sprint", 950.0),
    ("backwards", 150.0),
    ("leap", 520.0),
    ("carry", 160.0),
];
/// Where `move_to: <CHARACTER>` stops: this far from them, as a fraction of
/// the frame width, on the side the mover comes from.
const APPROACH_DISTANCE: f64 = 0.16;
/// `move_to: <CHARACTER>` stays at least this far inside the frame edges.
const APPROACH_MARGIN: f64 = 0.06;
/// The longest leap, as a fraction of the frame width.
const MAX_LEAP: f64 = 0.35;
/// Depth planes: name, vertical offset of the hips in pixels (positive is
/// lower on screen) and size relative to `mid`.
pub const DEPTHS: [(&str, f64, f64); 3] =
    [("near", 20.0, 1.25), ("mid", 0.0, 1.0), ("far", -60.0, 0.7)];
/// Walking distance, in pixels, of one depth plane.
const DEPTH_STEP: f64 = 260.0;
/// `follow` starts this long after the leader and stops this far behind them
/// (fraction of the frame width).
const FOLLOW_DELAY: f64 = 0.4;
const FOLLOW_GAP: f64 = 0.12;
/// `look_back` stops half way and looks back for this long.
const LOOK_BACK_SECONDS: f64 = 0.9;
/// A bystander in a walker's way steps this far upstage, and back, each step
/// taking `STEP_ASIDE_SECONDS`.
const STEP_ASIDE: f64 = 45.0;
const STEP_ASIDE_SECONDS: f64 = 0.4;
/// Waypoints in one `path`.
pub const MAX_PATH: usize = 6;
/// `pace` multiplies the gait's speed within these bounds.
pub const PACE_RANGE: (f64, f64) = (0.5, 1.5);
fn depth_of(name: &str) -> Option<(f64, f64)> {
    DEPTHS
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, dy, scale)| (*dy, *scale))
}
fn depth_index(name: &str) -> usize {
    DEPTHS.iter().position(|(n, _, _)| *n == name).unwrap_or(1)
}
fn gait_speed(gait: &str) -> f64 {
    GAIT_SPEEDS
        .iter()
        .find(|(name, _)| *name == gait)
        .map_or(GAIT_SPEEDS[0].1, |(_, speed)| *speed)
}

// ---------------------------------------------------------------------------
// Source document

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Story {
    pub kilsat_story: u32,
    pub title: String,
    /// `fi`, `en` (US English) or `en-gb`.
    pub language: String,
    #[serde(default = "default_true")]
    pub captions: bool,
    /// Story character id (a catalog id) to optional settings.
    pub cast: BTreeMap<String, Option<CastMember>>,
    pub scenes: Vec<StoryScene>,
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CastMember {
    /// Speech preset: Kokoro (`am_michael`) for English, Piper (`piper_harri`) for Finnish.
    pub voice: Option<String>,
    /// Hand prop id from the stick-studio hand prop catalog.
    pub holds: Option<String>,
    pub hand: Option<HandSide>,
    /// Speaking rate, 0.7–1.4; defaults to the character's own rate.
    pub speed: Option<f64>,
    /// Animation principles: a preset (`subtle`, `cartoon`, `snappy`) or a block.
    pub principles: Option<crate::PrinciplesSpec>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoryScene {
    pub id: String,
    pub set: String,
    /// Free text for the author; not interpreted.
    #[serde(default)]
    pub summary: Option<String>,
    /// Who is on stage and where they start.
    pub positions: BTreeMap<String, String>,
    pub beats: Vec<Beat>,
}

/// One story beat. Exactly one of `say`, `do` or `pause` is present.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Beat {
    pub say: Option<String>,
    pub line: Option<String>,
    #[serde(rename = "do")]
    pub act: Option<String>,
    pub pause: Option<f64>,
    pub emotion: Option<String>,
    pub gesture: Option<String>,
    pub look: Option<String>,
    pub face: Option<String>,
    /// A stage position, or an on-stage character to walk up to.
    pub move_to: Option<String>,
    /// Waypoints walked in order without stopping: positions or characters.
    pub path: Option<Vec<String>>,
    /// Walk after the character who moved in the previous beat.
    pub follow: Option<String>,
    /// Depth plane to walk to: `near`, `mid` or `far`.
    pub depth: Option<String>,
    /// Speed multiplier for the walk, 0.5–1.5.
    pub pace: Option<f64>,
    /// Stop half way through `move_to` and look back.
    pub look_back: Option<bool>,
    pub gait: Option<String>,
    /// Pick up a hand prop, or `none` to put down what the hand holds.
    pub holds: Option<String>,
    pub hand: Option<HandSide>,
    /// `stand`, `sit`, `crouch`, `kneel` or `lie`; lasts until changed.
    pub pose: Option<String>,
    pub hold: Option<f64>,
    pub shot: Option<String>,
    /// Camera movement during this beat's shot.
    pub camera: Option<String>,
    pub on: Option<String>,
    pub reaction: Option<Reaction>,
    /// Animation principles for this beat's gesture, over the cast member's.
    pub principles: Option<crate::PrinciplesSpec>,
}

impl Beat {
    /// True when the beat moves its actor.
    pub fn travels(&self) -> bool {
        self.move_to.is_some()
            || self.path.is_some()
            || self.follow.is_some()
            || self.depth.is_some()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Reaction {
    pub by: String,
    pub emotion: Option<String>,
    pub gesture: Option<String>,
    pub hold: Option<f64>,
}

// ---------------------------------------------------------------------------
// Catalog supplied by the caller

#[derive(Debug, Clone, Default)]
pub struct StoryCatalog {
    pub characters: BTreeMap<String, CatalogCharacter>,
    pub sets: BTreeMap<String, CatalogSet>,
}

#[derive(Debug, Clone)]
pub struct CatalogCharacter {
    pub name: String,
    pub description: String,
    /// Rig and asset; `voice` and `holds` are replaced by the story.
    pub character: Character,
    /// Caption colour `#rrggbb`.
    pub accent: String,
    pub english_voice: String,
    pub finnish_voice: String,
    /// Default speaking rate (0.7–1.4), part of the character's voice.
    pub speed: f64,
}

#[derive(Debug, Clone)]
pub struct CatalogSet {
    pub name: String,
    pub description: String,
    pub background_color: String,
}

// ---------------------------------------------------------------------------
// Diagnostics

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueKind {
    /// Not readable YAML.
    Syntax,
    /// Missing, unknown or wrongly shaped field.
    Schema,
    /// Names a character, set, actor or prop that does not exist.
    Reference,
    /// A value outside the supported vocabulary.
    Unsupported,
    /// A size or time limit.
    Bound,
    /// Inconsistent or impossible content.
    Invalid,
}
impl IssueKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Syntax => "syntax",
            Self::Schema => "schema",
            Self::Reference => "reference",
            Self::Unsupported => "unsupported",
            Self::Bound => "limit",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryIssue {
    pub kind: IssueKind,
    pub path: String,
    pub message: String,
    /// One-based source position, known for syntax and schema errors.
    pub line: Option<usize>,
    pub column: Option<usize>,
}
impl fmt::Display for StoryIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.kind.label(), self.path)?;
        if let (Some(line), Some(column)) = (self.line, self.column) {
            write!(f, " (line {line}, column {column})")?;
        }
        write!(f, ": {}", self.message)
    }
}
fn issue(kind: IssueKind, path: impl Into<String>, message: impl Into<String>) -> StoryIssue {
    StoryIssue {
        kind,
        path: path.into(),
        message: message.into(),
        line: None,
        column: None,
    }
}

/// Text to hand back to the assistant that wrote the story.
pub fn issues_report(issues: &[StoryIssue]) -> String {
    let mut text = String::from(
        "Kilsat Studio did not accept the story. Fix every item below and return the \
         complete corrected story as YAML (kilsat_story: 1), nothing else:\n",
    );
    for item in issues {
        text.push_str("- ");
        text.push_str(&item.to_string());
        text.push('\n');
    }
    text
}

// ---------------------------------------------------------------------------
// Output

/// A piece of furniture placed in a scene: its bottom centre stands at
/// `(x, floor)` in project pixels and its seat reaches `seat` (a y coordinate).
#[derive(Debug, Clone, PartialEq)]
pub struct Furniture {
    pub scene: String,
    /// Catalog prop id, for example `toimistotuoli`.
    pub prop: String,
    pub x: f64,
    pub floor: f64,
    pub seat: f64,
}

/// A measurable editing problem found after compilation. These checks do not
/// judge comedy, performance or lip-sync quality; they flag what a viewer
/// would likely notice so it can be fixed before watching.
#[derive(Debug, Clone, PartialEq)]
pub enum ReviewNote {
    /// The speaker is outside the frame for part of their line.
    SpeakerOffScreen {
        scene: String,
        actor: String,
        at: f64,
    },
    /// A shot shorter than `MIN_SHOT_SECONDS` (a flash cut).
    FlashCut {
        scene: String,
        at: f64,
        seconds: f64,
    },
    /// A shot longer than `LONG_SHOT_SECONDS` without a camera move.
    LongStaticShot {
        scene: String,
        at: f64,
        seconds: f64,
    },
    /// Speech faster than `FAST_SPEECH_CHARS` characters per second.
    FastSpeech {
        scene: String,
        actor: String,
        at: f64,
        chars_per_second: f64,
    },
    /// Silence longer than `LONG_SILENCE_SECONDS` between lines.
    LongSilence {
        scene: String,
        at: f64,
        seconds: f64,
    },
}
pub const MIN_SHOT_SECONDS: f64 = 0.75;
pub const LONG_SHOT_SECONDS: f64 = 10.0;
pub const FAST_SPEECH_CHARS: f64 = 18.0;
pub const LONG_SILENCE_SECONDS: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryTiming {
    /// Dialogue lengths are estimated from the text.
    Estimated,
    /// Dialogue lengths were supplied by the caller (for example measured audio).
    Supplied,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledLine {
    pub scene: String,
    pub actor: String,
    pub text: String,
    pub emotion: Option<String>,
    /// Project-relative speech WAV when the line was measured from audio.
    pub audio: Option<String>,
    /// Scene-local start and reserved length in seconds.
    pub at: f64,
    pub duration: f64,
}

#[derive(Debug, Clone)]
pub struct StoryCompilation {
    pub script: Script,
    /// `(scene id, set id)` for each compiled scene, in order.
    pub scene_sets: Vec<(String, String)>,
    /// Furniture to add to a scene's set, such as a chair under a sitting actor.
    pub furniture: Vec<Furniture>,
    pub lines: Vec<CompiledLine>,
    pub shot_count: usize,
    pub duration: f64,
    pub timing: StoryTiming,
    /// Explicit adjustments the compiler made (for example a skipped move).
    pub notes: Vec<String>,
    /// Automatic editing checks; see [`ReviewNote`].
    pub review: Vec<ReviewNote>,
}

// ---------------------------------------------------------------------------
// Parsing

/// The story text inside a pasted reply: the first fenced code block when the
/// text contains one, otherwise the whole text. Chat replies often wrap YAML
/// in ```yaml fences with prose around it.
pub fn extract_story_text(input: &str) -> &str {
    let input = input.trim_start_matches('\u{feff}');
    let mut offset = 0;
    let mut start = None;
    for line in input.split_inclusive('\n') {
        if line.trim_start().starts_with("```") {
            match start {
                None => start = Some(offset + line.len()),
                Some(begin) => return &input[begin..offset],
            }
        }
        offset += line.len();
    }
    match start {
        Some(begin) => &input[begin..],
        None => input,
    }
}

/// Parse and validate a story against a catalog without compiling it.
pub fn parse_story(input: &str, catalog: &StoryCatalog) -> Result<Story, Vec<StoryIssue>> {
    let text = extract_story_text(input);
    if text.len() > MAX_STORY_BYTES {
        return Err(vec![issue(
            IssueKind::Bound,
            "$",
            format!("story exceeds {MAX_STORY_BYTES} bytes"),
        )]);
    }
    if text.trim().is_empty() {
        return Err(vec![issue(
            IssueKind::Schema,
            "$",
            "story is empty; paste the YAML that starts with `kilsat_story: 1`",
        )]);
    }
    let story: Story = serde_yaml::from_str(text).map_err(|error| {
        let kind = if error.to_string().contains("unknown field")
            || error.to_string().contains("missing field")
            || error.to_string().contains("invalid type")
            || error.to_string().contains("unknown variant")
        {
            IssueKind::Schema
        } else {
            IssueKind::Syntax
        };
        let location = error.location();
        vec![StoryIssue {
            kind,
            path: "$".into(),
            message: error.to_string(),
            line: location.as_ref().map(|l| l.line()),
            column: location.as_ref().map(|l| l.column()),
        }]
    })?;
    let issues = validate(&story, catalog);
    if issues.is_empty() {
        Ok(story)
    } else {
        Err(issues)
    }
}

fn valid_scene_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id.as_bytes()[0].is_ascii_lowercase()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
fn position_fraction(name: &str) -> Option<f64> {
    POSITIONS.iter().find(|(n, _)| *n == name).map(|(_, f)| *f)
}
fn list(values: &[&str]) -> String {
    values.join(", ")
}
fn english(language: &str) -> bool {
    language != "fi"
}
fn valid_voice(language: &str, preset: &str) -> bool {
    let bytes = preset.as_bytes();
    if english(language) {
        bytes.len() > 3
            && bytes.len() <= 32
            && matches!(bytes[0], b'a' | b'b')
            && matches!(bytes[1], b'f' | b'm')
            && bytes[2] == b'_'
            && bytes[3..].iter().all(u8::is_ascii_lowercase)
    } else {
        preset.starts_with("piper_")
            && preset.len() <= 64
            && bytes[6..]
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
            && preset.len() > 6
    }
}

fn validate(story: &Story, catalog: &StoryCatalog) -> Vec<StoryIssue> {
    use IssueKind::*;
    let mut out = Vec::new();
    if story.kilsat_story != STORY_VERSION {
        out.push(issue(
            Unsupported,
            "kilsat_story",
            format!("version {} is not supported; use 1", story.kilsat_story),
        ));
    }
    let title_chars = story.title.trim().chars().count();
    if title_chars == 0 || title_chars > MAX_TITLE_CHARS {
        out.push(issue(
            Invalid,
            "title",
            format!("title needs 1–{MAX_TITLE_CHARS} characters"),
        ));
    }
    if !LANGUAGES.contains(&story.language.as_str()) {
        out.push(issue(
            Unsupported,
            "language",
            format!(
                "language {:?} is not supported; use {}",
                story.language,
                list(&LANGUAGES)
            ),
        ));
    }
    let known_characters: Vec<&str> = catalog.characters.keys().map(String::as_str).collect();
    if story.cast.is_empty() || story.cast.len() > MAX_STORY_CAST {
        out.push(issue(
            Bound,
            "cast",
            format!("cast needs 1–{MAX_STORY_CAST} characters"),
        ));
    }
    let props: Vec<String> = motion_rig::stick_studio_hand_props()
        .map(|c| c.items.iter().map(|item| item.id.clone()).collect())
        .unwrap_or_default();
    for (id, member) in &story.cast {
        let path = format!("cast.{id}");
        let Some(entry) = catalog.characters.get(id) else {
            out.push(issue(
                Reference,
                &path,
                format!(
                    "unknown character {id}; available: {}",
                    list(&known_characters)
                ),
            ));
            continue;
        };
        let member = member.clone().unwrap_or_default();
        if let Some(voice) = &member.voice {
            if LANGUAGES.contains(&story.language.as_str()) && !valid_voice(&story.language, voice)
            {
                let hint = if english(&story.language) {
                    "an English Kokoro preset such as am_michael, af_heart, bf_emma or bm_george"
                } else {
                    "a Finnish Piper preset such as piper_harri"
                };
                out.push(issue(
                    Unsupported,
                    format!("{path}.voice"),
                    format!(
                        "voice {voice:?} does not fit language {}; use {hint}, or leave voice out",
                        story.language
                    ),
                ));
            }
        }
        if let Some(speed) = member.speed {
            if !speed.is_finite() || !(0.7..=1.4).contains(&speed) {
                out.push(issue(
                    Bound,
                    format!("{path}.speed"),
                    "speed must be 0.7–1.4 (1 is normal)",
                ));
            }
        }
        if let Err(error) = crate::character_principles(member.principles.as_ref(), id) {
            out.push(issue(
                Bound,
                format!("{path}.principles"),
                error.to_string(),
            ));
        }
        if member.hand.is_some() && member.holds.is_none() {
            out.push(issue(
                Schema,
                format!("{path}.hand"),
                "hand needs holds (the prop to hold)",
            ));
        }
        if let Some(item) = &member.holds {
            if !props.contains(item) {
                out.push(issue(
                    Reference,
                    format!("{path}.holds"),
                    format!("unknown hand prop {item}; available: {}", props.join(", ")),
                ));
            } else if held_character(&entry.character, item, member.hand).is_none() {
                out.push(issue(
                    Invalid,
                    format!("{path}.holds"),
                    format!(
                        "{id} cannot hold {item}{}; the hand is already busy with the character's own prop — try the other hand or leave holds out",
                        member
                            .hand
                            .map(|h| format!(" in the {h:?} hand").to_lowercase())
                            .unwrap_or_default()
                    ),
                ));
            }
        }
    }
    if story.scenes.is_empty() || story.scenes.len() > MAX_STORY_SCENES {
        out.push(issue(
            Bound,
            "scenes",
            format!("story needs 1–{MAX_STORY_SCENES} scenes"),
        ));
    }
    let total_beats: usize = story.scenes.iter().map(|s| s.beats.len()).sum();
    if total_beats > MAX_STORY_BEATS {
        out.push(issue(
            Bound,
            "scenes",
            format!("story has {total_beats} beats; the limit is {MAX_STORY_BEATS}"),
        ));
    }
    let known_sets: Vec<&str> = catalog.sets.keys().map(String::as_str).collect();
    let mut scene_ids = BTreeSet::new();
    for (index, scene) in story.scenes.iter().enumerate() {
        let path = format!("scenes[{index}]");
        if !valid_scene_id(&scene.id) {
            out.push(issue(
                Invalid,
                format!("{path}.id"),
                format!(
                    "scene id {:?} must be 1–40 characters a-z, 0-9 or _ and start with a letter",
                    scene.id
                ),
            ));
        } else if !scene_ids.insert(scene.id.as_str()) {
            out.push(issue(
                Invalid,
                format!("{path}.id"),
                format!("scene id {} is used twice", scene.id),
            ));
        }
        if !catalog.sets.contains_key(&scene.set) {
            out.push(issue(
                Reference,
                format!("{path}.set"),
                format!(
                    "unknown set {}; available: {}",
                    scene.set,
                    list(&known_sets)
                ),
            ));
        }
        if scene.positions.is_empty() {
            out.push(issue(
                Schema,
                format!("{path}.positions"),
                "positions must place at least one cast member on stage",
            ));
        }
        let mut taken = BTreeMap::new();
        for (actor, place) in &scene.positions {
            let ppath = format!("{path}.positions.{actor}");
            if !story.cast.contains_key(actor) {
                out.push(issue(
                    Reference,
                    &ppath,
                    format!("{actor} is not in cast; add it to cast or remove it here"),
                ));
            }
            if position_fraction(place).is_none() {
                out.push(issue(
                    Unsupported,
                    &ppath,
                    format!(
                        "position {place:?} is not supported; use {}",
                        POSITIONS.map(|(n, _)| n).join(", ")
                    ),
                ));
            } else if offscreen(place) {
                // Any number of actors can wait outside the frame.
            } else if let Some(other) = taken.insert(place.as_str(), actor.as_str()) {
                out.push(issue(
                    Invalid,
                    &ppath,
                    format!("{actor} and {other} cannot both start at {place}"),
                ));
            }
        }
        if scene.beats.is_empty() || scene.beats.len() > MAX_SCENE_BEATS {
            out.push(issue(
                Bound,
                format!("{path}.beats"),
                format!("a scene needs 1–{MAX_SCENE_BEATS} beats"),
            ));
        }
        let on_stage = |actor: &str| scene.positions.contains_key(actor);
        for (b, beat) in scene.beats.iter().enumerate() {
            validate_beat(
                &mut out,
                &format!("{path}.beats[{b}]"),
                beat,
                &scene.id,
                &on_stage,
                scene.positions.len(),
            );
        }
    }
    out
}

fn validate_beat(
    out: &mut Vec<StoryIssue>,
    path: &str,
    beat: &Beat,
    scene: &str,
    on_stage: &dyn Fn(&str) -> bool,
    stage_size: usize,
) {
    use IssueKind::*;
    let kinds = [beat.say.is_some(), beat.act.is_some(), beat.pause.is_some()]
        .iter()
        .filter(|present| **present)
        .count();
    if kinds != 1 {
        out.push(issue(
            Schema,
            path,
            "a beat needs exactly one of say (dialogue), do (action) or pause",
        ));
        return;
    }
    let actor_check = |out: &mut Vec<StoryIssue>, field: &str, actor: &str| {
        if !on_stage(actor) {
            out.push(issue(
                Reference,
                format!("{path}.{field}"),
                format!("{actor} is not on stage in scene {scene}; add it to positions"),
            ));
        }
    };
    let vocabulary =
        |out: &mut Vec<StoryIssue>, field: &str, value: &Option<String>, allowed: &[&str]| {
            if let Some(value) = value {
                if !allowed.contains(&value.as_str()) {
                    out.push(issue(
                        Unsupported,
                        format!("{path}.{field}"),
                        format!("{field} {value:?} is not supported; use {}", list(allowed)),
                    ));
                }
            }
        };
    let hold_check = |out: &mut Vec<StoryIssue>, field: &str, value: Option<f64>, min: f64| {
        if let Some(value) = value {
            if !value.is_finite() || value < min || value > MAX_HOLD_SECONDS {
                out.push(issue(
                    Bound,
                    format!("{path}.{field}"),
                    format!("{field} must be {min}–{MAX_HOLD_SECONDS} seconds"),
                ));
            }
        }
    };
    let positions = POSITIONS.map(|(n, _)| n);
    vocabulary(out, "emotion", &beat.emotion, &EMOTIONS);
    vocabulary(out, "gesture", &beat.gesture, &motion_animation::GESTURES);
    vocabulary(out, "face", &beat.face, &FACINGS);

    vocabulary(out, "gait", &beat.gait, &motion_animation::GAITS);
    vocabulary(out, "pose", &beat.pose, &crate::POSES);
    if let Some(item) = &beat.holds {
        let props: Vec<String> = motion_rig::stick_studio_hand_props()
            .map(|c| c.items.iter().map(|i| i.id.clone()).collect())
            .unwrap_or_default();
        if item != "none" && !props.contains(item) {
            out.push(issue(
                Reference,
                format!("{path}.holds"),
                format!(
                    "unknown hand prop {item}; use none to put down, or one of {}",
                    props.join(", ")
                ),
            ));
        }
    }
    if beat.hand.is_some() && beat.holds.is_none() {
        out.push(issue(Schema, format!("{path}.hand"), "hand needs holds"));
    }
    if beat.say.is_some() && (beat.holds.is_some() || beat.pose.is_some()) {
        out.push(issue(
            Schema,
            path,
            "holds and pose belong to a do beat; put them in a do beat before the line",
        ));
    }
    vocabulary(out, "shot", &beat.shot, &SHOTS);
    vocabulary(out, "camera", &beat.camera, &CAMERA_MOVES);
    hold_check(out, "hold", beat.hold, 0.0);
    if let Some(on) = &beat.on {
        actor_check(out, "on", on);
    }
    if let Some(pause) = beat.pause {
        hold_check(out, "pause", Some(pause), 0.2);
        let extra = beat.line.is_some()
            || beat.emotion.is_some()
            || beat.gesture.is_some()
            || beat.look.is_some()
            || beat.face.is_some()
            || beat.travels()
            || beat.pace.is_some()
            || beat.look_back.is_some()
            || beat.gait.is_some()
            || beat.holds.is_some()
            || beat.pose.is_some()
            || beat.hold.is_some()
            || beat.reaction.is_some();
        if extra {
            out.push(issue(
                Schema,
                path,
                "a pause beat takes only pause, shot, camera and on",
            ));
        }
        return;
    }
    let actor = beat
        .say
        .as_deref()
        .or(beat.act.as_deref())
        .unwrap_or_default();
    actor_check(out, if beat.say.is_some() { "say" } else { "do" }, actor);
    if let Some(look) = &beat.look {
        actor_check(out, "look", look);
        if look == actor {
            out.push(issue(
                Invalid,
                format!("{path}.look"),
                "an actor cannot look at itself",
            ));
        }
    }
    if beat.gait.is_some() && !beat.travels() {
        out.push(issue(
            Schema,
            format!("{path}.gait"),
            "gait needs move_to, path, follow or depth",
        ));
    }
    let target_check = |out: &mut Vec<StoryIssue>, field: &str, target: &str| {
        if !positions.contains(&target) && !on_stage(target) {
            out.push(issue(
                Unsupported,
                format!("{path}.{field}"),
                format!(
                    "{target:?} is not supported; use {} or a character on stage",
                    list(&positions)
                ),
            ));
        } else if target == actor {
            out.push(issue(
                Invalid,
                format!("{path}.{field}"),
                "an actor cannot walk to itself; name another character or a position",
            ));
        }
    };
    if let Some(stops) = &beat.path {
        if !(2..=MAX_PATH).contains(&stops.len()) {
            out.push(issue(
                Bound,
                format!("{path}.path"),
                format!("path lists 2–{MAX_PATH} waypoints; use move_to for one"),
            ));
        }
        for (i, stop) in stops.iter().enumerate() {
            target_check(out, &format!("path[{i}]"), stop);
        }
    }
    if let Some(leader) = &beat.follow {
        if !on_stage(leader) {
            actor_check(out, "follow", leader);
        } else if leader == actor {
            out.push(issue(
                Invalid,
                format!("{path}.follow"),
                "an actor cannot follow itself",
            ));
        }
        if beat.say.is_some() {
            out.push(issue(
                Schema,
                format!("{path}.follow"),
                "follow belongs to a do beat right after the leader's move",
            ));
        }
    }
    let ways = [
        beat.move_to.is_some(),
        beat.path.is_some(),
        beat.follow.is_some(),
    ]
    .iter()
    .filter(|w| **w)
    .count();
    if ways > 1 {
        out.push(issue(
            Schema,
            path,
            "use only one of move_to, path or follow in a beat",
        ));
    }
    if beat.follow.is_some() && beat.depth.is_some() {
        out.push(issue(
            Schema,
            format!("{path}.depth"),
            "follow keeps the leader's depth; move in depth in its own beat",
        ));
    }
    vocabulary(out, "depth", &beat.depth, &DEPTHS.map(|(name, _, _)| name));
    if let Some(pace) = beat.pace {
        let (low, high) = PACE_RANGE;
        if !pace.is_finite() || !(low..=high).contains(&pace) {
            out.push(issue(
                Bound,
                format!("{path}.pace"),
                format!("pace must be {low}–{high}"),
            ));
        }
        if !beat.travels() {
            out.push(issue(
                Schema,
                format!("{path}.pace"),
                "pace needs move_to, path, follow or depth",
            ));
        }
    }
    if beat.look_back == Some(true) {
        if beat.move_to.is_none() {
            out.push(issue(
                Schema,
                format!("{path}.look_back"),
                "look_back needs move_to",
            ));
        }
        if beat.gait.as_deref() == Some("leap") {
            out.push(issue(
                Invalid,
                format!("{path}.look_back"),
                "a leap cannot stop half way to look back",
            ));
        }
    }
    if let Some(target) = &beat.move_to {
        target_check(out, "move_to", target);
    }
    if beat.principles.is_some() && beat.gesture.is_none() {
        out.push(issue(
            Schema,
            format!("{path}.principles"),
            "beat principles shape its gesture; add a gesture or set principles on the cast member",
        ));
    }
    if matches!(beat.shot.as_deref(), Some("two_shot" | "over_shoulder")) && stage_size < 2 {
        out.push(issue(
            Invalid,
            format!("{path}.shot"),
            "two_shot and over_shoulder need a second actor on stage",
        ));
    }
    if beat.say.is_some() {
        match beat.line.as_deref().map(str::trim) {
            None | Some("") => out.push(issue(
                Schema,
                format!("{path}.line"),
                "a say beat needs a nonempty line (the exact words spoken)",
            )),
            Some(line) if line.chars().count() > MAX_LINE_CHARS => out.push(issue(
                Bound,
                format!("{path}.line"),
                format!("a line has at most {MAX_LINE_CHARS} characters; split it into several say beats"),
            )),
            Some(_) => {}
        }
        if let Some(reaction) = &beat.reaction {
            let rpath = format!("{path}.reaction");
            if !on_stage(&reaction.by) {
                out.push(issue(
                    Reference,
                    format!("{rpath}.by"),
                    format!("{:?} is not on stage in scene {scene}", reaction.by),
                ));
            } else if reaction.by == actor {
                out.push(issue(
                    Invalid,
                    format!("{rpath}.by"),
                    "the reaction must come from another actor than the speaker",
                ));
            }
            vocabulary(out, "reaction.emotion", &reaction.emotion, &EMOTIONS);
            vocabulary(
                out,
                "reaction.gesture",
                &reaction.gesture,
                &motion_animation::GESTURES,
            );
            hold_check(out, "reaction.hold", reaction.hold, 0.3);
        }
    } else {
        if beat.line.is_some() {
            out.push(issue(
                Schema,
                format!("{path}.line"),
                "line belongs to a say beat; use say: <ACTOR> for dialogue",
            ));
        }
        if beat.reaction.is_some() {
            out.push(issue(
                Schema,
                format!("{path}.reaction"),
                "reaction belongs to a say beat",
            ));
        }
    }
}

/// The catalog character with a held prop attached, or `None` when no hand is free.
fn held_character(character: &Character, item: &str, hand: Option<HandSide>) -> Option<Character> {
    let hands = match hand {
        Some(hand) => vec![hand],
        None => vec![HandSide::Left, HandSide::Right],
    };
    hands.into_iter().find_map(|hand| {
        let mut candidate = character.clone();
        candidate.holds = Some(HeldProp {
            item: item.into(),
            hand,
        });
        let free = candidate.resolved_rig().is_ok();
        free.then_some(candidate)
    })
}

// ---------------------------------------------------------------------------
// Compilation

/// Estimated speaking time for a line: 0.42 s per word or 0.068 s per
/// character, whichever is longer, and at least one second. A heuristic for
/// drafts in Finnish and English; it does not analyse pronunciation.
pub fn estimate_line_seconds(text: &str) -> f64 {
    let words = text.split_whitespace().count() as f64;
    let chars = text.chars().filter(|c| !c.is_whitespace()).count() as f64;
    (words * 0.42).max(chars * 0.068).max(1.0)
}

/// Measured speech for one `say` beat: its length and, optionally, the
/// project-relative 24 kHz mono PCM16 WAV that the speak event plays.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasuredLine {
    pub seconds: f64,
    pub audio: Option<String>,
}

/// Parse, validate and compile a story into a validated MotionScript [`Script`].
///
/// `measured`, when given, holds one entry per `say` beat in story order
/// (for example synthesized speech); otherwise lengths are estimated.
pub fn compile_story(
    input: &str,
    catalog: &StoryCatalog,
    measured: Option<&[MeasuredLine]>,
) -> Result<StoryCompilation, Vec<StoryIssue>> {
    let story = parse_story(input, catalog)?;
    let say_count = story
        .scenes
        .iter()
        .flat_map(|s| &s.beats)
        .filter(|b| b.say.is_some())
        .count();
    if let Some(lengths) = measured {
        if lengths.len() != say_count {
            return Err(vec![issue(
                IssueKind::Invalid,
                "$",
                format!(
                    "{} line lengths were supplied for {say_count} lines",
                    lengths.len()
                ),
            )]);
        }
        if let Some(index) = lengths.iter().position(|line| {
            !line.seconds.is_finite()
                || line.seconds <= 0.0
                || line.seconds > MAX_LINE_SECONDS
                || line.audio.as_deref().is_some_and(|path| {
                    path.trim().is_empty()
                        || path.len() > 4096
                        || path.starts_with('/')
                        || path.contains('\0')
                        || path.split('/').any(|part| part == "..")
                })
        }) {
            return Err(vec![issue(
                IssueKind::Bound,
                format!("line {}", index + 1),
                format!("line length must be above 0 and at most {MAX_LINE_SECONDS} seconds, with a project-relative audio path"),
            )]);
        }
    }
    let mut builder = Builder::new(&story, catalog, measured);
    let mut scenes = Vec::new();
    let mut scene_sets = Vec::new();
    let mut issues = Vec::new();
    for (index, scene) in story.scenes.iter().enumerate() {
        match builder.scene(index, scene) {
            Ok(compiled) => {
                scene_sets.push((scene.id.clone(), scene.set.clone()));
                scenes.push(compiled);
            }
            Err(error) => issues.push(error),
        }
    }
    if !issues.is_empty() {
        return Err(issues);
    }
    let duration: f64 = scenes.iter().map(|s: &Scene| s.duration).sum();
    if duration > MAX_STORY_SECONDS {
        return Err(vec![issue(
            IssueKind::Bound,
            "scenes",
            format!("story lasts {duration:.1} s; the limit is {MAX_STORY_SECONDS} s"),
        )]);
    }
    let characters = builder.characters()?;
    let captions = story.captions.then(|| CaptionStyle {
        font_size: 56.0,
        speaker_fill: story
            .cast
            .keys()
            .map(|id| (id.clone(), catalog.characters[id].accent.clone()))
            .collect(),
        ..CaptionStyle::default()
    });
    let script = Script {
        animation_clips: BTreeMap::new(),
        rig_clips: BTreeMap::new(),
        rules: Default::default(),
        pronunciations: BTreeMap::new(),
        project: Project {
            title: story.title.trim().to_string(),
            fps: STORY_FPS,
            width: STORY_WIDTH,
            height: STORY_HEIGHT,
            captions,
        },
        characters,
        scenes,
    };
    script.validate().map_err(|error| {
        vec![issue(
            IssueKind::Invalid,
            "$",
            format!("the compiled screenplay was rejected: {error}"),
        )]
    })?;
    Ok(StoryCompilation {
        shot_count: builder.shots,
        lines: builder.lines,
        notes: builder.notes,
        timing: if measured.is_some() {
            StoryTiming::Supplied
        } else {
            StoryTiming::Estimated
        },
        script,
        scene_sets,
        furniture: builder.furniture,
        review: builder.review,
        duration,
    })
}

/// Camera centre offset and zoom at `time`, interpolated linearly between
/// keys like the scene evaluator.
fn camera_x_at(keys: &[CameraKey], time: f64) -> (f64, f64) {
    let Some(first) = keys.first() else {
        return (0.0, 1.0);
    };
    let mut left = first;
    for right in &keys[1..] {
        if time <= right.at {
            let t = ((time - left.at) / (right.at - left.at)).clamp(0.0, 1.0);
            return (
                left.x + (right.x - left.x) * t,
                left.zoom + (right.zoom - left.zoom) * t,
            );
        }
        left = right;
    }
    (left.x, left.zoom)
}

fn slot(hand: HandSide) -> usize {
    match hand {
        HandSide::Left => 0,
        HandSide::Right => 1,
    }
}
/// Time given to picking up or putting down a prop, and to sitting or standing.
const PROP_SECONDS: f64 = 0.6;
const SIT_HOLD_SECONDS: f64 = 0.8;
/// Stick-studio rig geometry in rig units: hip joint to the soles, and the
/// seat surface just under the hips of a seated actor.
const FEET_BELOW_HIP: f64 = 136.0;
const SEAT_BELOW_HIP: f64 = 4.0;

/// One shot: from `at` until the next cut, with an optional camera move.
#[derive(Debug, Clone, PartialEq)]
struct Shot {
    at: f64,
    framing: Framing,
    movement: Option<String>,
}

/// Camera move ranges: push/pull zoom factor, pan sweep as a fraction of the
/// visible width each way, and a short decaying shake.
const PUSH_RANGE: f64 = 0.3;
const PAN_RANGE: f64 = 0.12;
const PAN_MIN_ZOOM: f64 = 1.2;
const SHAKE_SECONDS: f64 = 0.5;
const SHAKE_PIXELS: f64 = 14.0;

#[derive(Debug, Clone, PartialEq)]
enum Framing {
    Wide,
    Medium(String),
    CloseUp(String),
    Reaction(String),
    TwoShot(String, String),
    OverShoulder(String, String),
}

struct Builder<'a> {
    story: &'a Story,
    catalog: &'a StoryCatalog,
    measured: Option<&'a [MeasuredLine]>,
    next_line: usize,
    lines: Vec<CompiledLine>,
    notes: Vec<String>,
    shots: usize,
    furniture: Vec<Furniture>,
    review: Vec<ReviewNote>,
}

/// One move on stage: start and end time, start and end x, speed profile.
type StageMove = (f64, f64, f64, f64, MoveEase);

/// Record a move, keeping each actor's moves in time order.
fn push_move(stage: &mut Stage, actor: &str, stage_move: StageMove) {
    let moves = stage.moves.entry(actor.to_string()).or_default();
    moves.push(stage_move);
    moves.sort_by(|a, b| a.0.total_cmp(&b.0));
}

/// What a walk needs to know about the rest of the scene.
struct Blocking<'a> {
    facing: &'a BTreeMap<String, String>,
    posed: &'a BTreeMap<String, String>,
    hands: &'a BTreeMap<String, [Option<String>; 2]>,
    /// Start of the previous beat.
    previous_beat: f64,
}

/// Actor positions over one scene: a start offset plus sequential moves.
struct Stage {
    start: BTreeMap<String, f64>,
    moves: BTreeMap<String, Vec<StageMove>>,
    /// Current depth plane per actor; absent is `mid`.
    depth: BTreeMap<String, &'static str>,
    /// Size changes from depth moves: (time, size relative to `mid`).
    scale_keys: BTreeMap<String, Vec<(f64, f64)>>,
}
impl Stage {
    fn x(&self, actor: &str, t: f64) -> f64 {
        let mut x = self.start[actor];
        for &(t0, t1, x0, x1, ease) in self.moves.get(actor).into_iter().flatten() {
            if t >= t1 {
                x = x1;
            } else if t > t0 {
                return x0 + (x1 - x0) * ease.apply((t - t0) / (t1 - t0));
            } else {
                break;
            }
        }
        x
    }
    fn on_screen(&self, actor: &str, t: f64) -> bool {
        (0.0..=f64::from(STORY_WIDTH)).contains(&self.x(actor, t))
    }
    fn final_x(&self, actor: &str) -> f64 {
        self.x(actor, f64::INFINITY)
    }
    /// The nearest other actor inside the frame at time `t`.
    fn partner(&self, actor: &str, t: f64) -> Option<String> {
        let x = self.x(actor, t);
        self.start
            .keys()
            .filter(|other| other.as_str() != actor && self.on_screen(other, t))
            .min_by(|a, b| {
                (self.x(a, t) - x)
                    .abs()
                    .total_cmp(&(self.x(b, t) - x).abs())
                    .then(a.cmp(b))
            })
            .cloned()
    }
}

fn frame_ceil(t: f64) -> f64 {
    let fps = f64::from(STORY_FPS);
    (t * fps - 1e-6).ceil().max(0.0) / fps
}

impl<'a> Builder<'a> {
    fn new(
        story: &'a Story,
        catalog: &'a StoryCatalog,
        measured: Option<&'a [MeasuredLine]>,
    ) -> Self {
        Self {
            story,
            catalog,
            measured,
            next_line: 0,
            lines: Vec::new(),
            notes: Vec::new(),
            shots: 0,
            furniture: Vec::new(),
            review: Vec::new(),
        }
    }

    fn characters(&self) -> Result<BTreeMap<String, Character>, Vec<StoryIssue>> {
        let (provider, language) = match self.story.language.as_str() {
            "fi" => ("piper", "fi-fi"),
            "en-gb" => ("kokoro", "en-gb"),
            _ => ("kokoro", "en-us"),
        };
        let mut characters = BTreeMap::new();
        for (id, member) in &self.story.cast {
            let entry = &self.catalog.characters[id];
            let member = member.clone().unwrap_or_default();
            let mut character = match &member.holds {
                Some(item) => {
                    held_character(&entry.character, item, member.hand).ok_or_else(|| {
                        vec![issue(
                            IssueKind::Invalid,
                            format!("cast.{id}.holds"),
                            "no free hand",
                        )]
                    })?
                }
                None => entry.character.clone(),
            };
            let preset = member.voice.clone().unwrap_or_else(|| {
                if provider == "piper" {
                    entry.finnish_voice.clone()
                } else {
                    entry.english_voice.clone()
                }
            });
            if member.principles.is_some() {
                character.principles = member.principles.clone();
            }
            character.voice = Some(VoiceConfig {
                provider: provider.into(),
                preset,
                language: language.into(),
                speed: member.speed.unwrap_or(entry.speed).clamp(0.7, 1.4) as f32,
            });
            characters.insert(id.clone(), character);
        }
        Ok(characters)
    }

    fn catalog_held(&self, actor: &str) -> Option<HeldProp> {
        let member = self.story.cast.get(actor)?.clone().unwrap_or_default();
        let item = member.holds?;
        let character = held_character(
            &self.catalog.characters[actor].character,
            &item,
            member.hand,
        )?;
        character.holds
    }

    /// The first empty hand that can take a catalog prop (not busy with the
    /// character's own prop), preferring the left so gestures keep the right.
    fn free_hand(&self, actor: &str, state: &[Option<String>; 2]) -> Option<HandSide> {
        let mut rig = self.catalog.characters[actor].character.rig.clone()?;
        if let Some(held) = self.catalog_held(actor) {
            crate::held_rig(&mut rig, held.hand, Some(&held.item)).ok()?;
        }
        [HandSide::Left, HandSide::Right].into_iter().find(|hand| {
            state[slot(*hand)].is_none()
                && crate::held_rig(&mut rig.clone(), *hand, Some("kahvikuppi")).is_ok()
        })
    }

    /// An office chair under a seated actor, scaled so its seat meets the
    /// lowered hips (profile sits lower than the front view).
    fn place_chair(&mut self, scene: &str, x: f64, profile: bool) {
        let h = f64::from(STORY_HEIGHT);
        let root_y = h * 0.7;
        let drop = if profile { 60.0 } else { 22.0 };
        let furniture = Furniture {
            scene: scene.to_string(),
            prop: "toimistotuoli".into(),
            x,
            floor: root_y + FEET_BELOW_HIP * ACTOR_SCALE,
            seat: root_y + (drop + SEAT_BELOW_HIP) * ACTOR_SCALE,
        };
        let duplicate = self.furniture.iter().any(|f| {
            f.scene == furniture.scene
                && (f.x - furniture.x).abs() < 1.0
                && f.seat == furniture.seat
        });
        if !duplicate {
            self.furniture.push(furniture);
        }
    }

    fn review_scene(
        &mut self,
        scene: &str,
        cuts: &[Shot],
        camera: &[CameraKey],
        stage: &Stage,
        duration: f64,
    ) {
        let w = f64::from(STORY_WIDTH);
        for (index, shot) in cuts.iter().enumerate() {
            let end = cuts.get(index + 1).map_or(duration, |next| next.at);
            let seconds = end - shot.at;
            let last = index + 1 == cuts.len();
            if seconds < MIN_SHOT_SECONDS && !last {
                self.review.push(ReviewNote::FlashCut {
                    scene: scene.into(),
                    at: shot.at,
                    seconds,
                });
            }
            let moving_subject = stage
                .moves
                .values()
                .flatten()
                .any(|&(t0, t1, ..)| t0 < end && t1 > shot.at);
            if seconds > LONG_SHOT_SECONDS && shot.movement.is_none() && !moving_subject {
                self.review.push(ReviewNote::LongStaticShot {
                    scene: scene.into(),
                    at: shot.at,
                    seconds,
                });
            }
        }
        let lines: Vec<_> = self
            .lines
            .iter()
            .filter(|l| l.scene == scene)
            .cloned()
            .collect();
        for line in &lines {
            // Sample the line at a quarter, half and three quarters.
            let hidden = [0.25, 0.5, 0.75].iter().any(|f| {
                let t = line.at + line.duration * f;
                let (cx, zoom) = camera_x_at(camera, t);
                let half = w / (2.0 * zoom);
                let x = stage.x(&line.actor, t);
                (x - (cx + w / 2.0)).abs() > half * 0.95
            });
            if hidden {
                self.review.push(ReviewNote::SpeakerOffScreen {
                    scene: scene.into(),
                    actor: line.actor.clone(),
                    at: line.at,
                });
            }
            let rate =
                line.text.chars().filter(|c| !c.is_whitespace()).count() as f64 / line.duration;
            if rate > FAST_SPEECH_CHARS {
                self.review.push(ReviewNote::FastSpeech {
                    scene: scene.into(),
                    actor: line.actor.clone(),
                    at: line.at,
                    chars_per_second: rate,
                });
            }
        }
        for pair in lines.windows(2) {
            let gap = pair[1].at - (pair[0].at + pair[0].duration);
            if gap > LONG_SILENCE_SECONDS {
                self.review.push(ReviewNote::LongSilence {
                    scene: scene.into(),
                    at: pair[0].at + pair[0].duration,
                    seconds: gap,
                });
            }
        }
    }

    fn has_eyes(&self, actor: &str) -> bool {
        self.catalog.characters[actor]
            .character
            .rig
            .as_ref()
            .is_some_and(|rig| rig.eyes.is_some())
    }

    fn scene(&mut self, index: usize, scene: &StoryScene) -> Result<Scene, StoryIssue> {
        let w = f64::from(STORY_WIDTH);
        let mut stage = Stage {
            start: scene
                .positions
                .iter()
                .map(|(actor, place)| (actor.clone(), position_fraction(place).unwrap_or(0.5) * w))
                .collect(),
            moves: BTreeMap::new(),
            depth: BTreeMap::new(),
            scale_keys: BTreeMap::new(),
        };
        let mut events: Vec<Event> = Vec::new();
        let mut gaze: BTreeMap<String, Vec<GazeKey>> = BTreeMap::new();
        let mut cuts: Vec<Shot> = Vec::new();
        let mut content_end: f64 = 0.0;
        let mut facing: BTreeMap<String, String> = BTreeMap::new();
        // Actors in a pose other than standing, with that pose.
        let mut posed: BTreeMap<String, String> = BTreeMap::new();
        // What each hand holds; every scene starts from the cast's `holds`.
        let mut hands: BTreeMap<String, [Option<String>; 2]> = BTreeMap::new();
        for actor in stage.start.keys() {
            let mut state = [None, None];
            if let Some(held) = &self.catalog_held(actor) {
                state[slot(held.hand)] = Some(held.item.clone());
            }
            hands.insert(actor.clone(), state);
        }

        let eyes: BTreeSet<String> = stage
            .start
            .keys()
            .filter(|actor| self.has_eyes(actor))
            .cloned()
            .collect();
        let look =
            |gaze: &mut BTreeMap<String, Vec<GazeKey>>, actor: &str, t: f64, target: &str| {
                if !eyes.contains(actor) {
                    return;
                }
                let keys = gaze.entry(actor.to_string()).or_default();
                let key = GazeKey {
                    at: t,
                    x: None,
                    y: None,
                    target: Some(target.to_string()),
                };
                match keys.last_mut() {
                    Some(last) if last.at >= t - 1e-9 => last.target = key.target,
                    Some(last) if last.target == key.target => {}
                    _ => keys.push(key),
                }
            };
        // Everyone starts by looking at their nearest neighbour.
        for actor in stage.start.keys() {
            if let Some(partner) = stage.partner(actor, 0.0) {
                look(&mut gaze, actor, 0.0, &partner);
            }
        }
        // A camera move always starts its own shot; otherwise an unchanged
        // framing continues without a jump cut.
        let cut = |cuts: &mut Vec<Shot>, at: f64, framing: Framing, movement: Option<&String>| {
            let shot = Shot {
                at,
                framing,
                movement: movement.cloned(),
            };
            match cuts.last_mut() {
                Some(last) if (last.at - at).abs() < 1e-9 => *last = shot,
                Some(last)
                    if last.framing == shot.framing
                        && last.movement.is_none()
                        && shot.movement.is_none() => {}
                _ => cuts.push(shot),
            }
        };
        let last_framing =
            |cuts: &Vec<Shot>| cuts.last().map_or(Framing::Wide, |s| s.framing.clone());
        let gesture_seconds =
            |within: Option<f64>| within.map_or(GESTURE_SECONDS, |d: f64| d.clamp(0.8, 1.6));

        let mut t = LEAD;
        let mut previous_speaker: Option<String> = None;
        // Start of the previous beat; `follow` looks for the leader's move there.
        let mut beat_start = 0.0;
        for (b, beat) in scene.beats.iter().enumerate() {
            t = frame_ceil(t);
            let previous_beat = std::mem::replace(&mut beat_start, t);
            let first = b == 0;
            let explicit = beat.shot.as_deref().filter(|s| *s != "auto");
            if let Some(pause) = beat.pause {
                if explicit.is_some() || first || beat.camera.is_some() {
                    let subject = beat
                        .on
                        .clone()
                        .or_else(|| stage.start.keys().next().cloned())
                        .unwrap_or_default();
                    let framing = match explicit {
                        Some(shot) => self.framing(shot, &subject, None, &stage, t),
                        None if first => Framing::Wide,
                        None => last_framing(&cuts),
                    };
                    cut(
                        &mut cuts,
                        if first { 0.0 } else { t },
                        framing,
                        beat.camera.as_ref(),
                    );
                }
                t += pause;
                content_end = content_end.max(t);
                t += GAP;
                continue;
            }
            let actor = beat.say.clone().or(beat.act.clone()).unwrap_or_default();
            let partner = beat
                .look
                .clone()
                .or_else(|| {
                    previous_speaker
                        .clone()
                        .filter(|p| *p != actor && stage.start.contains_key(p))
                })
                .or_else(|| stage.partner(&actor, t));
            let subject = beat.on.clone().unwrap_or_else(|| actor.clone());
            if let Some(face) = &beat.face {
                events.push(Event::Face {
                    at: t,
                    actor: actor.clone(),
                    value: face.clone(),
                });
                facing.insert(actor.clone(), face.clone());
            }
            if let Some(target) = &beat.look {
                look(&mut gaze, &actor, t, target);
            }
            let end;
            if beat.say.is_some() {
                let text = beat.line.clone().unwrap_or_default().trim().to_string();
                let (seconds, audio) = match self.measured {
                    Some(lines) => (
                        lines[self.next_line].seconds,
                        lines[self.next_line].audio.clone(),
                    ),
                    None => (estimate_line_seconds(&text), None),
                };
                self.next_line += 1;
                events.push(Event::Speak {
                    at: t,
                    actor: actor.clone(),
                    text: text.clone(),
                    emotion: beat.emotion.clone(),
                    audio: audio.clone(),
                });
                self.lines.push(CompiledLine {
                    scene: scene.id.clone(),
                    actor: actor.clone(),
                    text,
                    emotion: beat.emotion.clone(),
                    audio,
                    at: t,
                    duration: seconds,
                });
                // Walk and talk: the move starts with the line.
                let mut walk_end = t;
                if beat.travels() {
                    self.must_stand(index, b, &actor, &posed)?;
                    if let Some(duration) = self.walk(
                        (index, b),
                        &actor,
                        beat,
                        t,
                        &mut stage,
                        &mut events,
                        Blocking {
                            facing: &facing,
                            posed: &posed,
                            hands: &hands,
                            previous_beat,
                        },
                    )? {
                        walk_end = t + duration;
                        content_end = content_end.max(walk_end);
                    }
                }
                if beat.look.is_none() {
                    if let Some(partner) = &partner {
                        look(&mut gaze, &actor, t, partner);
                    }
                }
                for listener in stage.start.keys().filter(|l| **l != actor) {
                    look(&mut gaze, listener, t + LISTENER_DELAY, &actor);
                }
                if let Some(gesture) = &beat.gesture {
                    let start = frame_ceil(t + 0.15);
                    let duration = gesture_seconds(Some(seconds - 0.15));
                    events.push(Event::Gesture {
                        at: start,
                        actor: actor.clone(),
                        value: gesture.clone(),
                        duration,
                        principles: beat.principles.clone(),
                    });
                    content_end = content_end.max(start + duration);
                }
                let shot = explicit.unwrap_or(if first {
                    "wide"
                } else if previous_speaker.as_deref() == Some(actor.as_str()) && beat.on.is_none() {
                    "keep"
                } else if matches!(beat.emotion.as_deref(), Some("angry" | "surprised" | "sad")) {
                    "close_up"
                } else {
                    "medium"
                });
                if shot != "keep" {
                    let framing = self.framing(shot, &subject, partner.as_deref(), &stage, t);
                    cut(
                        &mut cuts,
                        if first { 0.0 } else { t },
                        framing,
                        beat.camera.as_ref(),
                    );
                } else if beat.camera.is_some() {
                    let framing = last_framing(&cuts);
                    cut(&mut cuts, t, framing, beat.camera.as_ref());
                }
                let line_end = t + seconds;
                let spoken_end = if let Some(reaction) = &beat.reaction {
                    let at = frame_ceil(line_end);
                    let hold = reaction.hold.unwrap_or(REACTION_SECONDS);
                    if let Some(emotion) = &reaction.emotion {
                        events.push(Event::Emotion {
                            at,
                            actor: reaction.by.clone(),
                            value: emotion.clone(),
                        });
                    }
                    if let Some(gesture) = &reaction.gesture {
                        let duration = gesture_seconds(Some(hold));
                        events.push(Event::Gesture {
                            at,
                            actor: reaction.by.clone(),
                            value: gesture.clone(),
                            duration,
                            principles: None,
                        });
                        content_end = content_end.max(at + duration);
                    }
                    look(&mut gaze, &reaction.by, at, &actor);
                    cut(&mut cuts, at, Framing::Reaction(reaction.by.clone()), None);
                    at + hold
                } else {
                    line_end + beat.hold.unwrap_or(0.0)
                };
                end = spoken_end.max(walk_end);
                previous_speaker = Some(actor.clone());
            } else {
                // Action beat.
                if let Some(emotion) = &beat.emotion {
                    events.push(Event::Emotion {
                        at: t,
                        actor: actor.clone(),
                        value: emotion.clone(),
                    });
                }
                let mut length: f64 = 0.0;
                if let Some(gesture) = &beat.gesture {
                    events.push(Event::Gesture {
                        at: t,
                        actor: actor.clone(),
                        value: gesture.clone(),
                        duration: GESTURE_SECONDS,
                        principles: beat.principles.clone(),
                    });
                    length = length.max(GESTURE_SECONDS);
                }
                let mut moved = false;
                if beat.travels() {
                    // Standing up in the same beat happens before the walk.
                    let mut start = t;
                    if beat.pose.as_deref() == Some("stand") && posed.contains_key(&actor) {
                        events.push(Event::Pose {
                            at: t,
                            actor: actor.clone(),
                            value: "stand".into(),
                        });
                        posed.remove(&actor);
                        start = frame_ceil(t + motion_animation::SIT_SECONDS);
                    }
                    self.must_stand(index, b, &actor, &posed)?;
                    if let Some(duration) = self.walk(
                        (index, b),
                        &actor,
                        beat,
                        start,
                        &mut stage,
                        &mut events,
                        Blocking {
                            facing: &facing,
                            posed: &posed,
                            hands: &hands,
                            previous_beat,
                        },
                    )? {
                        length = length.max(start - t + duration);
                        moved = true;
                        if let Some(target) = beat
                            .move_to
                            .as_ref()
                            .or(beat.follow.as_ref())
                            .or(beat.path.as_ref().and_then(|p| p.last()))
                            .filter(|m| stage.start.contains_key(*m))
                        {
                            if beat.look.is_none() {
                                look(&mut gaze, &actor, t, target);
                            }
                        }
                    }
                }
                // Props and poses change once the actor has arrived.
                let arrive = frame_ceil(t + if moved { length } else { 0.0 });
                if let Some(item) = &beat.holds {
                    let state = hands.get_mut(&actor).expect("on-stage actor");
                    let put_down = item == "none";
                    let hand = match beat.hand {
                        Some(hand) => hand,
                        None if put_down => {
                            if state[1].is_some() {
                                HandSide::Right
                            } else {
                                HandSide::Left
                            }
                        }
                        None => self
                            .free_hand(&actor, state)
                            .ok_or_else(|| {
                                issue(
                                    IssueKind::Invalid,
                                    format!("scenes[{index}].beats[{b}].holds"),
                                    format!("{actor} has no free hand for {item}; put something down first (holds: none)"),
                                )
                            })?,
                    };
                    if put_down && state[slot(hand)].is_none() {
                        self.notes.push(format!(
                            "scenes[{index}].beats[{b}]: {actor} holds nothing to put down"
                        ));
                    } else {
                        let item = (!put_down).then(|| item.clone());
                        state[slot(hand)] = item.clone();
                        events.push(Event::Hold {
                            at: arrive,
                            actor: actor.clone(),
                            hand,
                            item,
                        });
                        length = length.max(arrive - t + PROP_SECONDS);
                    }
                }
                if let Some(pose) = &beat.pose {
                    let current = posed.get(&actor).map_or("stand", String::as_str);
                    if pose != current {
                        events.push(Event::Pose {
                            at: arrive,
                            actor: actor.clone(),
                            value: pose.clone(),
                        });
                        if pose == "sit" {
                            let profile = matches!(
                                beat.face
                                    .as_deref()
                                    .or(facing.get(&actor).map(String::as_str)),
                                Some("left" | "right")
                            );
                            self.place_chair(&scene.id, stage.final_x(&actor), profile);
                        }
                        if pose == "stand" {
                            posed.remove(&actor);
                        } else {
                            posed.insert(actor.clone(), pose.clone());
                        }
                        let settle = motion_animation::BodyPose::parse(pose)
                            .map_or(SIT_HOLD_SECONDS, |p| {
                                p.seconds() + SIT_HOLD_SECONDS - motion_animation::SIT_SECONDS
                            });
                        length = length.max(arrive - t + settle);
                    }
                }
                if length == 0.0 {
                    // Emotion, facing or a look alone: give the acting a beat to read.
                    length = ACTION_SECONDS;
                }
                content_end = content_end.max(t + length);
                length += beat.hold.unwrap_or(0.0);
                let shot = explicit.unwrap_or(if first || moved { "wide" } else { "medium" });
                let framing = self.framing(shot, &subject, partner.as_deref(), &stage, t);
                cut(
                    &mut cuts,
                    if first { 0.0 } else { t },
                    framing,
                    beat.camera.as_ref(),
                );
                end = t + length;
            }
            content_end = content_end.max(end);
            t = end + GAP;
        }
        let duration = frame_ceil(content_end + TAIL);
        if duration > MAX_SCENE_SECONDS {
            return Err(issue(
                IssueKind::Bound,
                format!("scenes[{index}]"),
                format!(
                    "scene {} lasts {duration:.1} s; the limit is {MAX_SCENE_SECONDS} s — split it into several scenes",
                    scene.id
                ),
            ));
        }
        if cuts.first().is_none_or(|shot| shot.at > 0.0) {
            cuts.insert(
                0,
                Shot {
                    at: 0.0,
                    framing: Framing::Wide,
                    movement: None,
                },
            );
        }
        self.shots += cuts.len();
        let camera = self.camera(&cuts, &stage, duration);
        self.review_scene(&scene.id, &cuts, &camera, &stage, duration);
        events.sort_by(|a, b| a.actor_time().0.total_cmp(&b.actor_time().0));
        let cast_order: Vec<&String> = self.story.cast.keys().collect();
        let composition = Composition {
            background_color: self.catalog.sets[&scene.set].background_color.clone(),
            camera,
            // One layer for the whole cast, painted by depth: whoever stands
            // nearer the camera (lower on screen) is in front, so depth moves
            // and stepping aside overlap correctly. Ties keep cast (id) order.
            layers: cast_order
                .iter()
                .map(|id| {
                    (
                        (*id).clone(),
                        Layer {
                            z: 0,
                            visible: stage.start.contains_key(*id),
                        },
                    )
                })
                .collect(),
            depth_sort: true,
            actor_tracks: stage
                .start
                .iter()
                .map(|(actor, x)| {
                    let key = |at: f64, size: f64| ActorKey {
                        at,
                        x: x - w / 2.0,
                        scale_x: ACTOR_SCALE * size,
                        scale_y: ACTOR_SCALE * size,
                        ..ActorKey::default()
                    };
                    let mut keys = vec![key(0.0, 1.0)];
                    for &(at, size) in stage.scale_keys.get(actor).into_iter().flatten() {
                        match keys.last_mut() {
                            Some(last) if (last.at - at).abs() < 1e-9 => *last = key(at, size),
                            _ => keys.push(key(at, size)),
                        }
                    }
                    (actor.clone(), keys)
                })
                .collect(),
            gaze_tracks: gaze,
            ..Composition::default()
        };
        Ok(Scene {
            composition,
            background: Some(scene.set.clone()),
            id: scene.id.clone(),
            duration,
            events,
        })
    }

    /// Moving needs a standing actor.
    fn must_stand(
        &self,
        index: usize,
        b: usize,
        actor: &str,
        posed: &BTreeMap<String, String>,
    ) -> Result<(), StoryIssue> {
        match posed.get(actor) {
            Some(pose) => Err(issue(
                IssueKind::Invalid,
                format!("scenes[{index}].beats[{b}].move_to"),
                format!("{actor} is in pose {pose}; add pose: stand to this beat or a do beat before moving"),
            )),
            None => Ok(()),
        }
    }

    /// Compile a beat's travel for `actor` starting at `t`: `move_to`,
    /// `path`, `depth`, `look_back` and `pace`, or `follow`. Emits the moves,
    /// profile facing during them, size keys for depth, and step-aside moves
    /// for bystanders in the way. Returns the seconds from `t` until the
    /// travel (and any step aside) is over, or `None` when the actor is
    /// already there (with a note).
    #[allow(clippy::too_many_arguments)]
    fn walk(
        &mut self,
        (index, b): (usize, usize),
        actor: &str,
        beat: &Beat,
        t: f64,
        stage: &mut Stage,
        events: &mut Vec<Event>,
        blocking: Blocking,
    ) -> Result<Option<f64>, StoryIssue> {
        let path = format!("scenes[{index}].beats[{b}]");
        let invalid = |field: &str, message: String| {
            issue(IssueKind::Invalid, format!("{path}.{field}"), message)
        };
        let (w, h) = (f64::from(STORY_WIDTH), f64::from(STORY_HEIGHT));
        let root_y = h * 0.7;
        // Both hands full: a slow, hunched carry unless a gait is given.
        let loaded = blocking
            .hands
            .get(actor)
            .is_some_and(|hands| hands.iter().all(Option::is_some));
        let default_gait = if loaded { "carry" } else { "walk" };
        let pace = beat.pace.unwrap_or(1.0);
        if let Some(leader) = &beat.follow {
            return self.follow(
                &path,
                actor,
                leader,
                beat,
                t,
                stage,
                events,
                &blocking,
                default_gait,
            );
        }
        let gait = beat.gait.clone().unwrap_or_else(|| default_gait.into());
        let speed = gait_speed(&gait) * pace;
        let from = stage.final_x(actor);
        // Resolve the stops in order; a character stop is relative to where
        // the walker arrives from.
        let mut stops = Vec::new();
        let mut x = from;
        for place in beat.path.iter().flatten().chain(beat.move_to.iter()) {
            let to = match position_fraction(place) {
                Some(fraction) => fraction * w,
                None => {
                    let target = stage.x(place, t);
                    let side = if x <= target { -1.0 } else { 1.0 };
                    (target + side * APPROACH_DISTANCE * w)
                        .clamp(APPROACH_MARGIN * w, (1.0 - APPROACH_MARGIN) * w)
                }
            };
            if (to - x).abs() >= 1.0 {
                stops.push(to);
                x = to;
            }
        }
        let look_back = beat.look_back == Some(true) && stops.len() == 1;
        if look_back {
            stops.insert(0, (from + stops[0]) / 2.0);
        }
        let depth_from = stage.depth.get(actor).copied().unwrap_or("mid");
        let depth_to = beat
            .depth
            .as_deref()
            .and_then(|d| DEPTHS.iter().find(|(n, _, _)| *n == d))
            .map_or(depth_from, |(n, _, _)| *n);
        let (dy_from, size_from) = depth_of(depth_from).unwrap_or((0.0, 1.0));
        let (dy_to, size_to) = depth_of(depth_to).unwrap_or((0.0, 1.0));
        let depth_px = DEPTH_STEP * depth_index(depth_to).abs_diff(depth_index(depth_from)) as f64;
        if stops.is_empty() && depth_px > 0.0 {
            // Straight toward or away from the camera.
            stops.push(from);
        }
        if stops.is_empty() {
            let place = beat.move_to.as_deref().unwrap_or("there");
            self.notes.push(format!(
                "{path}: {actor} is already at {place}; the move was skipped"
            ));
            return Ok(None);
        }
        let mut legs = Vec::new();
        let mut x0 = from;
        for &x1 in &stops {
            if gait == "leap" && (x1 - x0).abs() > MAX_LEAP * w {
                return Err(invalid(
                    "gait",
                    format!(
                        "a leap covers at most one neighbouring position; {actor} would leap {:.0} % of the frame — walk closer first",
                        100.0 * (x1 - x0).abs() / w
                    ),
                ));
            }
            legs.push((x0, x1));
            x0 = x1;
        }
        let travel_x: f64 = legs.iter().map(|(a, c)| (c - a).abs()).sum();
        let before = blocking
            .facing
            .get(actor)
            .cloned()
            .unwrap_or_else(|| "front".into());
        let offset = stage.start[actor] - w / 2.0;
        let mut at = t;
        let mut covered = 0.0;
        let mut busy_until = t;
        let count = legs.len();
        for (i, &(x0, x1)) in legs.iter().enumerate() {
            let dx = x1 - x0;
            let share = if travel_x > 0.0 {
                dx.abs() / travel_x
            } else {
                1.0
            };
            covered += share;
            let distance = dx.hypot(depth_px * share);
            let duration = frame_ceil((distance / speed).max(0.6));
            // One continuous journey speeds up once and slows down once;
            // a look back stops half way. A leap keeps its momentum.
            let ease = if gait == "leap" {
                MoveEase::Linear
            } else if count == 1 || look_back {
                MoveEase::InOut
            } else if i == 0 {
                MoveEase::In
            } else if i + 1 == count {
                MoveEase::Out
            } else {
                MoveEase::Linear
            };
            let y = root_y + dy_from + (dy_to - dy_from) * covered.min(1.0);
            events.push(Event::Move {
                at,
                actor: actor.to_string(),
                x: x1 - offset,
                y,
                duration,
                ease: (ease != MoveEase::Linear).then(|| ease.name().to_string()),
                pace: (pace != 1.0).then_some(pace),
                gait: Some(gait.clone()),
            });
            if beat.face.is_none() {
                let side = if dx.abs() < 1.0 {
                    // Nearer shows the face; farther shows the back.
                    if dy_to > dy_from {
                        "front"
                    } else {
                        "back"
                    }
                } else if (dx > 0.0) != (gait == "backwards") {
                    "right"
                } else {
                    "left"
                };
                events.push(Event::Face {
                    at,
                    actor: actor.to_string(),
                    value: side.into(),
                });
                if look_back && i == 0 {
                    // Stop, look over the shoulder, then carry on (the next
                    // leg turns back to the travel direction).
                    let back = if side == "right" { "left" } else { "right" };
                    events.push(Event::Face {
                        at: at + duration,
                        actor: actor.to_string(),
                        value: back.into(),
                    });
                }
            }
            if dx.abs() >= 1.0 && gait != "leap" {
                busy_until = busy_until.max(self.clear_the_way(
                    &path,
                    actor,
                    beat,
                    (at, duration, x0, x1, ease),
                    stage,
                    events,
                    &blocking,
                ));
            }
            push_move(stage, actor, (at, at + duration, x0, x1, ease));
            at += duration;
            if look_back && i == 0 {
                at = frame_ceil(at + LOOK_BACK_SECONDS);
            }
        }
        if beat.face.is_none() {
            events.push(Event::Face {
                at,
                actor: actor.to_string(),
                value: before,
            });
        }
        if depth_to != depth_from {
            stage.depth.insert(actor.to_string(), depth_to);
            let keys = stage.scale_keys.entry(actor.to_string()).or_default();
            keys.push((t, size_from));
            keys.push((at, size_to));
        }
        Ok(Some(at.max(busy_until) - t))
    }

    /// `follow`: walk after `leader`'s travel in the previous beat, starting a
    /// moment after them and stopping a little behind them.
    #[allow(clippy::too_many_arguments)]
    fn follow(
        &mut self,
        path: &str,
        actor: &str,
        leader: &str,
        beat: &Beat,
        t: f64,
        stage: &mut Stage,
        events: &mut Vec<Event>,
        blocking: &Blocking,
        default_gait: &str,
    ) -> Result<Option<f64>, StoryIssue> {
        let w = f64::from(STORY_WIDTH);
        let root_y = f64::from(STORY_HEIGHT) * 0.7;
        let led: Vec<StageMove> = stage
            .moves
            .get(leader)
            .into_iter()
            .flatten()
            .filter(|m| m.0 >= blocking.previous_beat - 1e-9 && m.2 != m.3)
            .copied()
            .collect();
        let (Some(first), Some(last)) = (led.first(), led.last()) else {
            return Err(issue(
                IssueKind::Invalid,
                format!("{path}.follow"),
                format!("{leader} does not walk in the previous beat; put follow in the beat right after {leader}'s move_to"),
            ));
        };
        let start = frame_ceil(first.0 + FOLLOW_DELAY);
        if stage
            .moves
            .get(actor)
            .into_iter()
            .flatten()
            .any(|m| m.1 > start)
        {
            return Err(issue(
                IssueKind::Invalid,
                format!("{path}.follow"),
                format!("{actor} is still moving when {leader} sets off"),
            ));
        }
        let from = stage.x(actor, start);
        let direction = (last.3 - first.2).signum();
        let to = last.3 - direction * FOLLOW_GAP * w;
        if (to - from).abs() < 1.0 {
            self.notes.push(format!(
                "{path}: {actor} is already behind {leader}; follow was skipped"
            ));
            return Ok(None);
        }
        let leader_gait = events.iter().rev().find_map(|e| match e {
            Event::Move { actor: a, gait, .. } if a == leader => gait.clone(),
            _ => None,
        });
        let gait = beat
            .gait
            .clone()
            .or(leader_gait.filter(|g| g != "leap"))
            .unwrap_or_else(|| default_gait.into());
        let speed = gait_speed(&gait) * beat.pace.unwrap_or(1.0);
        // Never overtake: arrive after the leader has stopped.
        let duration = frame_ceil(
            ((to - from).abs() / speed)
                .max(0.6)
                .max(last.1 + FOLLOW_DELAY - start),
        );
        let (dy, _) =
            depth_of(stage.depth.get(actor).copied().unwrap_or("mid")).unwrap_or((0.0, 1.0));
        let offset = stage.start[actor] - w / 2.0;
        events.push(Event::Move {
            at: start,
            actor: actor.to_string(),
            x: to - offset,
            y: root_y + dy,
            duration,
            ease: Some(MoveEase::InOut.name().to_string()),
            pace: beat.pace.filter(|p| *p != 1.0),
            gait: Some(gait.clone()),
        });
        if beat.face.is_none() {
            let right = (to > from) != (gait == "backwards");
            events.push(Event::Face {
                at: start,
                actor: actor.to_string(),
                value: if right { "right" } else { "left" }.into(),
            });
            events.push(Event::Face {
                at: start + duration,
                actor: actor.to_string(),
                value: blocking
                    .facing
                    .get(actor)
                    .cloned()
                    .unwrap_or_else(|| "front".into()),
            });
        }
        push_move(
            stage,
            actor,
            (start, start + duration, from, to, MoveEase::InOut),
        );
        Ok(Some((start + duration - t).max(0.0)))
    }

    /// Standing actors that `actor`'s leg passes step upstage and back as the
    /// walker goes by. Returns when the last of them is back in place.
    #[allow(clippy::too_many_arguments)]
    fn clear_the_way(
        &mut self,
        path: &str,
        actor: &str,
        beat: &Beat,
        (at, duration, x0, x1, ease): (f64, f64, f64, f64, MoveEase),
        stage: &mut Stage,
        events: &mut Vec<Event>,
        blocking: &Blocking,
    ) -> f64 {
        let w = f64::from(STORY_WIDTH);
        let root_y = f64::from(STORY_HEIGHT) * 0.7;
        let (low, high) = (x0.min(x1) + 40.0, x0.max(x1) - 40.0);
        let named: Vec<&String> = beat
            .path
            .iter()
            .flatten()
            .chain(beat.move_to.iter())
            .collect();
        let mut until = at;
        let others: Vec<String> = stage.start.keys().cloned().collect();
        for other in others {
            if other == actor
                || named.contains(&&other)
                || blocking.posed.contains_key(&other)
                || stage.depth.get(&other) != stage.depth.get(actor)
            {
                continue;
            }
            let ox = stage.x(&other, at);
            if !(low < ox && ox < high) {
                continue;
            }
            // When the walker reaches them (the eased position is monotonic).
            let target = (ox - x0) / (x1 - x0);
            let (mut a, mut c) = (0.0, 1.0);
            for _ in 0..40 {
                let m = 0.5 * (a + c);
                if ease.apply(m) < target {
                    a = m;
                } else {
                    c = m;
                }
            }
            let crossing = at + duration * a;
            let aside = frame_ceil((crossing - 0.5).max(at));
            let back = frame_ceil((crossing + 0.3).max(aside + STEP_ASIDE_SECONDS));
            let end = back + STEP_ASIDE_SECONDS;
            let busy = stage
                .moves
                .get(&other)
                .into_iter()
                .flatten()
                .any(|m| m.0 < end && m.1 > aside);
            if busy {
                continue;
            }
            let (dy, _) =
                depth_of(stage.depth.get(&other).copied().unwrap_or("mid")).unwrap_or((0.0, 1.0));
            let x = ox - (stage.start[&other] - w / 2.0);
            for (when, y) in [(aside, root_y + dy - STEP_ASIDE), (back, root_y + dy)] {
                events.push(Event::Move {
                    at: when,
                    actor: other.clone(),
                    x,
                    y,
                    duration: STEP_ASIDE_SECONDS,
                    ease: Some(MoveEase::InOut.name().to_string()),
                    pace: None,
                    gait: Some("walk".into()),
                });
                push_move(
                    stage,
                    &other,
                    (when, when + STEP_ASIDE_SECONDS, ox, ox, MoveEase::InOut),
                );
            }
            self.notes
                .push(format!("{path}: {other} steps aside to let {actor} pass"));
            until = until.max(end);
        }
        until
    }

    fn framing(
        &self,
        shot: &str,
        subject: &str,
        partner: Option<&str>,
        stage: &Stage,
        t: f64,
    ) -> Framing {
        let partner = partner
            .filter(|p| *p != subject)
            .map(str::to_string)
            .or_else(|| stage.partner(subject, t));
        match (shot, partner) {
            ("wide", _) => Framing::Wide,
            ("close_up", _) => Framing::CloseUp(subject.into()),
            ("two_shot", Some(p)) => Framing::TwoShot(subject.into(), p),
            ("over_shoulder", Some(p)) => Framing::OverShoulder(subject.into(), p),
            _ => Framing::Medium(subject.into()),
        }
    }

    /// World-space view centre and zoom for a framing at time `t`, before
    /// camera movement and the set-bounds clamp.
    fn view(framing: &Framing, stage: &Stage, t: f64) -> (f64, f64, f64) {
        let (w, h) = (f64::from(STORY_WIDTH), f64::from(STORY_HEIGHT));
        let root_y = h * 0.7;
        // Vertical framing targets in unscaled rig pixels above the hip joint.
        let above = |pixels: f64| root_y - pixels * ACTOR_SCALE;
        let (cx, cy, zoom) = match framing {
            Framing::Wide => (w / 2.0, h / 2.0, 1.0),
            Framing::Medium(a) => (stage.x(a, t), above(100.0), 1.5),
            Framing::CloseUp(a) => (stage.x(a, t), above(170.0), 2.4),
            Framing::Reaction(a) => (stage.x(a, t), above(155.0), 2.0),
            Framing::TwoShot(a, b) => {
                let (xa, xb) = (stage.x(a, t), stage.x(b, t));
                let zoom = (w * 0.8 / ((xa - xb).abs() + 420.0)).clamp(1.0, 1.6);
                (
                    (xa + xb) / 2.0,
                    h / 2.0 + (above(90.0) - h / 2.0) * (zoom - 1.0) / 0.6,
                    zoom,
                )
            }
            Framing::OverShoulder(a, b) => {
                let (xa, xb) = (stage.x(a, t), stage.x(b, t));
                (xa + (xb - xa) * 0.3, above(140.0), 1.8)
            }
        };
        (cx, cy, zoom)
    }

    /// Apply a camera move at `progress` through the shot (`shake` uses the
    /// seconds since the shot started), then keep the view inside the set.
    fn moved_view(
        (cx, cy, zoom): (f64, f64, f64),
        movement: Option<&str>,
        progress: f64,
        since: f64,
    ) -> (f64, f64, f64) {
        let (w, h) = (f64::from(STORY_WIDTH), f64::from(STORY_HEIGHT));
        let p = progress.clamp(0.0, 1.0);
        let (mut cx, mut cy, mut zoom) = (cx, cy, zoom);
        match movement {
            Some("push_in") => zoom *= 1.0 + PUSH_RANGE * p,
            Some("pull_out") => zoom *= 1.0 + PUSH_RANGE * (1.0 - p),
            Some(side @ ("pan_left" | "pan_right")) => {
                zoom = zoom.max(PAN_MIN_ZOOM);
                let sweep = PAN_RANGE * w / zoom * (1.0 - 2.0 * p);
                cx += if side == "pan_left" { sweep } else { -sweep };
            }
            Some("shake") if since < SHAKE_SECONDS => {
                let k = since * f64::from(STORY_FPS);
                let decay = 1.0 - since / SHAKE_SECONDS;
                cx += SHAKE_PIXELS * decay * (k * 2.3).sin();
                cy += SHAKE_PIXELS * decay * (k * 3.1).cos();
            }
            _ => {}
        }
        // Keep the view inside the painted set.
        let (half_w, half_h) = (w / (2.0 * zoom), h / (2.0 * zoom));
        (
            cx.clamp(half_w, w - half_w),
            cy.clamp(half_h, h - half_h),
            zoom,
        )
    }

    fn camera(&self, cuts: &[Shot], stage: &Stage, duration: f64) -> Vec<CameraKey> {
        let (w, h) = (f64::from(STORY_WIDTH), f64::from(STORY_HEIGHT));
        let mut keys: Vec<CameraKey> = Vec::new();
        for (index, shot) in cuts.iter().enumerate() {
            let (start, framing, movement) = (&shot.at, &shot.framing, shot.movement.as_deref());
            let end = cuts
                .get(index + 1)
                .map_or(duration, |next| next.at - CUT_EPSILON);
            let length = end - start;
            // Without an explicit move, a slow push-in keeps held shots alive;
            // wide shots stay static.
            let push = if movement.is_some() || *framing == Framing::Wide || length < 1.5 {
                1.0
            } else {
                1.025
            };
            let mut times = vec![*start, end];
            if movement == Some("shake") {
                let frames = (SHAKE_SECONDS * f64::from(STORY_FPS)) as usize;
                times.extend(
                    (1..=frames)
                        .map(|k| start + k as f64 / f64::from(STORY_FPS))
                        .filter(|t| *t < end),
                );
            }
            for moves in stage.moves.values() {
                for &(t0, t1, ..) in moves {
                    // Quarter points (on frames) keep a framing on an easing actor.
                    times.extend(
                        (0..=4)
                            .map(|k| match k {
                                0 => t0,
                                4 => t1,
                                _ => frame_ceil(t0 + (t1 - t0) * f64::from(k) / 4.0),
                            })
                            .filter(|t| *t > *start && *t < end),
                    );
                }
            }
            times.sort_by(f64::total_cmp);
            times.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
            for at in times {
                let progress = if length > 0.0 {
                    (at - start) / length
                } else {
                    0.0
                };
                let (cx, cy, zoom) = Self::moved_view(
                    Self::view(framing, stage, at),
                    movement,
                    progress,
                    at - start,
                );
                let key = CameraKey {
                    at,
                    x: cx - w / 2.0,
                    y: cy - h / 2.0,
                    zoom: zoom * (1.0 + (push - 1.0) * progress),
                };
                if keys.last().is_none_or(|last| at > last.at) {
                    keys.push(key);
                }
            }
        }
        keys
    }
}

// ---------------------------------------------------------------------------
// Authoring guide

/// Instructions for a chat assistant, listing this catalog's characters and
/// sets. The user pastes it into the assistant together with a story idea.
pub fn authoring_guide(catalog: &StoryCatalog) -> String {
    let mut text = String::new();
    text.push_str(
        "You write animated short films for Kilsat Studio. Turn the user's idea into a story in the
Kilsat story format below. Reply with ONLY one ```yaml code block containing the story.

RULES
- Keys are English exactly as shown. Dialogue is written in the story language.
- Use only the characters, sets, positions and values listed here. Anything else is rejected.
- Every scene places its actors with positions; only those actors can act, speak or be looked at.
- A beat has exactly one of: say (dialogue), do (action) or pause.
- line is the exact words spoken: no stage directions, no speaker names, no emojis.
- Keep lines short (one or two sentences). Split long speeches into several say beats.
- Good films have rhythm: setup, a turn, a reaction, a pause before the punchline.
- Bystanders step aside automatically when someone walks past them.
- To enter, start an actor at offscreen_left or offscreen_right and move_to a position; to exit, move_to offscreen_left or offscreen_right.
- Short film: 1–6 scenes, 4–20 beats per scene, 30 seconds to 5 minutes in total.

FORMAT
kilsat_story: 1
title: <title>
language: fi | en | en-gb
captions: true            # optional, default true
cast:                     # characters used in the film
  <CHARACTER>: {}         # or {voice: <preset>, speed: 0.7–1.4, holds: <hand prop>, hand: left|right}
scenes:
  - id: <scene_id>        # a-z, 0-9, _
    set: <set>
    summary: <one sentence, optional>
    positions: {<CHARACTER>: <position>, ...}
    beats:
      - say: <CHARACTER>
        line: <spoken words>
        emotion: <emotion>          # optional
        gesture: <gesture>          # optional, played while speaking
        look: <CHARACTER>           # optional, whom the speaker looks at
        shot: <shot>                # optional, default auto
        camera: <camera move>       # optional
        on: <CHARACTER>             # optional, whom the camera frames
        hold: <seconds>             # optional pause after the line, 0–10
        move_to: <position or CHARACTER>  # optional, walks while talking
        gait: <gait>                # optional, with move_to
        reaction: {by: <CHARACTER>, emotion: <emotion>, gesture: <gesture>, hold: <seconds>}
      - do: <CHARACTER>
        emotion: <emotion>          # any of these, all optional
        gesture: <gesture>
        look: <CHARACTER>
        face: <facing>
        move_to: <position or CHARACTER>  # a CHARACTER: walk up to them
        path: [<stop>, <stop>, ...] # instead of move_to: 2–6 stops in one walk
        follow: <CHARACTER>         # instead of move_to: walk after whoever moved in the previous beat
        depth: near | mid | far     # walk toward or away from the camera
        pace: <0.5–1.5>             # optional speed factor for the walk
        look_back: true             # with move_to: stop half way and look back
        gait: <gait>                # with move_to, path, follow or depth
        holds: <hand prop>          # pick it up (after any walk); none puts it down
        hand: left | right          # optional, with holds
        pose: <pose>                # sit adds a chair; stand up before walking
        hold: <seconds>
        shot: <shot>
        camera: <camera move>       # say beats take camera too
      - pause: <seconds>            # 0.2–10, silence; optional shot and camera
\nVALUES\n",
    );
    text.push_str(&format!(
        "- position: {}\n",
        POSITIONS.map(|(n, _)| n).join(", ")
    ));
    text.push_str(&format!("- emotion: {}\n", list(&EMOTIONS)));
    text.push_str(&format!(
        "- camera move: {} (push_in/pull_out for emphasis, pan for reveals, shake for impacts)\n",
        list(&CAMERA_MOVES)
    ));
    text.push_str(&format!(
        "- gesture: {}\n",
        list(&motion_animation::GESTURES)
    ));
    text.push_str(&format!("- face: {}\n", list(&FACINGS)));
    text.push_str(&format!(
        "- gait: {} (default walk; backwards faces away from the travel direction; leap jumps to a neighbouring position)\n",
        list(&motion_animation::GAITS)
    ));
    text.push_str(&format!("- pose: {}\n", list(&crate::POSES)));
    text.push_str(&format!(
        "- shot: {} (auto: wide to open a scene, medium on the speaker, close_up on strong emotion, wide for walks; reaction cuts to the listener)\n",
        list(&SHOTS)
    ));
    if let Ok(props) = motion_rig::stick_studio_hand_props() {
        let ids: Vec<&str> = props.items.iter().map(|i| i.id.as_str()).collect();
        text.push_str(&format!("- holds (hand prop): {}\n", ids.join(", ")));
    }
    text.push_str("- voice: optional; English Kokoro presets such as am_michael, af_heart, bf_emma, bm_george; Finnish Piper presets such as piper_harri\n\nCHARACTERS\n");
    for (id, entry) in &catalog.characters {
        text.push_str(&format!("- {id} ({}): {}\n", entry.name, entry.description));
    }
    text.push_str("\nSETS\n");
    for (id, set) in &catalog.sets {
        text.push_str(&format!("- {id} ({}): {}\n", set.name, set.description));
    }
    let mut ids = catalog.characters.keys();
    let a = ids.next().map_or("A", String::as_str);
    let b = ids.next().map_or(a, String::as_str);
    let set = catalog.sets.keys().next().map_or("set", String::as_str);
    text.push_str(&format!(
        "
EXAMPLE
```yaml
kilsat_story: 1
title: Late Again
language: en
cast:
  {a}: {{}}
  {b}: {{}}
scenes:
  - id: arrival
    set: {set}
    summary: {b} arrives late and tries to hide it.
    positions: {{{a}: left, {b}: far_right}}
    beats:
      - do: {b}
        move_to: right
        gait: jog
      - say: {a}
        line: You're late. Again.
        emotion: angry
        look: {b}
      - say: {b}
        line: Late? I was early for tomorrow.
        emotion: happy
        gesture: shrug
        reaction: {{by: {a}, emotion: surprised}}
      - pause: 1.0
      - say: {a}
        line: Sit down.
        shot: close_up
```

Now write the story for this idea:
"
    ));
    text
}
