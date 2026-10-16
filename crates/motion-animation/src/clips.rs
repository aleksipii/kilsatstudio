//! Rule-based keyframe clips for the built-in seven-joint humanoid.
//!
//! Every clip is an additive translation offset over the rest pose, sampled
//! directly from scene-derived time, so seeking never depends on earlier frames.
//! Layer order: emotion posture (blended across changes), walk cycle while a
//! move is active, then at most one gesture weighted by its envelope.
use crate::{
    blend_pose, Easing, Keyframe, Track, Transform2, Vec2, CHEST, HEAD, HIP, LEFT_FOOT, LEFT_HAND,
    RIGHT_FOOT, RIGHT_HAND,
};
use motion_rig::{Joint, Rig};
use std::sync::OnceLock;

/// Seconds over which a new emotion's posture replaces the previous one.
pub const POSTURE_BLEND_SECONDS: f64 = 0.25;
/// Seconds per full walk cycle (two steps).
pub const WALK_CYCLE_SECONDS: f64 = 2.0 / 3.0;
/// MotionScript gesture names, in the order editors should present them.
pub const GESTURES: [&str; 4] = ["wave", "point", "nod", "shrug"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    Wave,
    Point,
    Nod,
    Shrug,
}
impl Gesture {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "wave" => Self::Wave,
            "point" => Self::Point,
            "nod" => Self::Nod,
            "shrug" => Self::Shrug,
            _ => return None,
        })
    }
    fn index(self) -> usize {
        self as usize
    }
}

/// Smooth in/out weight for normalized gesture progress; zero outside `[0, 1)`.
pub fn gesture_envelope(progress: f64) -> f64 {
    if progress.is_finite() && (0.0..1.0).contains(&progress) {
        (std::f64::consts::PI * progress).sin()
    } else {
        0.0
    }
}

/// Everything the scene rules selected for one actor at one instant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HumanoidState<'a> {
    /// Seconds since the active move began, or `None` when standing.
    pub walk_seconds: Option<f64>,
    /// Emotion before the latest change.
    pub previous_emotion: &'a str,
    /// Current emotion.
    pub emotion: &'a str,
    /// Seconds since the current emotion took effect.
    pub emotion_seconds: f64,
    /// Active gesture, its normalized progress and its weight in `0..=1`.
    pub gesture: Option<(Gesture, f64, f64)>,
}
impl Default for HumanoidState<'_> {
    fn default() -> Self {
        Self {
            walk_seconds: None,
            previous_emotion: "neutral",
            emotion: "neutral",
            emotion_seconds: f64::INFINITY,
            gesture: None,
        }
    }
}

struct Library {
    rig: Rig,
    walk: Vec<Option<Track>>,
    gestures: [Vec<Option<Track>>; 4],
}

fn offset(time: f64, x: f64, y: f64) -> Keyframe {
    Keyframe {
        time,
        value: Transform2 {
            translation: Vec2::new(x, y),
            ..Default::default()
        },
        easing: Easing::SmoothStep,
    }
}

/// Offset keys as (time, dx, dy).
type Keys<'a> = &'a [(f64, f64, f64)];

fn clip(entries: &[(usize, Keys)]) -> Vec<Option<Track>> {
    let mut tracks = vec![None; 7];
    for &(joint, keys) in entries {
        let keys = keys.iter().map(|&(t, x, y)| offset(t, x, y)).collect();
        tracks[joint] = Some(Track::new(keys).expect("built-in clip is valid"));
    }
    tracks
}

fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let positions = [
            (0.0, -10.0, None),
            (0.0, -40.0, Some(HIP)),
            (0.0, -40.0, Some(CHEST)),
            (-30.0, 30.0, Some(CHEST)),
            (30.0, 30.0, Some(CHEST)),
            (-20.0, 35.0, Some(HIP)),
            (20.0, 35.0, Some(HIP)),
        ];
        let rig = Rig::new(
            positions
                .iter()
                .enumerate()
                .map(|(i, &(x, y, parent))| Joint {
                    name: format!("joint_{i}"),
                    parent,
                    rest: Transform2 {
                        translation: Vec2::new(x, y),
                        ..Default::default()
                    },
                })
                .collect(),
        )
        .expect("built-in rig is valid");
        // Quarter-cycle keys: contact, left forward, contact, right forward, contact.
        let q = WALK_CYCLE_SECONDS / 4.0;
        let stride = |a: f64| -> [(f64, f64, f64); 5] {
            [
                (0.0, 0.0, 0.0),
                (q, a, 0.0),
                (2.0 * q, 0.0, 0.0),
                (3.0 * q, -a, 0.0),
                (4.0 * q, 0.0, 0.0),
            ]
        };
        let bob = [
            (0.0, 0.0, 0.0),
            (q, 0.0, -2.0),
            (2.0 * q, 0.0, 0.0),
            (3.0 * q, 0.0, -2.0),
            (4.0 * q, 0.0, 0.0),
        ];
        let walk = clip(&[
            (HIP, &bob),
            (LEFT_FOOT, &stride(13.0)),
            (RIGHT_FOOT, &stride(-13.0)),
            (LEFT_HAND, &stride(-7.0)),
            (RIGHT_HAND, &stride(7.0)),
        ]);
        let wave = clip(&[(
            RIGHT_HAND,
            &[
                (0.0, 0.0, -65.0),
                (0.25, 8.0, -63.0),
                (0.5, 0.0, -65.0),
                (0.75, 8.0, -63.0),
                (1.0, 0.0, -65.0),
            ],
        )]);
        let point = clip(&[(RIGHT_HAND, &[(0.0, 35.0, -45.0)])]);
        let nod = clip(&[(
            HEAD,
            &[
                (0.0, 0.0, 0.0),
                (0.25, 0.0, 7.0),
                (0.5, 0.0, 0.0),
                (0.75, 0.0, 7.0),
                (1.0, 0.0, 0.0),
            ],
        )]);
        let shrug = clip(&[
            (LEFT_HAND, &[(0.0, -10.0, -22.0)]),
            (RIGHT_HAND, &[(0.0, 10.0, -22.0)]),
            (HEAD, &[(0.0, 0.0, 4.0)]),
            (CHEST, &[(0.0, 0.0, -3.0)]),
        ]);
        Library {
            rig,
            walk,
            gestures: [wave, point, nod, shrug],
        }
    })
}

/// Fixed posture offsets per supported emotion; unknown values stand neutral.
fn posture(emotion: &str) -> [(usize, f64, f64); 4] {
    match emotion {
        "happy" => [
            (CHEST, 0.0, -3.0),
            (HEAD, 0.0, -2.0),
            (HIP, 0.0, 0.0),
            (HIP, 0.0, 0.0),
        ],
        "sad" => [
            (HEAD, 0.0, 6.0),
            (CHEST, 0.0, 3.0),
            (LEFT_HAND, 0.0, 4.0),
            (RIGHT_HAND, 0.0, 4.0),
        ],
        "surprised" => [
            (HEAD, 0.0, -4.0),
            (LEFT_HAND, -4.0, -12.0),
            (RIGHT_HAND, 4.0, -12.0),
            (HIP, 0.0, 0.0),
        ],
        "angry" => [
            (HEAD, 0.0, 2.0),
            (CHEST, 0.0, 2.0),
            (LEFT_HAND, 8.0, 0.0),
            (RIGHT_HAND, -8.0, 0.0),
        ],
        _ => [(HIP, 0.0, 0.0); 4],
    }
}

fn posture_pose(rig: &Rig, emotion: &str) -> Vec<Transform2> {
    let mut pose = rig.rest_pose();
    for (joint, x, y) in posture(emotion) {
        pose[joint].translation.x += x;
        pose[joint].translation.y += y;
    }
    pose
}

fn add(pose: &mut [Transform2], offsets: &[Option<Track>], time: f64, weight: f64) {
    for (local, track) in pose.iter_mut().zip(offsets) {
        if let Some(delta) = track.as_ref().and_then(|t| t.sample(time).ok()) {
            local.translation.x += delta.translation.x * weight;
            local.translation.y += delta.translation.y * weight;
        }
    }
}

/// Local skeleton points for the built-in humanoid. Nonfinite or out-of-range
/// inputs fall back to neutral layers instead of producing invalid geometry.
pub fn evaluate_humanoid(state: &HumanoidState) -> [Vec2; 7] {
    let lib = library();
    let progress = if state.emotion_seconds.is_finite() && state.emotion_seconds >= 0.0 {
        (state.emotion_seconds / POSTURE_BLEND_SECONDS).min(1.0)
    } else {
        1.0
    };
    let mut pose = blend_pose(
        &lib.rig,
        &posture_pose(&lib.rig, state.previous_emotion),
        &posture_pose(&lib.rig, state.emotion),
        progress,
    )
    .expect("built-in postures are bounded");
    if let Some(t) = state.walk_seconds.filter(|t| t.is_finite() && *t >= 0.0) {
        add(&mut pose, &lib.walk, t.rem_euclid(WALK_CYCLE_SECONDS), 1.0);
    }
    if let Some((gesture, progress, weight)) = state.gesture {
        if progress.is_finite() && weight.is_finite() && weight > 0.0 {
            add(
                &mut pose,
                &lib.gestures[gesture.index()],
                progress.clamp(0.0, 1.0),
                weight.min(1.0),
            );
        }
    }
    let world = lib.rig.evaluate(&pose).expect("bounded built-in pose");
    std::array::from_fn(|i| world[i].point(Vec2::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(gesture: Gesture, progress: f64) -> [Vec2; 7] {
        evaluate_humanoid(&HumanoidState {
            gesture: Some((gesture, progress, gesture_envelope(progress))),
            ..Default::default()
        })
    }

    #[test]
    fn gesture_names_round_trip_and_unknown_is_rejected() {
        for name in GESTURES {
            assert!(Gesture::parse(name).is_some(), "{name}");
        }
        assert_eq!(Gesture::parse("dance"), None);
        assert_eq!(Gesture::parse("Wave"), None);
    }

    #[test]
    fn envelope_is_zero_at_edges_and_full_at_midpoint() {
        assert_eq!(gesture_envelope(0.0), 0.0);
        assert_eq!(gesture_envelope(0.5), 1.0);
        for p in [1.0, 1.5, -0.1, f64::NAN, f64::INFINITY] {
            assert_eq!(gesture_envelope(p), 0.0, "{p}");
        }
    }

    #[test]
    fn each_gesture_moves_only_at_nonzero_weight_and_is_seekable() {
        let idle = evaluate_humanoid(&HumanoidState::default());
        for name in GESTURES {
            let gesture = Gesture::parse(name).unwrap();
            assert_eq!(at(gesture, 0.0), idle, "{name} starts at rest");
            assert_ne!(at(gesture, 0.4), idle, "{name} moves");
            assert_eq!(at(gesture, 0.4), at(gesture, 0.4), "{name} deterministic");
        }
        assert_eq!(at(Gesture::Wave, 0.5)[RIGHT_HAND].y, -85.0);
        assert!(at(Gesture::Nod, 0.25)[HEAD].y > idle[HEAD].y);
        let shrug = at(Gesture::Shrug, 0.5);
        assert!(shrug[LEFT_HAND].y < idle[LEFT_HAND].y && shrug[RIGHT_HAND].y < idle[RIGHT_HAND].y);
    }

    #[test]
    fn posture_blends_across_emotion_change() {
        let state = |seconds| HumanoidState {
            previous_emotion: "neutral",
            emotion: "sad",
            emotion_seconds: seconds,
            ..Default::default()
        };
        let neutral = evaluate_humanoid(&HumanoidState::default());
        let start = evaluate_humanoid(&state(0.0));
        let mid = evaluate_humanoid(&state(POSTURE_BLEND_SECONDS / 2.0));
        let done = evaluate_humanoid(&state(POSTURE_BLEND_SECONDS));
        assert_eq!(start, neutral);
        assert!(mid[HEAD].y > neutral[HEAD].y && mid[HEAD].y < done[HEAD].y);
        assert_eq!(done, evaluate_humanoid(&state(10.0)));
        assert_eq!(done[HEAD].y, neutral[HEAD].y + 9.0);
        assert_eq!(evaluate_humanoid(&state(f64::NAN)), done);
    }

    #[test]
    fn walk_cycle_repeats_and_combines_with_gesture() {
        let walk = |t| {
            evaluate_humanoid(&HumanoidState {
                walk_seconds: Some(t),
                ..Default::default()
            })
        };
        let idle = evaluate_humanoid(&HumanoidState::default());
        assert_eq!(walk(0.0), idle);
        assert_ne!(walk(WALK_CYCLE_SECONDS / 4.0), idle);
        let a = walk(0.1);
        let b = walk(0.1 + WALK_CYCLE_SECONDS * 3.0);
        for (a, b) in a.iter().zip(&b) {
            assert!((a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9);
        }
        let both = evaluate_humanoid(&HumanoidState {
            walk_seconds: Some(WALK_CYCLE_SECONDS / 4.0),
            gesture: Some((Gesture::Wave, 0.5, 1.0)),
            ..Default::default()
        });
        assert!(both[RIGHT_HAND].y < -80.0);
        assert_ne!(both[LEFT_FOOT], idle[LEFT_FOOT]);
        assert!(walk(f64::MAX)
            .iter()
            .all(|p| p.x.is_finite() && p.y.is_finite()));
    }
}
