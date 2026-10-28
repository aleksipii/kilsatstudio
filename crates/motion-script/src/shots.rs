//! Shot library: named framing presets and dialogue-driven cut rules.
//!
//! A [`ShotList`] is declarative data. [`expand_shots`] is a pure function that
//! turns it into ordinary scenes (one scene per shot, joined by hard cuts) whose
//! compositions contain only existing structures: scene-local camera keys,
//! actor tracks and speak events. Nothing is evaluated at runtime, nothing is
//! random, and the result is validated like any other screenplay.
//!
//! Cut rules, all in seconds on the scene clock of the generated scene:
//! * a shot with a line starts `lead_in` seconds before the line, so the cut
//!   lands on the dialogue start (`lead_in` 0) or just ahead of it;
//! * it ends `tail` seconds after the line, then the next shot cuts in;
//! * a shot without a line, and an optional `reaction` shot following a line,
//!   hold for a bounded `hold` pause.
//!
//! Line durations are supplied by the author (the imported or placeholder audio
//! length); this module reads no files and synthesizes nothing.
use crate::{CameraKey, Composition, Error, Event, Scene, Script, Transition};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Maximum authored shots per list (each may add one reaction scene).
pub const MAX_SHOTS: usize = 64;
/// Maximum duration of one generated scene.
pub const MAX_SHOT_SECONDS: f64 = 120.0;
/// Maximum seconds a cut may precede its dialogue.
pub const MAX_LEAD_IN: f64 = 2.0;
/// Maximum pause between the end of a line and the next cut.
pub const MAX_TAIL: f64 = 5.0;
/// Maximum hold of a shot without a line, or of a reaction shot.
pub const MAX_HOLD: f64 = 10.0;
/// Maximum supplied dialogue duration of one line.
pub const MAX_LINE_SECONDS: f64 = 60.0;
/// Maximum extra authored events per shot.
pub const MAX_SHOT_EVENTS: usize = 64;
/// Fraction of the canvas width by which an over-the-shoulder framing shifts
/// the subject away from the centre (on screen, independent of zoom).
pub const OVER_THE_SHOULDER_SHIFT: f64 = 0.18;

/// Named framing. The zoom is relative to the full canvas (see [`Self::zoom`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShotPreset {
    /// Wide establishing shot, no zoom, static.
    Establishing,
    Medium,
    CloseUp,
    /// Subject off-centre so a foreground shoulder can fill the other side.
    OverTheShoulder,
    /// Tight, static framing held for a bounded pause.
    ReactionHold,
}

impl ShotPreset {
    /// Starting camera zoom.
    pub fn zoom(self) -> f64 {
        match self {
            Self::Establishing => 1.0,
            Self::Medium => 1.5,
            Self::CloseUp => 2.4,
            Self::OverTheShoulder => 1.8,
            Self::ReactionHold => 2.0,
        }
    }
    /// Default end/start zoom ratio (a slow push); `1.0` is static.
    pub fn default_push(self) -> f64 {
        match self {
            Self::Medium => 1.04,
            Self::CloseUp => 1.06,
            Self::Establishing | Self::OverTheShoulder | Self::ReactionHold => 1.0,
        }
    }
}

/// Which side of the frame the foreground shoulder occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShoulderSide {
    Left,
    Right,
}

/// A line of dialogue: the speak event plus the length of its audio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShotLine {
    pub actor: String,
    pub text: String,
    #[serde(default)]
    pub emotion: Option<String>,
    /// Imported audio reference stored on the speak event, if any.
    #[serde(default)]
    pub audio: Option<String>,
    /// Seconds of audio, imported or placeholder; drives the cut points.
    pub duration: f64,
}

/// A bounded reaction shot appended after the line's shot.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReactionShot {
    /// Scene id; defaults to `<shot id>_reaction`.
    pub id: Option<String>,
    pub actor: Option<String>,
    pub point: Option<[f64; 2]>,
    pub offset: [f64; 2],
    /// Pause in seconds; defaults to the cut rule `reaction_hold`.
    pub hold: Option<f64>,
    pub composition: Composition,
    /// Stage positions; when both `place` and `off_stage` are empty the reaction
    /// inherits the shot's stage.
    pub place: BTreeMap<String, [f64; 2]>,
    pub off_stage: Vec<String>,
}

/// Cut rules shared by every shot unless a shot overrides them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CutRules {
    /// Seconds the cut precedes the dialogue start.
    pub lead_in: f64,
    /// Seconds from the end of a line to the next cut.
    pub tail: f64,
    /// Default hold for shots without a line and for reaction shots.
    pub reaction_hold: f64,
}

impl Default for CutRules {
    fn default() -> Self {
        Self {
            lead_in: 0.0,
            tail: 0.5,
            reaction_hold: 1.5,
        }
    }
}

/// One shot. Exactly one of `actor` and `point` names the framing target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ShotSpec {
    /// Becomes the generated scene id.
    pub id: String,
    pub preset: ShotPreset,
    /// Frame this actor; the target follows the actor's `place` and `move` events.
    pub actor: Option<String>,
    /// Frame this canvas point instead (scene pixels).
    pub point: Option<[f64; 2]>,
    /// Extra offset from the target in scene pixels (for example up to the face).
    pub offset: [f64; 2],
    /// Required for `over_the_shoulder`, rejected for other presets.
    pub side: Option<ShoulderSide>,
    /// End/start zoom ratio, 0.5..=2; overrides the preset's default push.
    pub push: Option<f64>,
    pub line: Option<ShotLine>,
    pub lead_in: Option<f64>,
    /// Pause after the line, or the whole shot when there is no line.
    pub hold: Option<f64>,
    pub reaction: Option<ReactionShot>,
    /// Absolute canvas positions held for the whole shot.
    pub place: BTreeMap<String, [f64; 2]>,
    /// Actors moved far outside the canvas for the whole shot.
    pub off_stage: Vec<String>,
    /// Extra shot-local events (moves, emotions, gestures, more speech).
    pub events: Vec<Event>,
    /// Background, layers and other composition; `camera` must stay empty and
    /// the transition must be `cut`.
    pub composition: Composition,
}

impl Default for ShotSpec {
    fn default() -> Self {
        Self {
            id: String::new(),
            preset: ShotPreset::Medium,
            actor: None,
            point: None,
            offset: [0.0, 0.0],
            side: None,
            push: None,
            line: None,
            lead_in: None,
            hold: None,
            reaction: None,
            place: BTreeMap::new(),
            off_stage: vec![],
            events: vec![],
            composition: Composition::default(),
        }
    }
}

/// An ordered shot list with its cut rules.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ShotList {
    pub cut: CutRules,
    pub shots: Vec<ShotSpec>,
}

impl ShotList {
    /// Strict YAML decoding (unknown fields are syntax errors).
    pub fn from_yaml(source: &str) -> Result<Self, Error> {
        Ok(serde_yaml::from_str(source)?)
    }
}

/// Typed shot failure; converts to [`Error`] (`Unsupported` stays `Unsupported`).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ShotError {
    #[error("shot {shot}: unknown {kind} {name}")]
    UnknownReference {
        shot: String,
        kind: &'static str,
        name: String,
    },
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("shots conflict: {0}")]
    Conflict(String),
    #[error("shot bound exceeded: {0}")]
    Bound(String),
    #[error("shot {shot}: {message}")]
    Invalid { shot: String, message: String },
}

impl From<ShotError> for Error {
    fn from(error: ShotError) -> Self {
        match error {
            ShotError::Unsupported(message) => Error::Unsupported(message),
            other => Error::Validation(other.to_string()),
        }
    }
}

fn invalid(shot: &str, message: impl Into<String>) -> ShotError {
    ShotError::Invalid {
        shot: shot.into(),
        message: message.into(),
    }
}

/// Role of a generated scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedKind {
    /// Authored shot (with or without a line).
    Shot,
    /// Reaction shot generated from a [`ReactionShot`].
    Reaction,
}

/// Cut timing of one generated scene. Times are seconds on the sequence clock;
/// every transition is a hard cut, so `start` is the previous `end`.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedShot {
    pub scene_id: String,
    pub scene_index: usize,
    pub kind: PlannedKind,
    pub start: f64,
    pub end: f64,
    /// Absolute start of the dialogue, if the shot has a line.
    pub dialogue_start: Option<f64>,
    /// Absolute end of the dialogue, if the shot has a line.
    pub dialogue_end: Option<f64>,
}

/// Expanded screenplay plus the planned cut points.
#[derive(Debug, Clone)]
pub struct ShotExpansion {
    pub script: Script,
    pub plan: Vec<PlannedShot>,
}

fn finite_in(shot: &str, name: &str, value: f64, lo: f64, hi: f64) -> Result<f64, ShotError> {
    if value.is_finite() && value >= lo && value <= hi {
        Ok(value)
    } else {
        Err(invalid(
            shot,
            format!("{name} must be finite within {lo}..={hi}"),
        ))
    }
}

struct Stage<'a> {
    width: f64,
    height: f64,
    place: &'a BTreeMap<String, [f64; 2]>,
    events: &'a [Event],
}

impl Stage<'_> {
    /// Actor root position at `time`, mirroring the scene evaluator: default
    /// position, then completed/partial moves in time order, plus the held
    /// `place` offset (an actor track).
    fn position(&self, actor: &str, time: f64) -> (f64, f64) {
        let default = (self.width / 2.0, self.height * 0.7);
        let (mut x, mut y) = default;
        let mut moves: Vec<_> = self
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Move {
                    at,
                    actor: a,
                    x,
                    y,
                    duration,
                    ..
                } if a == actor && *at <= time => Some((*at, *x, *y, *duration)),
                _ => None,
            })
            .collect();
        moves.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (at, tx, ty, duration) in moves {
            let progress = ((time - at) / duration).clamp(0.0, 1.0);
            x += (tx - x) * progress;
            y += (ty - y) * progress;
        }
        if let Some([px, py]) = self.place.get(actor) {
            x += px - default.0;
            y += py - default.1;
        }
        (x, y)
    }
}

struct Framing<'a> {
    shot: &'a str,
    preset: ShotPreset,
    actor: Option<&'a str>,
    point: Option<[f64; 2]>,
    offset: [f64; 2],
    side: Option<ShoulderSide>,
    push: f64,
}

fn camera_keys(
    framing: &Framing,
    stage: &Stage,
    duration: f64,
) -> Result<Vec<CameraKey>, ShotError> {
    let mut times = vec![0.0, duration];
    if let Some(actor) = framing.actor {
        for event in stage.events {
            if let Event::Move {
                at,
                actor: a,
                duration: d,
                ..
            } = event
            {
                if a == actor {
                    times.extend([*at, at + d]);
                }
            }
        }
    }
    times.retain(|t| t.is_finite() && *t >= 0.0 && *t <= duration);
    times.sort_by(f64::total_cmp);
    times.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    let z0 = framing.preset.zoom();
    let z1 = z0 * framing.push;
    let (w, h) = (stage.width, stage.height);
    let key_at = |t: f64| {
        let zoom = z0 + (z1 - z0) * t / duration;
        let (mut cx, mut cy) = match (framing.actor, framing.point) {
            (Some(actor), _) => stage.position(actor, t),
            (None, Some([x, y])) => (x, y),
            (None, None) => (w / 2.0, h / 2.0),
        };
        cx += framing.offset[0];
        cy += framing.offset[1];
        if framing.preset == ShotPreset::OverTheShoulder {
            let shift = OVER_THE_SHOULDER_SHIFT * w / zoom;
            cx += match framing.side {
                Some(ShoulderSide::Left) => -shift,
                _ => shift,
            };
        }
        CameraKey {
            at: t,
            x: cx - w / 2.0,
            y: cy - h / 2.0,
            zoom,
        }
    };
    let mut keys: Vec<CameraKey> = times.into_iter().map(key_at).collect();
    // A static framing is one key; drop redundant trailing identical keys.
    if keys.windows(2).all(|pair| {
        pair[0].x == pair[1].x && pair[0].y == pair[1].y && pair[0].zoom == pair[1].zoom
    }) {
        keys.truncate(1);
    }
    for key in &keys {
        if !key.x.is_finite()
            || !key.y.is_finite()
            || key.x.abs() > 1_000_000.0
            || key.y.abs() > 1_000_000.0
            || !(0.05..=20.0).contains(&key.zoom)
        {
            return Err(invalid(
                framing.shot,
                "framing produced a camera key outside the supported range",
            ));
        }
    }
    Ok(keys)
}

fn check_target(
    shot: &str,
    script: &Script,
    actor: &Option<String>,
    point: &Option<[f64; 2]>,
    offset: [f64; 2],
    off_stage: &[String],
) -> Result<(), ShotError> {
    match (actor, point) {
        (Some(_), Some(_)) | (None, None) => {
            return Err(invalid(
                shot,
                "framing needs exactly one target: actor or point",
            ))
        }
        (Some(actor), None) => {
            if !script.characters.contains_key(actor) {
                return Err(ShotError::UnknownReference {
                    shot: shot.into(),
                    kind: "actor",
                    name: actor.clone(),
                });
            }
            if off_stage.contains(actor) {
                return Err(ShotError::Conflict(format!(
                    "shot {shot}: framing target {actor} is also off stage"
                )));
            }
        }
        (None, Some(p)) => {
            if p.iter().any(|v| !v.is_finite() || v.abs() > 100_000.0) {
                return Err(invalid(shot, "point must be finite within 100000 pixels"));
            }
        }
    }
    if offset.iter().any(|v| !v.is_finite() || v.abs() > 100_000.0) {
        return Err(invalid(shot, "offset must be finite within 100000 pixels"));
    }
    Ok(())
}

fn stage_tracks(
    shot: &str,
    script: &Script,
    place: &BTreeMap<String, [f64; 2]>,
    off_stage: &[String],
    composition: &mut Composition,
) -> Result<(), ShotError> {
    let (w, h) = (script.project.width as f64, script.project.height as f64);
    let mut add = |actor: &str, dx: f64, dy: f64| -> Result<(), ShotError> {
        if composition.actor_tracks.contains_key(actor) {
            return Err(ShotError::Conflict(format!(
                "shot {shot}: actor {actor} already has an authored actor track; \
                 remove it or drop place/off_stage"
            )));
        }
        composition.actor_tracks.insert(
            actor.to_string(),
            vec![crate::ActorKey {
                x: dx,
                y: dy,
                ..Default::default()
            }],
        );
        Ok(())
    };
    for (actor, [x, y]) in place {
        if !script.characters.contains_key(actor) {
            return Err(ShotError::UnknownReference {
                shot: shot.into(),
                kind: "actor",
                name: actor.clone(),
            });
        }
        if off_stage.contains(actor) {
            return Err(ShotError::Conflict(format!(
                "shot {shot}: actor {actor} is both placed and off stage"
            )));
        }
        if !x.is_finite() || !y.is_finite() || x.abs() > 100_000.0 || y.abs() > 100_000.0 {
            return Err(invalid(shot, "place must be finite within 100000 pixels"));
        }
        add(actor, x - w / 2.0, y - h * 0.7)?;
    }
    for actor in off_stage {
        if !script.characters.contains_key(actor) {
            return Err(ShotError::UnknownReference {
                shot: shot.into(),
                kind: "actor",
                name: actor.clone(),
            });
        }
        add(actor, -3.0 * w, 0.0)?;
    }
    Ok(())
}

fn check_composition(shot: &str, composition: &Composition) -> Result<(), ShotError> {
    if !composition.camera.is_empty() {
        return Err(ShotError::Conflict(format!(
            "shot {shot}: composition.camera must be empty; the preset generates the camera keys"
        )));
    }
    if composition.transition != Transition::Cut {
        return Err(ShotError::Unsupported(format!(
            "shot {shot}: only hard cuts between shots are supported (transition must be cut)"
        )));
    }
    Ok(())
}

/// Expand `list` into scenes appended after any scenes already in `base`.
///
/// The result is a complete, validated screenplay. Errors are typed; nothing is
/// clamped. See the module documentation for the cut rules.
pub fn expand_shots(base: &Script, list: &ShotList) -> Result<ShotExpansion, ShotError> {
    let rules = list.cut;
    finite_in("cut", "lead_in", rules.lead_in, 0.0, MAX_LEAD_IN)?;
    finite_in("cut", "tail", rules.tail, 0.0, MAX_TAIL)?;
    finite_in("cut", "reaction_hold", rules.reaction_hold, 0.0, MAX_HOLD)?;
    if rules.reaction_hold <= 0.0 {
        return Err(invalid("cut", "reaction_hold must be positive"));
    }
    if list.shots.is_empty() {
        return Err(ShotError::Invalid {
            shot: "list".into(),
            message: "at least one shot is required".into(),
        });
    }
    if list.shots.len() > MAX_SHOTS {
        return Err(ShotError::Bound(format!(
            "{} shots exceed the limit of {MAX_SHOTS}",
            list.shots.len()
        )));
    }
    let (w, h) = (base.project.width as f64, base.project.height as f64);
    let mut script = base.clone();
    let mut plan = Vec::new();
    let mut clock = base.scenes.iter().map(|s| s.duration).sum::<f64>();
    let mut ids: std::collections::BTreeSet<String> =
        base.scenes.iter().map(|s| s.id.clone()).collect();
    let mut claim = |id: &str| -> Result<(), ShotError> {
        if !ids.insert(id.to_string()) {
            return Err(ShotError::Conflict(format!("duplicate scene id {id}")));
        }
        Ok(())
    };
    for spec in &list.shots {
        let id = spec.id.as_str();
        if !crate::valid_clip_name(id) {
            return Err(invalid(
                id,
                "id must be 1..64 ASCII letters, digits, '_' or '-'",
            ));
        }
        claim(id)?;
        check_target(
            id,
            base,
            &spec.actor,
            &spec.point,
            spec.offset,
            &spec.off_stage,
        )?;
        check_composition(id, &spec.composition)?;
        if spec.events.len() > MAX_SHOT_EVENTS {
            return Err(ShotError::Bound(format!(
                "shot {id}: {} events exceed the limit of {MAX_SHOT_EVENTS}",
                spec.events.len()
            )));
        }
        match (spec.preset, spec.side) {
            (ShotPreset::OverTheShoulder, None) => {
                return Err(invalid(id, "over_the_shoulder requires side: left|right"))
            }
            (ShotPreset::OverTheShoulder, Some(_)) | (_, None) => {}
            (_, Some(_)) => return Err(invalid(id, "side applies only to over_the_shoulder")),
        }
        let push = match spec.push {
            Some(p) => finite_in(id, "push", p, 0.5, 2.0)?,
            None => spec.preset.default_push(),
        };
        let lead_in = finite_in(
            id,
            "lead_in",
            spec.lead_in.unwrap_or(rules.lead_in),
            0.0,
            MAX_LEAD_IN,
        )?;
        let mut events = spec.events.clone();
        let (duration, dialogue) = if let Some(line) = &spec.line {
            finite_in(id, "line.duration", line.duration, 0.0, MAX_LINE_SECONDS)?;
            if line.duration <= 0.0 {
                return Err(invalid(id, "line.duration must be positive"));
            }
            if !base.characters.contains_key(&line.actor) {
                return Err(ShotError::UnknownReference {
                    shot: id.into(),
                    kind: "actor",
                    name: line.actor.clone(),
                });
            }
            let tail = finite_in(
                id,
                "hold",
                spec.hold.unwrap_or(rules.tail),
                0.0,
                MAX_TAIL.max(MAX_HOLD),
            )?;
            events.insert(
                0,
                Event::Speak {
                    at: lead_in,
                    actor: line.actor.clone(),
                    text: line.text.clone(),
                    emotion: line.emotion.clone(),
                    audio: line.audio.clone(),
                },
            );
            (
                lead_in + line.duration + tail,
                Some((lead_in, lead_in + line.duration)),
            )
        } else {
            let hold = finite_in(
                id,
                "hold",
                spec.hold.unwrap_or(rules.reaction_hold),
                0.0,
                MAX_HOLD,
            )?;
            if hold <= 0.0 {
                return Err(invalid(id, "hold must be positive"));
            }
            (hold, None)
        };
        if duration > MAX_SHOT_SECONDS {
            return Err(ShotError::Bound(format!(
                "shot {id}: {duration} s exceeds the {MAX_SHOT_SECONDS} s limit"
            )));
        }
        events.sort_by(|a, b| a.actor_time().0.total_cmp(&b.actor_time().0));
        let mut composition = spec.composition.clone();
        stage_tracks(id, base, &spec.place, &spec.off_stage, &mut composition)?;
        let stage = Stage {
            width: w,
            height: h,
            place: &spec.place,
            events: &events,
        };
        composition.camera = camera_keys(
            &Framing {
                shot: id,
                preset: spec.preset,
                actor: spec.actor.as_deref(),
                point: spec.point,
                offset: spec.offset,
                side: spec.side,
                push,
            },
            &stage,
            duration,
        )?;
        plan.push(PlannedShot {
            scene_id: id.into(),
            scene_index: script.scenes.len(),
            kind: PlannedKind::Shot,
            start: clock,
            end: clock + duration,
            dialogue_start: dialogue.map(|(s, _)| clock + s),
            dialogue_end: dialogue.map(|(_, e)| clock + e),
        });
        script.scenes.push(Scene {
            composition,
            background: None,
            id: id.into(),
            duration,
            events,
        });
        clock += duration;

        if let Some(reaction) = &spec.reaction {
            if spec.line.is_none() {
                return Err(invalid(id, "a reaction shot needs a line to react to"));
            }
            let rid = reaction
                .id
                .clone()
                .unwrap_or_else(|| format!("{id}_reaction"));
            if !crate::valid_clip_name(&rid) {
                return Err(invalid(
                    &rid,
                    "id must be 1..64 ASCII letters, digits, '_' or '-'",
                ));
            }
            claim(&rid)?;
            check_composition(&rid, &reaction.composition)?;
            let hold = finite_in(
                &rid,
                "hold",
                reaction.hold.unwrap_or(rules.reaction_hold),
                0.0,
                MAX_HOLD,
            )?;
            if hold <= 0.0 {
                return Err(invalid(&rid, "hold must be positive"));
            }
            // The reaction stays in the same room: it inherits the shot's stage
            // unless it names its own.
            let (place, off_stage) = if reaction.place.is_empty() && reaction.off_stage.is_empty() {
                (&spec.place, &spec.off_stage)
            } else {
                (&reaction.place, &reaction.off_stage)
            };
            check_target(
                &rid,
                base,
                &reaction.actor,
                &reaction.point,
                reaction.offset,
                off_stage,
            )?;
            let mut composition = reaction.composition.clone();
            stage_tracks(&rid, base, place, off_stage, &mut composition)?;
            let stage = Stage {
                width: w,
                height: h,
                place,
                events: &[],
            };
            composition.camera = camera_keys(
                &Framing {
                    shot: &rid,
                    preset: ShotPreset::ReactionHold,
                    actor: reaction.actor.as_deref(),
                    point: reaction.point,
                    offset: reaction.offset,
                    side: None,
                    push: ShotPreset::ReactionHold.default_push(),
                },
                &stage,
                hold,
            )?;
            plan.push(PlannedShot {
                scene_id: rid.clone(),
                scene_index: script.scenes.len(),
                kind: PlannedKind::Reaction,
                start: clock,
                end: clock + hold,
                dialogue_start: None,
                dialogue_end: None,
            });
            script.scenes.push(Scene {
                composition,
                background: None,
                id: rid,
                duration: hold,
                events: vec![],
            });
            clock += hold;
        }
    }
    script
        .validate()
        .map_err(|e| ShotError::Conflict(format!("expanded screenplay rejected: {e}")))?;
    Ok(ShotExpansion { script, plan })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Script {
        crate::parse(
            "project: {title: t, fps: 24, width: 640, height: 360}\n\
             characters: {A: {asset: a}, B: {asset: b}}\n\
             scenes: [{id: pre, duration: 1, events: []}]",
        )
        .unwrap()
    }
    fn list(yaml: &str) -> ShotList {
        ShotList::from_yaml(yaml).unwrap()
    }
    const BASIC: &str = "
cut: {lead_in: 0.25, tail: 0.5, reaction_hold: 1.5}
shots:
  - id: wide
    preset: establishing
    point: [320, 180]
    hold: 2
  - id: close
    preset: close_up
    actor: A
    offset: [0, -50]
    line: {actor: A, text: Hello there, duration: 2, audio: a.wav}
    reaction: {actor: B, offset: [0, -40], hold: 1}
    place: {A: [400, 250], B: [200, 250]}
";

    #[test]
    fn cuts_land_on_dialogue_starts_and_reaction_holds_are_bounded() {
        let out = expand_shots(&base(), &list(BASIC)).unwrap();
        let p = &out.plan;
        assert_eq!(p.len(), 3);
        assert_eq!((p[0].start, p[0].end), (1.0, 3.0));
        assert_eq!(p[0].dialogue_start, None);
        // lead_in 0.25 + line 2 + tail 0.5
        assert_eq!((p[1].start, p[1].end), (3.0, 5.75));
        assert_eq!(p[1].dialogue_start, Some(3.25));
        assert_eq!(p[1].dialogue_end, Some(5.25));
        assert_eq!(p[2].kind, PlannedKind::Reaction);
        assert_eq!((p[2].start, p[2].end), (5.75, 6.75));
        assert_eq!(p[2].scene_id, "close_reaction");
        // The speak event sits at the lead-in on the shot-local clock.
        let scene = &out.script.scenes[2];
        assert!(
            matches!(&scene.events[0], Event::Speak {at, audio: Some(a), ..} if *at == 0.25 && a == "a.wav")
        );
        // The expansion is a plain screenplay that round-trips and re-validates.
        let reparsed = crate::parse(&crate::to_source(&out.script).unwrap()).unwrap();
        assert_eq!(reparsed.scenes.len(), 4);
    }

    #[test]
    fn framing_centres_target_with_offset_zoom_and_slow_push() {
        let out = expand_shots(&base(), &list(BASIC)).unwrap();
        let wide = &out.script.scenes[1].composition.camera;
        assert_eq!(
            wide,
            &[CameraKey {
                at: 0.0,
                x: 0.0,
                y: 0.0,
                zoom: 1.0
            }]
        );
        let close = &out.script.scenes[2].composition.camera;
        assert_eq!(close.len(), 2);
        // A placed at (400, 250), offset (0,-50): centre (400, 200).
        assert_eq!((close[0].x, close[0].y), (80.0, 20.0));
        assert_eq!(close[0].zoom, 2.4);
        assert!((close[1].zoom - 2.4 * 1.06).abs() < 1e-12);
        assert_eq!(close[1].at, 2.75);
        // Projecting the framed point lands on the canvas centre at any zoom.
        let p = |k: &CameraKey| {
            (
                320.0 + (400.0 - 320.0 - k.x) * k.zoom,
                180.0 + (200.0 - 180.0 - k.y) * k.zoom,
            )
        };
        assert_eq!(p(&close[0]), (320.0, 180.0));
        let reaction = &out.script.scenes[3].composition.camera;
        assert_eq!(reaction.len(), 1);
        assert_eq!(reaction[0].zoom, 2.0);
        assert_eq!(
            (reaction[0].x, reaction[0].y),
            (200.0 - 320.0, 250.0 - 40.0 - 180.0)
        );
    }

    #[test]
    fn camera_follows_a_moving_actor_through_each_move_boundary() {
        let l = list(
            "shots:\n  - id: walk\n    preset: medium\n    actor: A\n    push: 1\n    hold: 4\n    \
             events: [{type: move, at: 1, actor: A, x: 420, y: 252, duration: 2}]\n",
        );
        let cam = &expand_shots(&base(), &l).unwrap().script.scenes[1]
            .composition
            .camera;
        let times: Vec<f64> = cam.iter().map(|k| k.at).collect();
        assert_eq!(times, vec![0.0, 1.0, 3.0, 4.0]);
        assert_eq!(cam[0].x, 0.0);
        assert_eq!(cam[2].x, 100.0);
        assert_eq!(cam[3].x, 100.0);
    }

    #[test]
    fn over_the_shoulder_shifts_subject_by_a_fixed_screen_fraction() {
        let l = list(
            "shots:\n  - id: ots\n    preset: over_the_shoulder\n    side: left\n    actor: A\n    hold: 1\n",
        );
        let k = &expand_shots(&base(), &l).unwrap().script.scenes[1]
            .composition
            .camera[0];
        let screen_x = 320.0 + (320.0 - 320.0 - k.x) * k.zoom;
        assert!(
            (screen_x - (320.0 + 0.18 * 640.0)).abs() < 1e-9,
            "{screen_x}"
        );
        let right = list("shots:\n  - id: ots\n    preset: over_the_shoulder\n    side: right\n    actor: A\n    hold: 1\n");
        let k = &expand_shots(&base(), &right).unwrap().script.scenes[1]
            .composition
            .camera[0];
        assert!(k.x > 0.0);
    }

    #[test]
    fn off_stage_and_place_become_held_actor_tracks() {
        let l = list(
            "shots:\n  - id: s\n    preset: medium\n    actor: A\n    off_stage: [B]\n    place: {A: [100, 100]}\n    hold: 1\n",
        );
        let c = &expand_shots(&base(), &l).unwrap().script.scenes[1].composition;
        assert_eq!(c.actor_tracks["B"][0].x, -1920.0);
        assert_eq!(c.actor_tracks["A"][0].x, 100.0 - 320.0);
        assert_eq!(c.actor_tracks["A"][0].y, 100.0 - 360.0 * 0.7);
    }

    #[test]
    fn expansion_is_deterministic() {
        let a = expand_shots(&base(), &list(BASIC)).unwrap();
        let b = expand_shots(&base(), &list(BASIC)).unwrap();
        assert_eq!(
            crate::to_source(&a.script).unwrap(),
            crate::to_source(&b.script).unwrap()
        );
        assert_eq!(a.plan, b.plan);
    }

    fn err(yaml: &str) -> ShotError {
        expand_shots(&base(), &list(yaml)).unwrap_err()
    }

    #[test]
    fn typed_errors_for_every_class() {
        assert!(matches!(
            err("shots: [{id: s, actor: Z, hold: 1}]"),
            ShotError::UnknownReference { .. }
        ));
        assert!(matches!(
            err("shots: [{id: s, point: [0,0], composition: {transition: {type: crossfade, duration: 0.5}}, hold: 1}]"),
            ShotError::Unsupported(_)
        ));
        assert!(matches!(
            err("shots: [{id: s, point: [0,0], hold: 1, composition: {camera: [{at: 0, x: 0, y: 0, zoom: 1}]}}]"),
            ShotError::Conflict(_)
        ));
        assert!(matches!(
            err("shots: [{id: pre, point: [0,0], hold: 1}]"),
            ShotError::Conflict(_)
        ));
        assert!(matches!(
            err("shots: [{id: s, point: [0,0], hold: 1}, {id: s, point: [0,0], hold: 1}]"),
            ShotError::Conflict(_)
        ));
        assert!(matches!(
            err("shots: [{id: s, actor: A, off_stage: [A], hold: 1}]"),
            ShotError::Conflict(_)
        ));
        let many = format!(
            "shots: [{}]",
            (0..65)
                .map(|i| format!("{{id: s{i}, point: [0,0], hold: 1}}"))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert!(matches!(err(&many), ShotError::Bound(_)));
        assert!(matches!(
            err("shots: [{id: s, point: [0,0], line: {actor: A, text: x, duration: 100}}]"),
            ShotError::Invalid { .. }
        ));
    }

    #[test]
    fn invalid_numbers_and_shapes_are_rejected_not_clamped() {
        for bad in [
            "shots: [{id: s, point: [0,0], hold: 0}]",
            "shots: [{id: s, point: [0,0], hold: 11}]",
            "shots: [{id: s, point: [0,0], hold: .nan}]",
            "shots: [{id: s, point: [.inf,0], hold: 1}]",
            "shots: [{id: s, hold: 1}]",
            "shots: [{id: s, actor: A, point: [0,0], hold: 1}]",
            "shots: [{id: s, point: [0,0], hold: 1, push: 3}]",
            "shots: [{id: s, point: [0,0], hold: 1, lead_in: 3}]",
            "shots: [{id: s, point: [0,0], hold: 1, preset: over_the_shoulder}]",
            "shots: [{id: s, point: [0,0], hold: 1, side: left}]",
            "shots: [{id: 'bad id', point: [0,0], hold: 1}]",
            "shots: [{id: s, point: [0,0], hold: 1, reaction: {point: [0,0]}}]",
            "shots: [{id: s, point: [0,0], line: {actor: A, text: x, duration: 0}}]",
            "shots: []",
            "cut: {tail: 6}\nshots: [{id: s, point: [0,0], hold: 1}]",
        ] {
            assert!(
                matches!(
                    expand_shots(&base(), &list(bad)),
                    Err(ShotError::Invalid { .. })
                ),
                "{bad}"
            );
        }
        // Unknown fields are syntax errors.
        assert!(matches!(
            ShotList::from_yaml("shots: [{id: s, zoom: 2}]"),
            Err(Error::Syntax(_))
        ));
        assert!(matches!(
            Error::from(ShotError::Unsupported("x".into())),
            Error::Unsupported(_)
        ));
    }
}
