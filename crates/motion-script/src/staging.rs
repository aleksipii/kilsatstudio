//! Staging hints for the animation principles.
//!
//! The checks only report; they never change a script, and none of them blocks
//! parsing, validation or rendering.
//!
//! * **Simultaneous actions** (`simultaneous_actions`). Two or more moves or
//!   gestures in one scene start on the same frame *and* end on the same frame.
//!   Parts that start and stop together read as mechanical; the hint suggests
//!   offsetting them by 2–4 frames, for example with `op: stagger, delay: 3f`.
//!   Frames are the scene-local times rounded to the project frame rate.
//! * **Anticipation shortened** (`anticipation_shortened`). A gesture of a
//!   rigged character asks for `anticipation_frames` that do not fit into 30 %
//!   of the gesture, so the rig layer shortens the anticipation.
//! * **Grounded arc** (`grounded_arc`). A move with a ground gait (anything but
//!   `leap`; `walk` when no gait is given) whose `arc` or `bezier` route strays
//!   more than [`GROUNDED_ARC_LIFT`] of the frame height from the straight
//!   line. The feet keep stepping while the body rises or sinks, which reads as
//!   stepping in the air. Use `gait: leap` for a jump, or a flatter route; a
//!   curve that stands for a path through depth may be kept.
use crate::{Event, Script};
use motion_animation::{MovePath, Vec2};
use std::collections::BTreeMap;

/// Smallest and largest stagger offset the hint recommends, in frames.
pub const STAGGER_HINT_FRAMES: (u32, u32) = (2, 4);

/// Largest distance of a ground gait's route from the straight line, as a
/// fraction of the frame height, before the grounded-arc hint.
pub const GROUNDED_ARC_LIFT: f64 = 0.03;

/// A staging hint kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StagingCheck {
    SimultaneousActions,
    AnticipationShortened,
    GroundedArc,
}

impl StagingCheck {
    /// Stable snake_case name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SimultaneousActions => "simultaneous_actions",
            Self::AnticipationShortened => "anticipation_shortened",
            Self::GroundedArc => "grounded_arc",
        }
    }
}

/// One action named in a hint: the actor and the event kind (`move` or
/// `gesture`) with its index in the scene's event list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedAction {
    pub actor: String,
    pub kind: &'static str,
    pub event_index: usize,
}

/// One staging hint, located in a scene.
#[derive(Debug, Clone, PartialEq)]
pub struct StagingNote {
    pub check: StagingCheck,
    pub scene_id: String,
    pub scene_index: usize,
    /// Scene-local start of the actions, in seconds.
    pub at: f64,
    /// Start and end frame at the project frame rate.
    pub start_frame: i64,
    pub end_frame: i64,
    pub actions: Vec<StagedAction>,
    pub message: String,
}

/// Staging hints for every scene, in scene order and then by start time.
pub fn check_staging(script: &Script) -> Vec<StagingNote> {
    let fps = script.project.fps.max(1);
    let frame = |seconds: f64| (seconds * fps as f64).round() as i64;
    let mut notes = Vec::new();
    for (scene_index, scene) in script.scenes.iter().enumerate() {
        let mut groups: BTreeMap<(i64, i64), Vec<(f64, StagedAction)>> = BTreeMap::new();
        for (event_index, event) in scene.events.iter().enumerate() {
            let (at, actor, duration, kind) = match event {
                Event::Move {
                    at,
                    actor,
                    duration,
                    ..
                } => (*at, actor, *duration, "move"),
                Event::Gesture {
                    at,
                    actor,
                    duration,
                    principles,
                    ..
                } => {
                    if let Some(note) = shortened_anticipation(
                        script,
                        scene_index,
                        event_index,
                        (*at, actor, *duration),
                        principles.as_ref(),
                    ) {
                        notes.push(note);
                    }
                    (*at, actor, *duration, "gesture")
                }
                _ => continue,
            };
            if !at.is_finite() || !duration.is_finite() || duration <= 0.0 {
                continue;
            }
            groups
                .entry((frame(at), frame(at + duration)))
                .or_default()
                .push((
                    at,
                    StagedAction {
                        actor: actor.clone(),
                        kind,
                        event_index,
                    },
                ));
        }
        for ((start_frame, end_frame), group) in groups {
            if group.len() < 2 {
                continue;
            }
            let at = group
                .iter()
                .map(|(at, _)| *at)
                .fold(f64::INFINITY, f64::min);
            let actions: Vec<_> = group.into_iter().map(|(_, action)| action).collect();
            let names = actions
                .iter()
                .map(|a| format!("{} {}", a.actor, a.kind))
                .collect::<Vec<_>>()
                .join(", ");
            let (low, high) = STAGGER_HINT_FRAMES;
            notes.push(StagingNote {
                check: StagingCheck::SimultaneousActions,
                scene_id: scene.id.clone(),
                scene_index,
                at,
                start_frame,
                end_frame,
                message: format!(
                    "{names} start on frame {start_frame} and stop on frame {end_frame} together; offset them by {low}–{high} frames (for example op: stagger, delay: {}f)",
                    (low + high) / 2
                ),
                actions,
            });
        }
    }
    notes.extend(grounded_arcs(script));
    notes.sort_by(|a, b| {
        a.scene_index
            .cmp(&b.scene_index)
            .then(a.at.total_cmp(&b.at))
            .then(a.check.cmp(&b.check))
    });
    notes
}

/// Largest distance of `path` from the straight line `from → to`, in pixels.
fn route_deviation(path: MovePath, from: Vec2, to: Vec2) -> f64 {
    let (cx, cy) = (to.x - from.x, to.y - from.y);
    let length = cx.hypot(cy);
    (1..32)
        .map(|i| {
            let p = path.point(from, to, i as f64 / 32.0);
            let (dx, dy) = (p.x - from.x, p.y - from.y);
            if length == 0.0 {
                dx.hypot(dy)
            } else {
                (dx * cy - dy * cx).abs() / length
            }
        })
        .fold(0.0, f64::max)
}

/// Moves with a ground gait on a route far from the straight line.
fn grounded_arcs(script: &Script) -> Vec<StagingNote> {
    let fps = script.project.fps.max(1);
    let frame = |seconds: f64| (seconds * fps as f64).round() as i64;
    let limit = GROUNDED_ARC_LIFT * script.project.height as f64;
    let (start_x, start_y) = script.project.actor_start();
    let mut notes = Vec::new();
    for (scene_index, scene) in script.scenes.iter().enumerate() {
        // Each actor's moves in time order: a move starts where the previous ended.
        let mut moves: Vec<_> = scene
            .events
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e, Event::Move { .. }))
            .collect();
        moves.sort_by(|a, b| {
            a.1.actor_time()
                .0
                .total_cmp(&b.1.actor_time().0)
                .then(a.0.cmp(&b.0))
        });
        let mut positions: BTreeMap<&str, Vec2> = BTreeMap::new();
        for (event_index, event) in moves {
            let Event::Move {
                at,
                actor,
                x,
                y,
                duration,
                gait,
                arc,
                bezier,
                ..
            } = event
            else {
                continue;
            };
            let to = Vec2::new(*x, *y);
            let from = positions
                .insert(actor, to)
                .unwrap_or(Vec2::new(start_x, start_y));
            let point = |p: &crate::PathPoint| Vec2::new(p.x, p.y);
            let path = match (arc, bezier.as_deref()) {
                (_, Some([c])) => MovePath::Quadratic(point(c)),
                (_, Some([c1, c2])) => MovePath::Cubic(point(c1), point(c2)),
                (Some(h), _) => MovePath::Arc(*h),
                _ => continue,
            };
            let gait = gait.as_deref().unwrap_or("walk");
            if gait == "leap" {
                continue;
            }
            let deviation = route_deviation(path, from, to);
            if deviation <= limit {
                continue;
            }
            notes.push(StagingNote {
                check: StagingCheck::GroundedArc,
                scene_id: scene.id.clone(),
                scene_index,
                at: *at,
                start_frame: frame(*at),
                end_frame: frame(at + duration),
                actions: vec![StagedAction {
                    actor: actor.clone(),
                    kind: "move",
                    event_index,
                }],
                message: format!(
                    "{actor} move: a {gait} on a route {deviation:.0} px from the straight line steps in the air; use gait: leap for a jump or keep the route within {limit:.0} px"
                ),
            });
        }
    }
    notes
}

fn shortened_anticipation(
    script: &Script,
    scene_index: usize,
    event_index: usize,
    (at, actor, duration): (f64, &String, f64),
    gesture: Option<&crate::PrinciplesSpec>,
) -> Option<StagingNote> {
    let character = script.characters.get(actor)?;
    character.rig.as_ref()?;
    let fps = script.project.fps;
    let (frames, fits) =
        crate::anticipation_fits(character.principles.as_ref(), gesture, duration, fps)?;
    if fits {
        return None;
    }
    let scene = &script.scenes[scene_index];
    let frame = |seconds: f64| (seconds * fps as f64).round() as i64;
    Some(StagingNote {
        check: StagingCheck::AnticipationShortened,
        scene_id: scene.id.clone(),
        scene_index,
        at,
        start_frame: frame(at),
        end_frame: frame(at + duration),
        actions: vec![StagedAction {
            actor: actor.clone(),
            kind: "gesture",
            event_index,
        }],
        message: format!(
            "{actor} gesture: anticipation_frames {frames} does not fit into 30 % of a {} frame gesture and is shortened; lengthen the gesture or lower anticipation_frames",
            frame(duration)
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(events: &str, characters: &str) -> Script {
        crate::parse(&format!(
            "project: {{title: T, fps: 24, width: 640, height: 360}}\ncharacters:\n{characters}scenes:\n  - id: s\n    duration: 10\n    events:\n{events}"
        ))
        .unwrap()
    }
    const TWO: &str = "  A: {asset: a}\n  B: {asset: b}\n";

    #[test]
    fn actions_that_start_and_stop_together_are_hinted() {
        let s = script(
            "      - {type: move, actor: A, at: 1, x: 10, y: 20, duration: 1}\n      - {type: move, actor: B, at: 1.01, x: 30, y: 20, duration: 1}\n      - {type: move, actor: A, at: 4, x: 10, y: 20, duration: 1}\n",
            TWO,
        );
        let notes = check_staging(&s);
        assert_eq!(notes.len(), 1, "{notes:?}");
        let note = &notes[0];
        assert_eq!(note.check, StagingCheck::SimultaneousActions);
        assert_eq!((note.start_frame, note.end_frame), (24, 48));
        assert_eq!(note.at, 1.0);
        assert_eq!(
            note.actions
                .iter()
                .map(|a| a.event_index)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert!(note.message.contains("2–4 frames"), "{}", note.message);
        assert!(note.message.contains("delay: 3f"), "{}", note.message);
    }

    #[test]
    fn staggered_or_differently_timed_actions_are_quiet() {
        for second in [
            // Three frames later.
            "{type: move, actor: B, at: 1.125, x: 30, y: 20, duration: 1}",
            // Same start, different end (overlapping action).
            "{type: move, actor: B, at: 1, x: 30, y: 20, duration: 1.25}",
        ] {
            let s = script(
                &format!("      - {{type: move, actor: A, at: 1, x: 10, y: 20, duration: 1}}\n      - {second}\n"),
                TWO,
            );
            assert!(check_staging(&s).is_empty(), "{second}");
        }
    }

    #[test]
    fn walks_far_off_the_straight_line_are_hinted_but_leaps_are_not() {
        let mv = |extra: &str| {
            script(
                &format!(
                    "      - {{type: move, actor: A, at: 0, x: 600, y: 252, duration: 1{extra}}}\n"
                ),
                "  A: {asset: a}\n",
            )
        };
        // From (320, 252) to (600, 252): 280 px; the limit is 3 % of 360 = 10.8 px.
        let high = check_staging(&mv(", arc: 0.2"));
        assert_eq!(high.len(), 1, "{high:?}");
        assert_eq!(high[0].check, StagingCheck::GroundedArc);
        assert!(high[0].message.contains("walk"), "{}", high[0].message);
        assert!(high[0].message.contains("56 px"), "{}", high[0].message);
        assert!(check_staging(&mv(", arc: 0.2, gait: leap")).is_empty());
        assert!(
            check_staging(&mv(", arc: 0.03")).is_empty(),
            "a low arc is fine"
        );
        assert!(check_staging(&mv("")).is_empty(), "straight");
        let curve = check_staging(&mv(", gait: run, bezier: [{x: 460, y: 100}]"));
        assert_eq!(curve.len(), 1);
        assert!(curve[0].message.contains("run"), "{}", curve[0].message);
    }

    #[test]
    fn grounded_arcs_start_where_the_previous_move_ended() {
        // The second move goes straight up from (600, 252) to (600, 100); a
        // small arc on that vertical chord bulges sideways by 0.2 * 152 px.
        let s = script(
            "      - {type: move, actor: A, at: 2, x: 600, y: 100, duration: 1, arc: 0.2}\n      - {type: move, actor: A, at: 0, x: 600, y: 252, duration: 1}\n",
            "  A: {asset: a}\n",
        );
        let notes = check_staging(&s);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].message.contains("30 px"), "{}", notes[0].message);
        assert_eq!(notes[0].actions[0].event_index, 0);
    }

    #[test]
    fn a_shortened_anticipation_is_hinted() {
        let rigged = "  A: {asset: a, principles: {anticipation_frames: 6}, rig: {version: 1, id: a, joints: [{id: root, parent: null}, {id: upper_arm_r, parent: root, x: 5}], parts: [{id: p, joint: upper_arm_r, points: [[0, 0], [2, 0], [0, 2]], color: [1, 2, 3, 255]}]}}\n";
        let script = crate::parse(&format!(
            "project: {{title: T, fps: 24, width: 640, height: 360}}\ncharacters:\n{rigged}scenes:\n  - id: s\n    duration: 10\n    events:\n      - {{type: gesture, actor: A, at: 0, value: wave, duration: 0.5}}\n      - {{type: gesture, actor: A, at: 2, value: wave, duration: 2}}\n"
        ))
        .unwrap();
        let notes = check_staging(&script);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert_eq!(notes[0].check, StagingCheck::AnticipationShortened);
        assert_eq!(notes[0].start_frame, 0);
        assert!(
            notes[0].message.contains("12 frame"),
            "{}",
            notes[0].message
        );
    }
}
