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

/// Named stage positions as fractions of the frame width.
pub const POSITIONS: [(&str, f64); 5] = [
    ("far_left", 0.14),
    ("left", 0.30),
    ("center", 0.50),
    ("right", 0.70),
    ("far_right", 0.86),
];
pub const SHOTS: [&str; 6] = [
    "auto",
    "wide",
    "medium",
    "close_up",
    "two_shot",
    "over_shoulder",
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
const GAIT_SPEEDS: [(&str, f64); 3] = [("walk", 260.0), ("jog", 450.0), ("run", 700.0)];

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
    pub move_to: Option<String>,
    pub gait: Option<String>,
    pub hold: Option<f64>,
    pub shot: Option<String>,
    pub on: Option<String>,
    pub reaction: Option<Reaction>,
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
    /// Scene-local start and reserved length in seconds.
    pub at: f64,
    pub duration: f64,
}

#[derive(Debug, Clone)]
pub struct StoryCompilation {
    pub script: Script,
    /// `(scene id, set id)` for each compiled scene, in order.
    pub scene_sets: Vec<(String, String)>,
    pub lines: Vec<CompiledLine>,
    pub shot_count: usize,
    pub duration: f64,
    pub timing: StoryTiming,
    /// Explicit adjustments the compiler made (for example a skipped move).
    pub notes: Vec<String>,
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
    vocabulary(out, "move_to", &beat.move_to, &positions);
    vocabulary(out, "gait", &beat.gait, &motion_animation::GAITS);
    vocabulary(out, "shot", &beat.shot, &SHOTS);
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
            || beat.move_to.is_some()
            || beat.gait.is_some()
            || beat.hold.is_some()
            || beat.reaction.is_some();
        if extra {
            out.push(issue(
                Schema,
                path,
                "a pause beat takes only pause, shot and on",
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
    if beat.gait.is_some() && beat.move_to.is_none() {
        out.push(issue(Schema, format!("{path}.gait"), "gait needs move_to"));
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
        if beat.move_to.is_some() {
            out.push(issue(
                Schema,
                format!("{path}.move_to"),
                "a say beat cannot move; put the walk in a do beat before the line",
            ));
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

/// Parse, validate and compile a story into a validated MotionScript [`Script`].
///
/// `line_seconds`, when given, holds one reserved length per `say` beat in
/// story order (for example measured speech); otherwise lengths are estimated.
pub fn compile_story(
    input: &str,
    catalog: &StoryCatalog,
    line_seconds: Option<&[f64]>,
) -> Result<StoryCompilation, Vec<StoryIssue>> {
    let story = parse_story(input, catalog)?;
    let say_count = story
        .scenes
        .iter()
        .flat_map(|s| &s.beats)
        .filter(|b| b.say.is_some())
        .count();
    if let Some(lengths) = line_seconds {
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
        if let Some(index) = lengths
            .iter()
            .position(|s| !s.is_finite() || *s <= 0.0 || *s > MAX_LINE_SECONDS)
        {
            return Err(vec![issue(
                IssueKind::Bound,
                format!("line {}", index + 1),
                format!("line length must be above 0 and at most {MAX_LINE_SECONDS} seconds"),
            )]);
        }
    }
    let mut builder = Builder::new(&story, catalog, line_seconds);
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
        timing: if line_seconds.is_some() {
            StoryTiming::Supplied
        } else {
            StoryTiming::Estimated
        },
        script,
        scene_sets,
        duration,
    })
}

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
    line_seconds: Option<&'a [f64]>,
    next_line: usize,
    lines: Vec<CompiledLine>,
    notes: Vec<String>,
    shots: usize,
}

/// Actor positions over one scene: a start offset plus sequential linear moves.
struct Stage {
    start: BTreeMap<String, f64>,
    moves: BTreeMap<String, Vec<(f64, f64, f64, f64)>>,
}
impl Stage {
    fn x(&self, actor: &str, t: f64) -> f64 {
        let mut x = self.start[actor];
        for &(t0, t1, x0, x1) in self.moves.get(actor).into_iter().flatten() {
            if t >= t1 {
                x = x1;
            } else if t > t0 {
                return x0 + (x1 - x0) * (t - t0) / (t1 - t0);
            } else {
                break;
            }
        }
        x
    }
    fn final_x(&self, actor: &str) -> f64 {
        self.x(actor, f64::INFINITY)
    }
    /// The nearest other on-stage actor at time `t`.
    fn partner(&self, actor: &str, t: f64) -> Option<String> {
        let x = self.x(actor, t);
        self.start
            .keys()
            .filter(|other| other.as_str() != actor)
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
    fn new(story: &'a Story, catalog: &'a StoryCatalog, line_seconds: Option<&'a [f64]>) -> Self {
        Self {
            story,
            catalog,
            line_seconds,
            next_line: 0,
            lines: Vec::new(),
            notes: Vec::new(),
            shots: 0,
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
            character.voice = Some(VoiceConfig {
                provider: provider.into(),
                preset,
                language: language.into(),
                speed: 1.0,
            });
            characters.insert(id.clone(), character);
        }
        Ok(characters)
    }

    fn has_eyes(&self, actor: &str) -> bool {
        self.catalog.characters[actor]
            .character
            .rig
            .as_ref()
            .is_some_and(|rig| rig.eyes.is_some())
    }

    fn scene(&mut self, index: usize, scene: &StoryScene) -> Result<Scene, StoryIssue> {
        let (w, h) = (f64::from(STORY_WIDTH), f64::from(STORY_HEIGHT));
        let root_y = h * 0.7;
        let mut stage = Stage {
            start: scene
                .positions
                .iter()
                .map(|(actor, place)| (actor.clone(), position_fraction(place).unwrap_or(0.5) * w))
                .collect(),
            moves: BTreeMap::new(),
        };
        let mut events: Vec<Event> = Vec::new();
        let mut gaze: BTreeMap<String, Vec<GazeKey>> = BTreeMap::new();
        let mut cuts: Vec<(f64, Framing)> = Vec::new();
        let mut content_end: f64 = 0.0;
        let mut facing: BTreeMap<String, String> = BTreeMap::new();

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
        let cut = |cuts: &mut Vec<(f64, Framing)>, t: f64, framing: Framing| match cuts.last_mut() {
            Some((_, last)) if *last == framing => {}
            Some((at, last)) if (*at - t).abs() < 1e-9 => *last = framing,
            _ => cuts.push((t, framing)),
        };
        let gesture_seconds =
            |within: Option<f64>| within.map_or(GESTURE_SECONDS, |d: f64| d.clamp(0.8, 1.6));

        let mut t = LEAD;
        let mut previous_speaker: Option<String> = None;
        for (b, beat) in scene.beats.iter().enumerate() {
            t = frame_ceil(t);
            let first = b == 0;
            let explicit = beat.shot.as_deref().filter(|s| *s != "auto");
            if let Some(pause) = beat.pause {
                if explicit.is_some() || first {
                    let subject = beat
                        .on
                        .clone()
                        .or_else(|| stage.start.keys().next().cloned())
                        .unwrap_or_default();
                    let framing =
                        self.framing(explicit.unwrap_or("wide"), &subject, None, &stage, t);
                    cut(&mut cuts, if first { 0.0 } else { t }, framing);
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
                let seconds = match self.line_seconds {
                    Some(lengths) => lengths[self.next_line],
                    None => estimate_line_seconds(&text),
                };
                self.next_line += 1;
                events.push(Event::Speak {
                    at: t,
                    actor: actor.clone(),
                    text: text.clone(),
                    emotion: beat.emotion.clone(),
                    audio: None,
                });
                self.lines.push(CompiledLine {
                    scene: scene.id.clone(),
                    actor: actor.clone(),
                    text,
                    at: t,
                    duration: seconds,
                });
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
                    cut(&mut cuts, if first { 0.0 } else { t }, framing);
                }
                let line_end = t + seconds;
                end = if let Some(reaction) = &beat.reaction {
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
                        });
                        content_end = content_end.max(at + duration);
                    }
                    look(&mut gaze, &reaction.by, at, &actor);
                    cut(&mut cuts, at, Framing::Reaction(reaction.by.clone()));
                    at + hold
                } else {
                    line_end + beat.hold.unwrap_or(0.0)
                };
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
                    });
                    length = length.max(GESTURE_SECONDS);
                }
                let mut moved = false;
                if let Some(place) = &beat.move_to {
                    let from = stage.final_x(&actor);
                    let to = position_fraction(place).unwrap_or(0.5) * w;
                    if (to - from).abs() < 1.0 {
                        self.notes.push(format!(
                            "scenes[{index}].beats[{b}]: {actor} is already at {place}; the move was skipped"
                        ));
                    } else {
                        let gait = beat.gait.clone().unwrap_or_else(|| "walk".into());
                        let speed = GAIT_SPEEDS
                            .iter()
                            .find(|(name, _)| *name == gait)
                            .map_or(GAIT_SPEEDS[0].1, |(_, speed)| *speed);
                        let duration = frame_ceil(((to - from).abs() / speed).max(0.6));
                        let offset = stage.start[&actor] - w / 2.0;
                        events.push(Event::Move {
                            at: t,
                            actor: actor.clone(),
                            x: to - offset,
                            y: root_y,
                            duration,
                            gait: Some(gait),
                        });
                        // Walk in profile unless the beat sets a facing itself.
                        if beat.face.is_none() {
                            let before = facing
                                .get(&actor)
                                .cloned()
                                .unwrap_or_else(|| "front".into());
                            let side = if to > from { "right" } else { "left" };
                            events.push(Event::Face {
                                at: t,
                                actor: actor.clone(),
                                value: side.into(),
                            });
                            events.push(Event::Face {
                                at: t + duration,
                                actor: actor.clone(),
                                value: before,
                            });
                        }
                        stage.moves.entry(actor.clone()).or_default().push((
                            t,
                            t + duration,
                            from,
                            to,
                        ));
                        length = length.max(duration);
                        moved = true;
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
                cut(&mut cuts, if first { 0.0 } else { t }, framing);
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
        if cuts.first().is_none_or(|(at, _)| *at > 0.0) {
            cuts.insert(0, (0.0, Framing::Wide));
        }
        self.shots += cuts.len();
        let camera = self.camera(&cuts, &stage, duration);
        events.sort_by(|a, b| a.actor_time().0.total_cmp(&b.actor_time().0));
        let cast_order: Vec<&String> = self.story.cast.keys().collect();
        let composition = Composition {
            background_color: self.catalog.sets[&scene.set].background_color.clone(),
            camera,
            layers: cast_order
                .iter()
                .enumerate()
                .map(|(z, id)| {
                    (
                        (*id).clone(),
                        Layer {
                            z: z as i32,
                            visible: stage.start.contains_key(*id),
                        },
                    )
                })
                .collect(),
            actor_tracks: stage
                .start
                .iter()
                .map(|(actor, x)| {
                    (
                        actor.clone(),
                        vec![ActorKey {
                            x: x - w / 2.0,
                            scale_x: ACTOR_SCALE,
                            scale_y: ACTOR_SCALE,
                            ..ActorKey::default()
                        }],
                    )
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

    /// World-space view centre and zoom for a framing at time `t`.
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
        // Keep the view inside the painted set.
        let (half_w, half_h) = (w / (2.0 * zoom), h / (2.0 * zoom));
        (
            cx.clamp(half_w, w - half_w),
            cy.clamp(half_h, h - half_h),
            zoom,
        )
    }

    fn camera(&self, cuts: &[(f64, Framing)], stage: &Stage, duration: f64) -> Vec<CameraKey> {
        let (w, h) = (f64::from(STORY_WIDTH), f64::from(STORY_HEIGHT));
        let mut keys: Vec<CameraKey> = Vec::new();
        for (index, (start, framing)) in cuts.iter().enumerate() {
            let end = cuts
                .get(index + 1)
                .map_or(duration, |(next, _)| next - CUT_EPSILON);
            let length = end - start;
            // A slow push-in keeps held shots alive; wide shots stay static.
            let push = if *framing == Framing::Wide || length < 1.5 {
                1.0
            } else {
                1.025
            };
            let mut times = vec![*start, end];
            for moves in stage.moves.values() {
                for &(t0, t1, _, _) in moves {
                    times.extend([t0, t1].into_iter().filter(|t| *t > *start && *t < end));
                }
            }
            times.sort_by(f64::total_cmp);
            times.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
            for at in times {
                let (cx, cy, zoom) = Self::view(framing, stage, at);
                let progress = if length > 0.0 {
                    (at - start) / length
                } else {
                    0.0
                };
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
- Short film: 1–6 scenes, 4–20 beats per scene, 30 seconds to 5 minutes in total.

FORMAT
kilsat_story: 1
title: <title>
language: fi | en | en-gb
captions: true            # optional, default true
cast:                     # characters used in the film
  <CHARACTER>: {}         # or {voice: <preset>, holds: <hand prop>, hand: left|right}
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
        on: <CHARACTER>             # optional, whom the camera frames
        hold: <seconds>             # optional pause after the line, 0–10
        reaction: {by: <CHARACTER>, emotion: <emotion>, gesture: <gesture>, hold: <seconds>}
      - do: <CHARACTER>
        emotion: <emotion>          # any of these, all optional
        gesture: <gesture>
        look: <CHARACTER>
        face: <facing>
        move_to: <position>
        gait: <gait>                # with move_to
        hold: <seconds>
        shot: <shot>
      - pause: <seconds>            # 0.2–10, silence; optional shot
\nVALUES\n",
    );
    text.push_str(&format!(
        "- position: {}\n",
        POSITIONS.map(|(n, _)| n).join(", ")
    ));
    text.push_str(&format!("- emotion: {}\n", list(&EMOTIONS)));
    text.push_str(&format!(
        "- gesture: {}\n",
        list(&motion_animation::GESTURES)
    ));
    text.push_str(&format!("- face: {}\n", list(&FACINGS)));
    text.push_str(&format!("- gait: {}\n", list(&motion_animation::GAITS)));
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
