//! Film-grammar continuity checks for an expanded shot list.
//!
//! The checks only report; they never change a shot, a cut or a decision of
//! the automatic director. Every note names the cut by the incoming scene, so
//! an editor can point at the exact shot.
//!
//! * **Axis crossing** (`axis_cross`, warning). For every pair of characters
//!   with dialogue in the list, each shot is read for the pair's screen order
//!   at its first and last frame:
//!   - an over-the-shoulder shot of one of them puts the other on the shoulder
//!     side;
//!   - otherwise, when both are inside the frame, their root positions decide
//!     the order; within [`AXIS_NEUTRAL_TOLERANCE`] of the canvas width the
//!     shot is neutral (on the line);
//!   - a shot showing neither of them (a cutaway) is neutral;
//!   - a single of one of them carries no order and is skipped.
//!
//!   A cut that reverses the last known order warns unless a neutral shot came
//!   in between. A reversal that happens inside one shot (a character walks
//!   past the other on screen) is seen by the audience and is allowed; the
//!   order at the end of that shot becomes the reference.
//! * **Jump cut** (`jump_cut`, warning). Consecutive shots frame the same
//!   character with the same preset (and shoulder side), the zoom changes by
//!   less than [`JUMP_CUT_MAX_ZOOM_RATIO`] and the character moves on screen by
//!   less than [`JUMP_CUT_MAX_SHIFT`] of the canvas width.
//! * **Cut on action** (`cut_on_action`, hint). The cut lands while no move or
//!   gesture is in progress, and a move, gesture or pose change starts within
//!   [`CUT_ON_ACTION_WINDOW`] seconds of it. The hint suggests that start as
//!   the cut point; nothing is moved.
//!
//! A shot acknowledges notes on the cuts into it (and into its reaction shot)
//! with `continuity: {allow: [axis_cross, jump_cut, cut_on_action]}`.
//!
//! Limits: the stage is 2D, so "the camera crossing the line" shows up as the
//! characters' screen order flipping between shots. Eyelines of single shots
//! are not inferred from gaze tracks, because gaze points often target props.
use crate::shots::{PlannedKind, ShotExpansion, ShotList, ShotPreset, ShoulderSide, Stage};
use crate::{CameraKey, Event};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Fraction of the canvas width within which two characters count as aligned
/// on the axis (a neutral shot).
pub const AXIS_NEUTRAL_TOLERANCE: f64 = 0.02;
/// A jump cut moves the framed character less than this fraction of the
/// canvas width on screen.
pub const JUMP_CUT_MAX_SHIFT: f64 = 0.10;
/// A jump cut changes the zoom by less than this ratio.
pub const JUMP_CUT_MAX_ZOOM_RATIO: f64 = 1.25;
/// Seconds around a cut searched for an action start.
pub const CUT_ON_ACTION_WINDOW: f64 = 0.5;
const EPS: f64 = 1e-6;

/// A continuity rule; also the value written in `continuity.allow`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuityCheck {
    AxisCross,
    JumpCut,
    CutOnAction,
}

impl ContinuityCheck {
    pub fn severity(self) -> ContinuitySeverity {
        match self {
            Self::AxisCross | Self::JumpCut => ContinuitySeverity::Warning,
            Self::CutOnAction => ContinuitySeverity::Hint,
        }
    }
    /// The MotionScript name used in `continuity.allow`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AxisCross => "axis_cross",
            Self::JumpCut => "jump_cut",
            Self::CutOnAction => "cut_on_action",
        }
    }
}

/// How strongly a note should be shown. Neither level blocks an expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuitySeverity {
    /// Likely to confuse the viewer.
    Warning,
    /// A suggestion that may improve the cut.
    Hint,
}

/// Per-shot acknowledgements: `continuity: {allow: [axis_cross]}`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ContinuityRules {
    pub allow: Vec<ContinuityCheck>,
}

impl ContinuityRules {
    pub fn is_empty(&self) -> bool {
        self.allow.is_empty()
    }
    pub fn allows(&self, check: ContinuityCheck) -> bool {
        self.allow.contains(&check)
    }
}

/// One finding, located at the cut into `scene_id`.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinuityNote {
    pub check: ContinuityCheck,
    pub severity: ContinuitySeverity,
    /// Authored shot id that owns the incoming scene (the parent shot for a
    /// reaction scene); its `continuity.allow` acknowledges the note.
    pub shot: String,
    /// Incoming generated scene and its index in the expanded screenplay.
    pub scene_id: String,
    pub scene_index: usize,
    /// Outgoing generated scene.
    pub previous_scene_id: String,
    /// Cut time in seconds on the sequence clock.
    pub at: f64,
    /// Suggested cut time (sequence clock) for `cut_on_action` hints.
    pub suggested_cut: Option<f64>,
    pub message: String,
}

/// One generated scene with what the checks need to read it.
struct Entry<'a> {
    shot: &'a str,
    scene_id: &'a str,
    scene_index: usize,
    start: f64,
    duration: f64,
    preset: ShotPreset,
    actor: Option<&'a str>,
    side: Option<ShoulderSide>,
    place: &'a BTreeMap<String, [f64; 2]>,
    off_stage: &'a [String],
    events: &'a [Event],
    camera: &'a [CameraKey],
    allow: &'a ContinuityRules,
}

fn entries<'a>(list: &'a ShotList, expansion: &'a ShotExpansion) -> Vec<Entry<'a>> {
    let mut plan = expansion.plan.iter();
    let mut out = Vec::new();
    for spec in &list.shots {
        let Some(planned) = plan.next() else { break };
        let Some(scene) = expansion.script.scenes.get(planned.scene_index) else {
            break;
        };
        out.push(Entry {
            shot: &spec.id,
            scene_id: &planned.scene_id,
            scene_index: planned.scene_index,
            start: planned.start,
            duration: planned.end - planned.start,
            preset: spec.preset,
            actor: spec.actor.as_deref(),
            side: spec.side,
            place: &spec.place,
            off_stage: &spec.off_stage,
            events: &scene.events,
            camera: &scene.composition.camera,
            allow: &spec.continuity,
        });
        if let Some(reaction) = &spec.reaction {
            let Some(planned) = plan.next() else { break };
            if planned.kind != PlannedKind::Reaction {
                break;
            }
            let Some(scene) = expansion.script.scenes.get(planned.scene_index) else {
                break;
            };
            // Same inheritance rule as `expand_shots`.
            let (place, off_stage) = if reaction.place.is_empty() && reaction.off_stage.is_empty() {
                (&spec.place, spec.off_stage.as_slice())
            } else {
                (&reaction.place, reaction.off_stage.as_slice())
            };
            out.push(Entry {
                shot: &spec.id,
                scene_id: &planned.scene_id,
                scene_index: planned.scene_index,
                start: planned.start,
                duration: planned.end - planned.start,
                preset: ShotPreset::ReactionHold,
                actor: reaction.actor.as_deref(),
                side: None,
                place,
                off_stage,
                events: &scene.events,
                camera: &scene.composition.camera,
                allow: &spec.continuity,
            });
        }
    }
    out
}

struct Canvas {
    width: f64,
    height: f64,
}

impl Canvas {
    fn position(&self, entry: &Entry, actor: &str, time: f64) -> (f64, f64) {
        Stage {
            width: self.width,
            height: self.height,
            place: entry.place,
            events: entry.events,
        }
        .position(actor, time)
    }

    /// Camera at the first (`end == false`) or last frame of the shot.
    fn camera(entry: &Entry, end: bool) -> CameraKey {
        let key = if end {
            entry.camera.last()
        } else {
            entry.camera.first()
        };
        key.cloned().unwrap_or(CameraKey {
            at: 0.0,
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        })
    }

    /// Screen position of a scene point under a camera key.
    fn screen(&self, key: &CameraKey, (x, y): (f64, f64)) -> (f64, f64) {
        let (cx, cy) = (self.width / 2.0, self.height / 2.0);
        (
            cx + (x - cx - key.x) * key.zoom,
            cy + (y - cy - key.y) * key.zoom,
        )
    }

    fn visible(&self, entry: &Entry, actor: &str, end: bool) -> bool {
        if entry.off_stage.iter().any(|a| a == actor) {
            return false;
        }
        let time = if end { entry.duration } else { 0.0 };
        let key = Self::camera(entry, end);
        let (sx, _) = self.screen(&key, self.position(entry, actor, time));
        (0.0..=self.width).contains(&sx)
    }
}

/// Screen order of a pair `(a, b)` in one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Order {
    ALeft,
    ARight,
    Neutral,
    Unknown,
}

fn order(canvas: &Canvas, entry: &Entry, a: &str, b: &str, end: bool) -> Order {
    if let (ShotPreset::OverTheShoulder, Some(framed), Some(side)) =
        (entry.preset, entry.actor, entry.side)
    {
        if framed == a || framed == b {
            // The partner's shoulder fills `side`; the framed actor is opposite.
            let framed_left = side == ShoulderSide::Right;
            let a_left = if framed == a {
                framed_left
            } else {
                !framed_left
            };
            return if a_left { Order::ALeft } else { Order::ARight };
        }
    }
    match (canvas.visible(entry, a, end), canvas.visible(entry, b, end)) {
        (true, true) => {
            let time = if end { entry.duration } else { 0.0 };
            let dx = canvas.position(entry, a, time).0 - canvas.position(entry, b, time).0;
            if dx.abs() <= AXIS_NEUTRAL_TOLERANCE * canvas.width {
                Order::Neutral
            } else if dx < 0.0 {
                Order::ALeft
            } else {
                Order::ARight
            }
        }
        (false, false) => Order::Neutral,
        _ => Order::Unknown,
    }
}

fn note(
    check: ContinuityCheck,
    prev: &Entry,
    next: &Entry,
    suggested_cut: Option<f64>,
    message: String,
) -> ContinuityNote {
    ContinuityNote {
        check,
        severity: check.severity(),
        shot: next.shot.into(),
        scene_id: next.scene_id.into(),
        scene_index: next.scene_index,
        previous_scene_id: prev.scene_id.into(),
        at: next.start,
        suggested_cut,
        message,
    }
}

fn axis_notes(canvas: &Canvas, list: &ShotList, shots: &[Entry]) -> Vec<(usize, ContinuityNote)> {
    let mut speakers: Vec<&str> = Vec::new();
    for spec in &list.shots {
        if let Some(line) = &spec.line {
            if !speakers.contains(&line.actor.as_str()) {
                speakers.push(&line.actor);
            }
        }
    }
    let mut notes = Vec::new();
    for (i, a) in speakers.iter().enumerate() {
        for b in &speakers[i + 1..] {
            // Last known order and the scene index where it was seen.
            let mut last: Option<(Order, usize)> = None;
            let mut bridged = false;
            for (index, entry) in shots.iter().enumerate() {
                for end in [false, true] {
                    match order(canvas, entry, a, b, end) {
                        Order::Neutral => bridged = true,
                        Order::Unknown => {}
                        seen => {
                            if let (false, Some((before, at)), false) = (end, last, bridged) {
                                if before != seen && index > 0 {
                                    let side = |o| if o == Order::ALeft { "left" } else { "right" };
                                    notes.push((
                                        index,
                                        note(
                                            ContinuityCheck::AxisCross,
                                            &shots[index - 1],
                                            entry,
                                            None,
                                            format!(
                                                "cut into {} crosses the line between {a} and {b}: \
                                                 {a} was {} of {b} in {}, now {}; add a neutral shot, \
                                                 show the move on screen, or acknowledge with \
                                                 continuity: {{allow: [axis_cross]}}",
                                                entry.scene_id,
                                                side(before),
                                                shots[at].scene_id,
                                                side(seen),
                                            ),
                                        ),
                                    ));
                                }
                            }
                            last = Some((seen, index));
                            bridged = false;
                        }
                    }
                }
            }
        }
    }
    notes
}

fn jump_cut(canvas: &Canvas, prev: &Entry, next: &Entry) -> Option<ContinuityNote> {
    let actor = prev.actor.filter(|a| Some(*a) == next.actor)?;
    if prev.preset != next.preset || prev.side != next.side {
        return None;
    }
    let (kp, kn) = (Canvas::camera(prev, true), Canvas::camera(next, false));
    let ratio = kp.zoom.max(kn.zoom) / kp.zoom.min(kn.zoom);
    let sp = canvas.screen(&kp, canvas.position(prev, actor, prev.duration));
    let sn = canvas.screen(&kn, canvas.position(next, actor, 0.0));
    let shift = (sp.0 - sn.0).hypot(sp.1 - sn.1) / canvas.width;
    if ratio >= JUMP_CUT_MAX_ZOOM_RATIO || shift >= JUMP_CUT_MAX_SHIFT {
        return None;
    }
    Some(note(
        ContinuityCheck::JumpCut,
        prev,
        next,
        None,
        format!(
            "cut from {} to {} is a jump cut: same character {actor}, same {:?} framing, \
             {:.0}% screen shift and {ratio:.2}x zoom change; change the shot size or angle, \
             insert another shot, or acknowledge with continuity: {{allow: [jump_cut]}}",
            prev.scene_id,
            next.scene_id,
            prev.preset,
            shift * 100.0,
        ),
    ))
}

/// Actions `(start, end, actor, kind)` on the sequence clock.
fn actions<'a>(entry: &'a Entry) -> impl Iterator<Item = (f64, f64, &'a str, &'static str)> + 'a {
    entry.events.iter().filter_map(move |event| {
        let start = entry.start;
        match event {
            Event::Move {
                at,
                actor,
                duration,
                ..
            } => Some((start + at, start + at + duration, actor.as_str(), "move")),
            Event::Gesture {
                at,
                actor,
                duration,
                ..
            } => Some((start + at, start + at + duration, actor.as_str(), "gesture")),
            Event::Pose { at, actor, .. } => Some((start + at, start + at, actor.as_str(), "pose")),
            _ => None,
        }
    })
}

fn cut_on_action(prev: &Entry, next: &Entry) -> Option<ContinuityNote> {
    let cut = next.start;
    let mut nearest: Option<(f64, &str, &str)> = None;
    for (start, end, actor, kind) in actions(prev).chain(actions(next)) {
        if (start - cut).abs() <= EPS || (start < cut - EPS && end >= cut - EPS) {
            // The cut lands on an action start, or the outgoing action runs
            // up to the cut (a matched-action edit).
            return None;
        }
        let distance = (start - cut).abs();
        if distance <= CUT_ON_ACTION_WINDOW + EPS
            && nearest.is_none_or(|(s, _, _)| {
                distance < (s - cut).abs() - EPS
                    || ((distance - (s - cut).abs()).abs() <= EPS && start < s)
            })
        {
            nearest = Some((start, actor, kind));
        }
    }
    let (start, actor, kind) = nearest?;
    Some(note(
        ContinuityCheck::CutOnAction,
        prev,
        next,
        Some(start),
        format!(
            "cut into {} at {cut:.2} s lands on a still moment; {actor}'s {kind} starts at \
             {start:.2} s ({:+.2} s): cutting on that action would hide the cut",
            next.scene_id,
            start - cut,
        ),
    ))
}

/// Run every continuity check over an expansion of `list`.
///
/// Notes are ordered by cut, then by check. Notes acknowledged by the
/// incoming shot's `continuity.allow` are left out.
pub fn check_continuity(list: &ShotList, expansion: &ShotExpansion) -> Vec<ContinuityNote> {
    let canvas = Canvas {
        width: expansion.script.project.width as f64,
        height: expansion.script.project.height as f64,
    };
    let shots = entries(list, expansion);
    let mut notes = axis_notes(&canvas, list, &shots);
    for (index, pair) in shots.windows(2).enumerate() {
        let (prev, next) = (&pair[0], &pair[1]);
        notes.extend(jump_cut(&canvas, prev, next).map(|n| (index + 1, n)));
        notes.extend(cut_on_action(prev, next).map(|n| (index + 1, n)));
    }
    notes.sort_by_key(|(index, n)| (*index, n.check));
    notes
        .into_iter()
        .filter(|(index, n)| !shots[*index].allow.allows(n.check))
        .map(|(_, n)| n)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{expand_shots, Error, Script};

    fn base() -> Script {
        crate::parse(
            "project: {title: t, fps: 24, width: 640, height: 360}\n\
             characters: {A: {asset: a}, B: {asset: b}, C: {asset: c}}\n\
             scenes: []",
        )
        .unwrap()
    }

    fn notes(yaml: &str) -> Vec<ContinuityNote> {
        let list = ShotList::from_yaml(yaml).unwrap();
        expand_shots(&base(), &list).unwrap().continuity
    }

    fn checks(yaml: &str) -> Vec<(ContinuityCheck, String)> {
        notes(yaml)
            .into_iter()
            .map(|n| (n.check, n.scene_id))
            .collect()
    }

    // A left of B, then B left of A: the cut crosses the line.
    const CROSS: &str = "
shots:
  - id: two_a
    preset: establishing
    point: [320, 180]
    place: {A: [200, 250], B: [440, 250]}
    line: {actor: A, text: Hi, duration: 1, audio: l.wav}
  - id: two_b
    preset: establishing
    point: [320, 180]
    place: {A: [440, 250], B: [200, 250]}
    line: {actor: B, text: Hello, duration: 1, audio: l.wav}
";

    #[test]
    fn axis_cross_warns_at_the_incoming_shot() {
        let n = notes(CROSS);
        assert_eq!(n.len(), 1, "{n:?}");
        let n = &n[0];
        assert_eq!(n.check, ContinuityCheck::AxisCross);
        assert_eq!(n.severity, ContinuitySeverity::Warning);
        assert_eq!((n.shot.as_str(), n.scene_id.as_str()), ("two_b", "two_b"));
        assert_eq!(n.previous_scene_id, "two_a");
        assert_eq!(n.scene_index, 1);
        assert_eq!(n.at, 1.5);
        assert!(n.message.contains("A was left of B"), "{}", n.message);
    }

    #[test]
    fn consistent_staging_and_over_the_shoulder_pairs_do_not_warn() {
        // A stays left of B; the over-the-shoulder shots keep the same sides:
        // on A with B's shoulder right, on B with A's shoulder left.
        let yaml = "
shots:
  - id: two
    preset: establishing
    point: [320, 180]
    place: {A: [200, 250], B: [440, 250]}
    line: {actor: A, text: Hi, duration: 1, audio: l.wav}
  - id: ots_a
    preset: over_the_shoulder
    side: right
    actor: A
    line: {actor: A, text: Hm, duration: 1, audio: l.wav}
  - id: ots_b
    preset: over_the_shoulder
    side: left
    actor: B
    line: {actor: B, text: Yes, duration: 1, audio: l.wav}
";
        assert_eq!(checks(yaml), vec![]);
        // Flipping one over-the-shoulder side crosses the line.
        let flipped = yaml.replace("side: left", "side: right");
        assert_eq!(
            checks(&flipped),
            vec![(ContinuityCheck::AxisCross, "ots_b".to_string())]
        );
    }

    #[test]
    fn crossing_shown_by_a_move_on_screen_is_allowed() {
        // A starts left of B (default x 320 vs 420) and walks past B to x 520
        // inside the first shot, so the reversed order later was seen on screen.
        let yaml = "
shots:
  - id: walk
    preset: establishing
    point: [320, 180]
    place: {B: [420, 252]}
    hold: 3
    events: [{type: move, at: 0.5, actor: A, x: 520, y: 252, duration: 2}]
  - id: after
    preset: establishing
    point: [320, 180]
    place: {A: [520, 252], B: [420, 252]}
    line: {actor: A, text: Done, duration: 1, audio: l.wav}
  - id: reply
    preset: establishing
    point: [320, 180]
    place: {A: [520, 252], B: [420, 252]}
    line: {actor: B, text: Ok, duration: 1, audio: l.wav}
";
        assert_eq!(checks(yaml), vec![]);
        // Without the move the same cut crosses the line.
        let jumped = yaml.replace(
            "    events: [{type: move, at: 0.5, actor: A, x: 520, y: 252, duration: 2}]\n",
            "",
        );
        assert_eq!(
            checks(&jumped),
            vec![(ContinuityCheck::AxisCross, "after".to_string())]
        );
    }

    #[test]
    fn neutral_cutaway_bridges_a_crossing() {
        let bridged = CROSS.replace(
            "  - id: two_b",
            "  - id: cutaway\n    preset: close_up\n    actor: C\n    off_stage: [A, B]\n    hold: 1\n  - id: two_b",
        );
        assert_eq!(checks(&bridged), vec![]);
        // A single of A carries no order and does not bridge.
        let single = CROSS.replace(
            "  - id: two_b",
            "  - id: single\n    preset: close_up\n    actor: A\n    off_stage: [B]\n    hold: 1\n  - id: two_b",
        );
        assert_eq!(
            checks(&single),
            vec![(ContinuityCheck::AxisCross, "two_b".to_string())]
        );
    }

    #[test]
    fn acknowledged_axis_cross_is_left_out() {
        let yaml = format!("{CROSS}    continuity: {{allow: [axis_cross]}}\n");
        assert_eq!(checks(&yaml), vec![]);
        // Acknowledging another check does not hide this one.
        let other = format!("{CROSS}    continuity: {{allow: [jump_cut]}}\n");
        assert_eq!(checks(&other).len(), 1);
        // Unknown names and keys are syntax errors.
        for bad in [
            format!("{CROSS}    continuity: {{allow: [crossing]}}\n"),
            format!("{CROSS}    continuity: {{ignore: [axis_cross]}}\n"),
        ] {
            assert!(matches!(ShotList::from_yaml(&bad), Err(Error::Syntax(_))));
        }
    }

    const JUMP: &str = "
shots:
  - id: a1
    preset: medium
    actor: A
    push: 1
    line: {actor: A, text: One, duration: 1, audio: l.wav}
  - id: a2
    preset: medium
    actor: A
    push: 1
    offset: [10, 0]
    line: {actor: A, text: Two, duration: 1, audio: l.wav}
";

    #[test]
    fn same_character_same_size_small_change_is_a_jump_cut() {
        let n = notes(JUMP);
        assert_eq!(n.len(), 1, "{n:?}");
        assert_eq!(n[0].check, ContinuityCheck::JumpCut);
        assert_eq!(n[0].severity, ContinuitySeverity::Warning);
        assert_eq!(n[0].scene_id, "a2");
        // A different shot size, or a different character, is a proper cut.
        assert_eq!(
            checks(&JUMP.replacen("preset: medium", "preset: close_up", 1)),
            vec![]
        );
        assert_eq!(
            checks(&JUMP.replace(
                "actor: A\n    push: 1\n    offset",
                "actor: B\n    push: 1\n    offset"
            )),
            vec![]
        );
        // A large reframing of the same size is not a jump cut.
        assert_eq!(
            checks(&JUMP.replace("offset: [10, 0]", "offset: [150, 0]")),
            vec![]
        );
        let ok = format!("{JUMP}    continuity: {{allow: [jump_cut]}}\n");
        assert_eq!(checks(&ok), vec![]);
    }

    #[test]
    fn cut_on_action_hint_suggests_a_nearby_action_start() {
        let yaml = "
shots:
  - id: still
    preset: establishing
    point: [320, 180]
    hold: 2
  - id: wave
    preset: medium
    actor: A
    hold: 2
    events: [{type: gesture, at: 0.3, actor: A, value: wave, duration: 1}]
";
        let n = notes(yaml);
        assert_eq!(n.len(), 1, "{n:?}");
        assert_eq!(n[0].check, ContinuityCheck::CutOnAction);
        assert_eq!(n[0].severity, ContinuitySeverity::Hint);
        assert_eq!(n[0].at, 2.0);
        assert_eq!(n[0].suggested_cut, Some(2.3));
        // An action that starts on the cut, or is in progress across it, is
        // already a cut on action; one far from the cut is not suggested.
        assert_eq!(checks(&yaml.replace("at: 0.3", "at: 0")), vec![]);
        assert_eq!(checks(&yaml.replace("at: 0.3", "at: 0.9")), vec![]);
        let spanning = yaml.replace(
            "    hold: 2\n  - id: wave",
            "    hold: 2\n    events: [{type: move, at: 1.0, actor: B, x: 100, y: 252, duration: 1}]\n  - id: wave",
        );
        assert_eq!(checks(&spanning), vec![]);
        let ok = yaml.replace(
            "    hold: 2\n    events",
            "    hold: 2\n    continuity: {allow: [cut_on_action]}\n    events",
        );
        assert_eq!(checks(&ok), vec![]);
    }

    #[test]
    fn notes_are_deterministic_and_located_on_reaction_scenes() {
        let yaml = "
shots:
  - id: a
    preset: reaction_hold
    actor: B
    hold: 1
  - id: b
    preset: close_up
    actor: A
    line: {actor: A, text: Hi, duration: 1, audio: l.wav}
    reaction: {actor: A, hold: 1}
";
        // The reaction (reaction_hold of A) follows a close-up of A: different
        // sizes, no jump cut. Reaction scenes are located by their own id.
        let first = notes(yaml);
        assert_eq!(first, notes(yaml));
        let same = yaml.replace("preset: close_up", "preset: reaction_hold");
        let n = notes(&same);
        assert_eq!(n.len(), 1, "{n:?}");
        assert_eq!(n[0].check, ContinuityCheck::JumpCut);
        assert_eq!(
            (n[0].shot.as_str(), n[0].scene_id.as_str()),
            ("b", "b_reaction")
        );
    }

    #[test]
    fn continuity_key_round_trips_and_is_omitted_when_empty() {
        let list =
            ShotList::from_yaml(&format!("{CROSS}    continuity: {{allow: [axis_cross]}}\n"))
                .unwrap();
        let text = serde_yaml::to_string(&list).unwrap();
        assert_eq!(text.matches("continuity").count(), 1, "{text}");
        assert_eq!(ShotList::from_yaml(&text).unwrap(), list);
    }
}
