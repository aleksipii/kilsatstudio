//! `ScriptIntent v1` intake for the bounded screenplay text format (plan
//! `docs/automation`, rules ING-002, ING-004 and ING-005).
//!
//! This is a transfer format that sits *before* the MotionScript compiler. It
//! does not parse YAML, evaluate scenes or own a clock. It reads a title line,
//! a duration/location line, a character line and `[a–b]` timed lines in Finnish
//! or English, keeps every dialogue line byte-for-byte with its UTF-8 byte
//! range in the original source and the source SHA-256, and reports anything it
//! cannot interpret at its source location instead of guessing.
//!
//! Intent times are scene-local seconds kept at millisecond precision; the
//! resolved plans convert them once to the shared 24 kHz sample clock.

use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::fmt;

/// Only supported `ScriptIntent` version.
pub const SCRIPT_INTENT_VERSION: u32 = 1;
/// Rule pack identifier recorded in provenance.
pub const INTENT_RULE_PACK: &str = "proposal-v1";
/// Largest accepted raw source.
pub const MAX_INTENT_SOURCE_BYTES: usize = 64 * 1024;
/// Largest total dialogue length in Unicode scalar values.
pub const MAX_INTENT_DIALOGUE_CHARS: usize = 1000;
/// Largest number of timed lines (and therefore beats).
pub const MAX_INTENT_BEATS: usize = 64;
/// Largest number of characters.
pub const MAX_INTENT_CHARACTERS: usize = 2;
/// Quick-profile duration bounds, in milliseconds.
pub const MIN_INTENT_DURATION_MS: u32 = 30_000;
pub const MAX_INTENT_DURATION_MS: u32 = 60_000;
const MAX_SCENE_ID: usize = 64;
const MAX_NAME_CHARS: usize = 32;

/// Script language; the same value is used for speech and captions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum IntentLanguage {
    #[serde(rename = "fi-FI")]
    FiFi,
    #[serde(rename = "en-US")]
    EnUs,
    #[serde(rename = "en-GB")]
    EnGb,
}

impl IntentLanguage {
    pub fn parse(tag: &str) -> Option<Self> {
        match tag {
            "fi-FI" => Some(Self::FiFi),
            "en-US" => Some(Self::EnUs),
            "en-GB" => Some(Self::EnGb),
            _ => None,
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Self::FiFi => "fi-FI",
            Self::EnUs => "en-US",
            Self::EnGb => "en-GB",
        }
    }

    fn family(self) -> Family {
        match self {
            Self::FiFi => Family::Finnish,
            Self::EnUs | Self::EnGb => Family::English,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    Finnish,
    English,
}

/// Choices the text cannot carry. They are explicit: language variant
/// (en-US vs en-GB is not inferred), seed and the stable story-scene id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntakeOptions {
    pub language: IntentLanguage,
    pub seed: u32,
    pub scene_id: String,
}

/// Non-negative scene-local time with millisecond precision. Serialized as a
/// JSON integer when whole, otherwise as a decimal number of seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seconds(u32);

impl Seconds {
    pub fn from_millis(ms: u32) -> Self {
        Self(ms)
    }
    pub fn millis(self) -> u32 {
        self.0
    }
    pub fn as_secs_f64(self) -> f64 {
        f64::from(self.0) / 1000.0
    }
}

impl Serialize for Seconds {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.0.is_multiple_of(1000) {
            s.serialize_u32(self.0 / 1000)
        } else {
            s.serialize_f64(self.as_secs_f64())
        }
    }
}

/// Half-open UTF-8 byte range `[start_byte, end_byte)` into the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct SourceSpan {
    pub start_byte: usize,
    pub end_byte: usize,
}

impl SourceSpan {
    /// The original text, or `None` when out of range or not on a character boundary.
    pub fn slice(self, source: &str) -> Option<&str> {
        source.get(self.start_byte..self.end_byte)
    }

    /// 1-based line and column (in characters) of the span start, for display.
    pub fn line_column(self, source: &str) -> (usize, usize) {
        let before = source.get(..self.start_byte).unwrap_or(source);
        let line = before.matches('\n').count() + 1;
        let col = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        (line, col)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetStatus {
    /// No character or environment package has been chosen or authored yet.
    ToBeAuthored,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct IntentCharacter {
    pub id: String,
    pub name: String,
    pub asset_status: AssetStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct IntentScene {
    pub id: String,
    pub duration_s: Seconds,
    pub environment_status: AssetStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BeatKind {
    Dialogue,
    Action,
    Hold,
}

/// Bounded action vocabulary from the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentAction {
    LookAt,
    PresentProp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceKind {
    /// Taken verbatim from the source (dialogue).
    Authored,
    /// Mapped from source text by a named rule (bounded action vocabulary).
    Derived,
    /// Preserved but not interpreted; a person must decide.
    NeedsReview,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Provenance {
    pub kind: ProvenanceKind,
    pub rule_id: &'static str,
    pub rule_pack: &'static str,
}

impl Provenance {
    fn new(kind: ProvenanceKind, rule_id: &'static str) -> Self {
        Self {
            kind,
            rule_id,
            rule_pack: INTENT_RULE_PACK,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct IntentBeat {
    pub id: String,
    pub scene: String,
    pub start_s: Seconds,
    pub end_s: Seconds,
    pub kind: BeatKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<IntentAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Dialogue exactly as written (dialogue beats only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub source_text: String,
    pub source_span: SourceSpan,
    pub provenance: Provenance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnresolvedCode {
    /// Action text outside the bounded vocabulary (ING-004 `UnsupportedAction`).
    UnsupportedAction,
    /// Timed lines end before the declared duration (`SCRIPT_UNDERFILLED`).
    TimelineUnderfilled,
}

/// A source location the intake preserved but did not interpret.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Unresolved {
    pub code: UnresolvedCode,
    pub rule_id: &'static str,
    pub scene: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_s: Option<Seconds>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_s: Option<Seconds>,
    pub source_text: String,
    pub source_span: SourceSpan,
    pub status: ProvenanceKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioStatus {
    MissingRecording,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderStatus {
    NotRendered,
}

/// `ScriptIntent v1`. Field order follows the committed pilot fixtures.
/// Direction-owned fields of those fixtures (`shot_proposal`, `app_insert`)
/// are not produced here.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScriptIntent {
    pub version: u32,
    pub seed: u32,
    pub language: IntentLanguage,
    pub duration_s: Seconds,
    pub source_sha256: String,
    pub characters: Vec<IntentCharacter>,
    pub scenes: Vec<IntentScene>,
    pub beats: Vec<IntentBeat>,
    pub unresolved: Vec<Unresolved>,
    pub audio_status: AudioStatus,
    pub render_status: RenderStatus,
    pub speech_language: IntentLanguage,
    pub caption_language: IntentLanguage,
}

impl ScriptIntent {
    /// Deterministic pretty JSON with a trailing newline.
    pub fn to_json_pretty(&self) -> String {
        let mut out = serde_json::to_string_pretty(self).expect("intent serializes");
        out.push('\n');
        out
    }

    /// Rejects the intent when `source` is not the exact text it was made
    /// from: hash mismatch, or any span no longer returning its stored text.
    pub fn verify_against(&self, source: &[u8]) -> Result<(), IntakeError> {
        if sha256_hex(source) != self.source_sha256 {
            return Err(IntakeError::new(
                IntakeErrorKind::StaleSource,
                None,
                "source SHA-256 differs from the intent; re-run intake",
            ));
        }
        let text = std::str::from_utf8(source).map_err(|_| {
            IntakeError::new(IntakeErrorKind::InvalidUtf8, None, "source is not UTF-8")
        })?;
        let spans = self
            .beats
            .iter()
            .map(|b| (b.source_span, &b.source_text))
            .chain(
                self.unresolved
                    .iter()
                    .map(|u| (u.source_span, &u.source_text)),
            );
        for (span, stored) in spans {
            if span.slice(text) != Some(stored.as_str()) {
                return Err(IntakeError::new(
                    IntakeErrorKind::StaleSource,
                    Some(span),
                    "source span no longer returns the stored text",
                ));
            }
        }
        Ok(())
    }
}

/// Header facts that the intent contract does not carry but a review UI shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenplayHeader {
    pub title: String,
    pub title_span: SourceSpan,
    pub location: String,
    pub location_span: SourceSpan,
    pub characters: Vec<HeaderCharacter>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderCharacter {
    pub name: String,
    pub role: Option<String>,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Intake {
    pub header: ScreenplayHeader,
    pub intent: ScriptIntent,
}

/// Broad class of an intake error (AGENTS: syntax vs reference vs unsupported).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntakeErrorCategory {
    Syntax,
    InvalidReference,
    Unsupported,
    Bound,
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntakeErrorKind {
    InvalidUtf8,
    SourceTooLarge,
    InvalidOption,
    MissingHeader,
    UnrecognizedLine,
    InvalidTime,
    LanguageMismatch,
    DurationOutOfRange,
    NoCharacters,
    TooManyCharacters,
    DuplicateCharacter,
    InvalidCharacterName,
    UnsupportedSceneMarker,
    InvalidInterval,
    OutOfOrder,
    ExceedsDuration,
    OverlappingDialogue,
    OverlappingSpeakers,
    UnknownSpeaker,
    AmbiguousSpeaker,
    MissingColon,
    EmptyText,
    NoTimedLines,
    TooManyBeats,
    DialogueTooLong,
    StaleSource,
}

impl IntakeErrorKind {
    pub fn category(self) -> IntakeErrorCategory {
        use IntakeErrorCategory as C;
        use IntakeErrorKind as K;
        match self {
            K::InvalidUtf8
            | K::MissingHeader
            | K::UnrecognizedLine
            | K::InvalidTime
            | K::MissingColon
            | K::EmptyText
            | K::InvalidCharacterName
            | K::NoTimedLines => C::Syntax,
            K::UnknownSpeaker | K::AmbiguousSpeaker | K::StaleSource => C::InvalidReference,
            K::UnsupportedSceneMarker | K::OverlappingSpeakers => C::Unsupported,
            K::SourceTooLarge
            | K::DurationOutOfRange
            | K::TooManyCharacters
            | K::TooManyBeats
            | K::DialogueTooLong
            | K::ExceedsDuration => C::Bound,
            K::InvalidOption
            | K::LanguageMismatch
            | K::NoCharacters
            | K::DuplicateCharacter
            | K::InvalidInterval
            | K::OutOfOrder
            | K::OverlappingDialogue => C::Invalid,
        }
    }
}

/// One error with its source location when it has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntakeError {
    pub kind: IntakeErrorKind,
    pub span: Option<SourceSpan>,
    pub message: String,
}

impl IntakeError {
    fn new(kind: IntakeErrorKind, span: Option<SourceSpan>, message: impl Into<String>) -> Self {
        Self {
            kind,
            span,
            message: message.into(),
        }
    }
}

impl fmt::Display for IntakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.span {
            Some(s) => write!(
                f,
                "{:?} at bytes {}..{}: {}",
                self.kind, s.start_byte, s.end_byte, self.message
            ),
            None => write!(f, "{:?}: {}", self.kind, self.message),
        }
    }
}

impl std::error::Error for IntakeError {}

/// All errors found; intake stops at header errors, otherwise reports every line.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("screenplay intake failed with {} error(s); first: {}", .errors.len(), .errors[0])]
pub struct IntakeFailure {
    pub errors: Vec<IntakeError>,
}

impl IntakeFailure {
    fn one(e: IntakeError) -> Self {
        Self { errors: vec![e] }
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Clone, Copy)]
struct Line<'a> {
    text: &'a str,
    start: usize,
}

impl<'a> Line<'a> {
    fn span(&self) -> SourceSpan {
        SourceSpan {
            start_byte: self.start,
            end_byte: self.start + self.text.len(),
        }
    }
    /// Span of `part`, which must be a subslice of `self.text`.
    fn sub(&self, part: &str) -> SourceSpan {
        let off = part.as_ptr() as usize - self.text.as_ptr() as usize;
        SourceSpan {
            start_byte: self.start + off,
            end_byte: self.start + off + part.len(),
        }
    }
}

fn split_lines(src: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for raw in src.split_inclusive('\n') {
        let text = raw.strip_suffix('\n').unwrap_or(raw);
        let text = text.strip_suffix('\r').unwrap_or(text);
        lines.push(Line { text, start });
        start += raw.len();
    }
    lines
}

fn err(kind: IntakeErrorKind, span: SourceSpan, msg: impl Into<String>) -> IntakeError {
    IntakeError::new(kind, Some(span), msg)
}

/// Parses `digits[.digits]` (up to 3 decimals) into milliseconds. Rejects
/// signs, exponents, NaN and infinity by construction.
fn parse_millis(s: &str) -> Option<u32> {
    let (whole, frac) = match s.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (s, None),
    };
    if whole.is_empty() || whole.len() > 4 || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut ms = whole.parse::<u32>().ok()?.checked_mul(1000)?;
    if let Some(f) = frac {
        if f.is_empty() || f.len() > 3 || !f.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let scale = [100, 10, 1][f.len() - 1];
        ms = ms.checked_add(f.parse::<u32>().ok()? * scale)?;
    }
    Some(ms)
}

fn strip_any<'a>(s: &'a str, prefixes: &[(&str, Family)]) -> Option<(&'a str, Family)> {
    prefixes
        .iter()
        .find_map(|(p, fam)| s.strip_prefix(p).map(|rest| (rest, *fam)))
}

fn is_speaker_label(label: &str) -> bool {
    !label.is_empty()
        && label.chars().any(char::is_alphabetic)
        && label
            .chars()
            .all(|c| c.is_uppercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn valid_scene_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_SCENE_ID
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

struct HeaderOut {
    header: ScreenplayHeader,
    duration_ms: u32,
    duration_span: SourceSpan,
    characters: Vec<IntentCharacter>,
}

fn check_family(found: Family, want: Family, span: SourceSpan) -> Result<(), IntakeError> {
    if found == want {
        Ok(())
    } else {
        Err(err(
            IntakeErrorKind::LanguageMismatch,
            span,
            "header keywords are in a different language from the selected script language",
        ))
    }
}

fn parse_header(lines: &[Line<'_>], family: Family) -> Result<HeaderOut, IntakeError> {
    let missing = |what: &str| {
        IntakeError::new(
            IntakeErrorKind::MissingHeader,
            None,
            format!("missing {what} line (expected title, duration/location and characters)"),
        )
    };
    // Title.
    let title_line = lines.first().ok_or_else(|| missing("title"))?;
    let title = title_line.text.trim();
    if title.is_empty() || title.starts_with('[') {
        return Err(err(
            IntakeErrorKind::MissingHeader,
            title_line.span(),
            "first line must be a non-empty title",
        ));
    }
    // Duration and location.
    let dl = lines.get(1).ok_or_else(|| missing("duration/location"))?;
    let (rest, fam) = strip_any(
        dl.text,
        &[("Kesto:", Family::Finnish), ("Duration:", Family::English)],
    )
    .ok_or_else(|| {
        err(
            IntakeErrorKind::MissingHeader,
            dl.span(),
            "second line must start with 'Kesto:' or 'Duration:'",
        )
    })?;
    check_family(fam, family, dl.span())?;
    let rest = rest.trim_start();
    let (number, after) = rest.split_once(' ').unwrap_or((rest, ""));
    let duration_span = dl.sub(number);
    let duration_ms = parse_millis(number).ok_or_else(|| {
        err(
            IntakeErrorKind::InvalidTime,
            duration_span,
            "duration must be a plain number of seconds such as 50 or 47.5",
        )
    })?;
    let unit = match family {
        Family::Finnish => "sekuntia.",
        Family::English => "seconds.",
    };
    let loc_part = after.strip_prefix(unit).ok_or_else(|| {
        err(
            IntakeErrorKind::MissingHeader,
            dl.span(),
            format!("expected '{unit}' after the duration"),
        )
    })?;
    let loc_part = loc_part.trim_start();
    let (loc_rest, loc_fam) = strip_any(
        loc_part,
        &[("Paikka:", Family::Finnish), ("Location:", Family::English)],
    )
    .ok_or_else(|| {
        err(
            IntakeErrorKind::MissingHeader,
            dl.span(),
            "expected 'Paikka:' or 'Location:' after the duration",
        )
    })?;
    check_family(loc_fam, family, dl.span())?;
    let location = loc_rest.trim();
    let location = location.strip_suffix('.').unwrap_or(location).trim_end();
    if location.is_empty() {
        return Err(err(
            IntakeErrorKind::MissingHeader,
            dl.span(),
            "location is empty",
        ));
    }
    if !(MIN_INTENT_DURATION_MS..=MAX_INTENT_DURATION_MS).contains(&duration_ms) {
        return Err(err(
            IntakeErrorKind::DurationOutOfRange,
            duration_span,
            "quick-profile duration must be 30..60 seconds",
        ));
    }
    // Characters.
    let cl = lines.get(2).ok_or_else(|| missing("characters"))?;
    let (list, fam) = strip_any(
        cl.text,
        &[
            ("Hahmot:", Family::Finnish),
            ("Characters:", Family::English),
        ],
    )
    .ok_or_else(|| {
        err(
            IntakeErrorKind::MissingHeader,
            cl.span(),
            "third line must start with 'Hahmot:' or 'Characters:'",
        )
    })?;
    check_family(fam, family, cl.span())?;
    let list = list.trim();
    let list = list.strip_suffix('.').unwrap_or(list);
    let mut header_chars = Vec::new();
    let mut characters: Vec<IntentCharacter> = Vec::new();
    for entry in list.split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let (name, role) = match entry.split_once(',') {
            Some((n, r)) => (n.trim(), Some(r.trim()).filter(|r| !r.is_empty())),
            None => (entry, None),
        };
        let span = cl.sub(entry);
        if name.is_empty()
            || name.chars().count() > MAX_NAME_CHARS
            || !name.chars().all(|c| c.is_alphabetic() || c == '-')
            || !name.chars().next().is_some_and(char::is_uppercase)
        {
            return Err(err(
                IntakeErrorKind::InvalidCharacterName,
                span,
                "character names are one capitalised word of letters or '-'",
            ));
        }
        let id = name.to_uppercase();
        if characters.iter().any(|c| c.id == id) {
            return Err(err(
                IntakeErrorKind::DuplicateCharacter,
                span,
                format!("character {id} is declared twice"),
            ));
        }
        if characters.len() == MAX_INTENT_CHARACTERS {
            return Err(err(
                IntakeErrorKind::TooManyCharacters,
                span,
                "the quick profile allows at most two characters",
            ));
        }
        characters.push(IntentCharacter {
            id,
            name: name.to_string(),
            asset_status: AssetStatus::ToBeAuthored,
        });
        header_chars.push(HeaderCharacter {
            name: name.to_string(),
            role: role.map(str::to_string),
            span,
        });
    }
    if characters.is_empty() {
        return Err(err(
            IntakeErrorKind::NoCharacters,
            cl.span(),
            "declare one or two characters",
        ));
    }
    Ok(HeaderOut {
        header: ScreenplayHeader {
            title: title.to_string(),
            title_span: title_line.sub(title),
            location: location.to_string(),
            location_span: dl.sub(location),
            characters: header_chars,
        },
        duration_ms,
        duration_span,
        characters,
    })
}

/// Classified body of a timed line.
enum Body<'a> {
    Dialogue { actor: String, text: &'a str },
    Action(&'a str),
}

fn classify<'a>(
    line: &Line<'a>,
    rest: &'a str,
    characters: &[IntentCharacter],
) -> Result<Body<'a>, IntakeError> {
    if let Some((label, after)) = rest.split_once(':') {
        if is_speaker_label(label) {
            if !characters.iter().any(|c| c.id == label) {
                return Err(err(
                    IntakeErrorKind::UnknownSpeaker,
                    line.sub(label),
                    format!("speaker {label} is not declared on the character line; no character is added"),
                ));
            }
            let text = after.trim();
            if text.is_empty() {
                return Err(err(
                    IntakeErrorKind::EmptyText,
                    line.sub(label),
                    "dialogue text is empty",
                ));
            }
            return Ok(Body::Dialogue {
                actor: label.to_string(),
                text,
            });
        }
        if !label.contains(char::is_whitespace)
            && characters.iter().any(|c| c.id == label.to_uppercase())
        {
            return Err(err(
                IntakeErrorKind::AmbiguousSpeaker,
                line.sub(label),
                format!(
                    "'{label}:' looks like a speaker but speaker labels must be upper case ({}:); not guessed",
                    label.to_uppercase()
                ),
            ));
        }
    }
    let first = rest.split_whitespace().next().unwrap_or("");
    if first.chars().count() >= 2
        && is_speaker_label(first)
        && characters.iter().any(|c| c.id == first)
    {
        return Err(err(
            IntakeErrorKind::MissingColon,
            line.sub(first),
            format!(
                "speaker {first} needs a colon ('{first}: …'); not guessed as dialogue or action"
            ),
        ));
    }
    Ok(Body::Action(rest))
}

/// Beat kind, actor, action and target of a vocabulary match.
type ActionMatch = (
    BeatKind,
    Option<String>,
    Option<IntentAction>,
    Option<String>,
);

/// Bounded ING-004 vocabulary over a whole single-sentence action line.
fn match_action(text: &str, family: Family, characters: &[IntentCharacter]) -> Option<ActionMatch> {
    let sentence = text.strip_suffix('.').unwrap_or(text).trim();
    if sentence.contains(['.', '!', '?', ';']) {
        return None;
    }
    let (hold, look, phone): (&str, &str, &[&str]) = match family {
        Family::Finnish => ("tauko", " katsoo ", &[" näyttää puhelimen"]),
        Family::English => (
            "pause",
            " looks at ",
            &[
                " shows the phone",
                " shows his phone",
                " shows her phone",
                " shows their phone",
            ],
        ),
    };
    if sentence.to_lowercase() == hold {
        return Some((BeatKind::Hold, None, None, None));
    }
    for c in characters {
        let Some(rest) = sentence.strip_prefix(c.name.as_str()) else {
            continue;
        };
        if phone.contains(&rest) {
            return Some((
                BeatKind::Action,
                Some(c.id.clone()),
                Some(IntentAction::PresentProp),
                Some("phone".into()),
            ));
        }
        if let Some(target) = rest.strip_prefix(look) {
            let found = characters
                .iter()
                .filter(|t| t.id != c.id)
                .find(|t| match family {
                    Family::English => target == t.name,
                    // Nominative or the partitive -a/-ä ending (katsoo Ranea).
                    Family::Finnish => {
                        target == t.name
                            || target == format!("{}a", t.name)
                            || target == format!("{}ä", t.name)
                    }
                });
            if let Some(t) = found {
                return Some((
                    BeatKind::Action,
                    Some(c.id.clone()),
                    Some(IntentAction::LookAt),
                    Some(t.id.clone()),
                ));
            }
        }
    }
    None
}

/// Parses `[a–b] rest` (en dash or hyphen). Returns times and the rest.
fn parse_timed<'a>(line: &Line<'a>) -> Result<(u32, u32, &'a str), IntakeError> {
    let text = line.text;
    let close = text.find(']').ok_or_else(|| {
        err(
            IntakeErrorKind::InvalidTime,
            line.span(),
            "timed line needs '[start–end]'",
        )
    })?;
    let inner = &text[1..close];
    let (a, b) = inner
        .split_once('–')
        .or_else(|| inner.split_once('-'))
        .ok_or_else(|| {
            err(
                IntakeErrorKind::InvalidTime,
                line.sub(inner),
                "expected '[start–end]'",
            )
        })?;
    let bad = |s: &str| {
        err(
            IntakeErrorKind::InvalidTime,
            line.sub(s),
            "time must be plain seconds such as 3 or 3.25",
        )
    };
    let start = parse_millis(a.trim()).ok_or_else(|| bad(a))?;
    let end = parse_millis(b.trim()).ok_or_else(|| bad(b))?;
    let rest = text[close + 1..].trim();
    if rest.is_empty() {
        return Err(err(
            IntakeErrorKind::EmptyText,
            line.span(),
            "timed line has no text",
        ));
    }
    Ok((start, end, rest))
}

/// Converts bounded screenplay text into `ScriptIntent v1`.
pub fn script_intent_from_text(
    source: &[u8],
    options: &IntakeOptions,
) -> Result<Intake, IntakeFailure> {
    if source.len() > MAX_INTENT_SOURCE_BYTES {
        return Err(IntakeFailure::one(IntakeError::new(
            IntakeErrorKind::SourceTooLarge,
            None,
            format!(
                "source is {} bytes; limit is {MAX_INTENT_SOURCE_BYTES}",
                source.len()
            ),
        )));
    }
    if !valid_scene_id(&options.scene_id) {
        return Err(IntakeFailure::one(IntakeError::new(
            IntakeErrorKind::InvalidOption,
            None,
            "scene id must be 1..64 of [a-z0-9_-]",
        )));
    }
    let text = std::str::from_utf8(source).map_err(|e| {
        let at = e.valid_up_to();
        IntakeFailure::one(IntakeError::new(
            IntakeErrorKind::InvalidUtf8,
            Some(SourceSpan {
                start_byte: at,
                end_byte: at + 1,
            }),
            "source is not valid UTF-8",
        ))
    })?;
    let family = options.language.family();
    let lines = split_lines(text);
    let head = parse_header(&lines, family).map_err(IntakeFailure::one)?;
    let scene = options.scene_id.clone();

    let mut errors = Vec::new();
    let mut beats = Vec::new();
    let mut unresolved = Vec::new();
    let mut timed = 0usize;
    let mut last_start = 0u32;
    let mut last_end = 0u32;
    // (start, end, actor) of accepted dialogue, in source order.
    let mut dialogue: Vec<(u32, u32, String)> = Vec::new();
    let mut dialogue_chars = 0usize;

    for line in lines.iter().skip(3) {
        let trimmed = line.text.trim();
        if trimmed.is_empty() {
            continue;
        }
        let upper = trimmed.to_uppercase();
        if upper.starts_with("KOHTAUS") || upper.starts_with("SCENE") {
            errors.push(err(
                IntakeErrorKind::UnsupportedSceneMarker,
                line.sub(trimmed),
                "scene markers are not implemented in this intake slice; one story scene per file",
            ));
            continue;
        }
        if !line.text.starts_with('[') {
            errors.push(err(
                IntakeErrorKind::UnrecognizedLine,
                line.sub(trimmed),
                "expected a timed line '[start–end] …'; continuation lines are not supported",
            ));
            continue;
        }
        timed += 1;
        if timed > MAX_INTENT_BEATS {
            errors.push(err(
                IntakeErrorKind::TooManyBeats,
                line.span(),
                "more than 64 timed lines",
            ));
            break;
        }
        let (start, end, rest) = match parse_timed(line) {
            Ok(v) => v,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        let span = line.span();
        if start >= end {
            errors.push(err(
                IntakeErrorKind::InvalidInterval,
                span,
                "end must be after start ([start,end) is half-open)",
            ));
            continue;
        }
        if end > head.duration_ms {
            errors.push(err(
                IntakeErrorKind::ExceedsDuration,
                span,
                "line ends after the declared duration",
            ));
        }
        if start < last_start {
            errors.push(err(
                IntakeErrorKind::OutOfOrder,
                span,
                "timed lines must be in start-time order",
            ));
        }
        last_start = last_start.max(start);
        last_end = last_end.max(end);
        let body = match classify(line, rest, &head.characters) {
            Ok(b) => b,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        let id = format!("b{:02}", beats.len() + 1);
        match body {
            Body::Dialogue { actor, text: dl } => {
                if let Some((_, _, other)) =
                    dialogue.iter().find(|(s, e, _)| start < *e && *s < end)
                {
                    let (kind, msg) = if *other == actor {
                        (
                            IntakeErrorKind::OverlappingDialogue,
                            "overlaps earlier dialogue by the same character",
                        )
                    } else {
                        (
                            IntakeErrorKind::OverlappingSpeakers,
                            "overlapping speakers are not supported",
                        )
                    };
                    errors.push(err(kind, span, msg));
                    continue;
                }
                dialogue.push((start, end, actor.clone()));
                dialogue_chars += dl.chars().count();
                beats.push(IntentBeat {
                    id,
                    scene: scene.clone(),
                    start_s: Seconds(start),
                    end_s: Seconds(end),
                    kind: BeatKind::Dialogue,
                    actor: Some(actor),
                    action: None,
                    target: None,
                    text: Some(dl.to_string()),
                    source_text: dl.to_string(),
                    source_span: line.sub(dl),
                    provenance: Provenance::new(ProvenanceKind::Authored, "ING-002"),
                });
            }
            Body::Action(act) => match match_action(act, family, &head.characters) {
                Some((kind, actor, action, target)) => beats.push(IntentBeat {
                    id,
                    scene: scene.clone(),
                    start_s: Seconds(start),
                    end_s: Seconds(end),
                    kind,
                    actor,
                    action,
                    target,
                    text: None,
                    source_text: act.to_string(),
                    source_span: line.sub(act),
                    provenance: Provenance::new(ProvenanceKind::Derived, "ING-004"),
                }),
                None => unresolved.push(Unresolved {
                    code: UnresolvedCode::UnsupportedAction,
                    rule_id: "ING-004",
                    scene: scene.clone(),
                    start_s: Some(Seconds(start)),
                    end_s: Some(Seconds(end)),
                    source_text: act.to_string(),
                    source_span: line.sub(act),
                    status: ProvenanceKind::NeedsReview,
                }),
            },
        }
    }
    if dialogue_chars > MAX_INTENT_DIALOGUE_CHARS {
        errors.push(IntakeError::new(
            IntakeErrorKind::DialogueTooLong,
            None,
            format!("dialogue has {dialogue_chars} characters; quick-profile limit is {MAX_INTENT_DIALOGUE_CHARS}"),
        ));
    }
    if timed == 0 && errors.is_empty() {
        errors.push(IntakeError::new(
            IntakeErrorKind::NoTimedLines,
            None,
            "no '[start–end]' lines",
        ));
    }
    if !errors.is_empty() {
        return Err(IntakeFailure { errors });
    }
    if last_end < head.duration_ms {
        let number = head.duration_span;
        unresolved.push(Unresolved {
            code: UnresolvedCode::TimelineUnderfilled,
            rule_id: "ING-005",
            scene: scene.clone(),
            start_s: Some(Seconds(last_end)),
            end_s: Some(Seconds(head.duration_ms)),
            source_text: number.slice(text).unwrap_or_default().to_string(),
            source_span: number,
            status: ProvenanceKind::NeedsReview,
        });
    }
    let duration = Seconds(head.duration_ms);
    Ok(Intake {
        header: head.header,
        intent: ScriptIntent {
            version: SCRIPT_INTENT_VERSION,
            seed: options.seed,
            language: options.language,
            duration_s: duration,
            source_sha256: sha256_hex(source),
            characters: head.characters,
            scenes: vec![IntentScene {
                id: scene,
                duration_s: duration,
                environment_status: AssetStatus::ToBeAuthored,
            }],
            beats,
            unresolved,
            audio_status: AudioStatus::MissingRecording,
            render_status: RenderStatus::NotRendered,
            speech_language: options.language,
            caption_language: options.language,
        },
    })
}

#[cfg(test)]
#[path = "intent_tests.rs"]
mod tests;
