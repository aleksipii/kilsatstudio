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

/// Artist-controlled intensity of the procedural layers; authored offsets are independent.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClipWeights {
    pub posture: f64,
    pub walk: f64,
    pub gesture: f64,
}
impl Default for ClipWeights {
    fn default() -> Self {
        Self {
            posture: 1.0,
            walk: 1.0,
            gesture: 1.0,
        }
    }
}
impl ClipWeights {
    pub fn is_valid(self) -> bool {
        [self.posture, self.walk, self.gesture]
            .into_iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(&v))
    }
}

/// Stable parent-before-child names for the original procedural rig.
pub const HUMANOID_BONES: [&str; 7] = [
    "hip",
    "chest",
    "head",
    "left_hand",
    "right_hand",
    "left_foot",
    "right_foot",
];

/// Conservative limits keep every authored hierarchy inside rig world bounds.
pub fn valid_humanoid_offset(value: Transform2) -> bool {
    value.is_valid()
        && value.translation.x.abs() <= 1000.0
        && value.translation.y.abs() <= 1000.0
        && value.rotation.abs() <= 1000.0
        && (0.1..=4.0).contains(&value.scale.x)
        && (0.1..=4.0).contains(&value.scale.y)
}

/// Seconds over which a new emotion's posture replaces the previous one.
pub const POSTURE_BLEND_SECONDS: f64 = 0.25;
/// Seconds per full walk cycle (two steps).
pub const WALK_CYCLE_SECONDS: f64 = 2.0 / 3.0;
/// Seconds per full jog cycle (two steps).
pub const JOG_CYCLE_SECONDS: f64 = 0.5;
/// Seconds per full run cycle (two steps).
pub const RUN_CYCLE_SECONDS: f64 = 0.4;
/// MotionScript gait names, in the order editors should present them.
pub const GAITS: [&str; 10] = [
    "walk",
    "jog",
    "run",
    "stroll",
    "sneak",
    "tiptoe",
    "limp",
    "sprint",
    "backwards",
    "leap",
];

/// How a character travels while a move is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Gait {
    #[default]
    Walk,
    Jog,
    Run,
    /// Slow, relaxed walk.
    Stroll,
    /// Crouched, careful steps with the hands raised.
    Sneak,
    /// Quick, short steps on the toes.
    Tiptoe,
    /// Uneven walk favouring the right leg.
    Limp,
    /// Flat-out run with a strong lean.
    Sprint,
    /// Walking backwards: the body faces away from the travel direction.
    Backwards,
    /// One jump along the move: crouch, flight and landing.
    Leap,
}
impl Gait {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "walk" => Self::Walk,
            "jog" => Self::Jog,
            "run" => Self::Run,
            "stroll" => Self::Stroll,
            "sneak" => Self::Sneak,
            "tiptoe" => Self::Tiptoe,
            "limp" => Self::Limp,
            "sprint" => Self::Sprint,
            "backwards" => Self::Backwards,
            "leap" => Self::Leap,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        GAITS[self as usize]
    }
    /// Seconds per full cycle (two steps). A leap is one jump over the whole
    /// move; its value only paces the ease-in.
    pub fn cycle_seconds(self) -> f64 {
        match self {
            Self::Walk => WALK_CYCLE_SECONDS,
            Self::Jog => JOG_CYCLE_SECONDS,
            Self::Run => RUN_CYCLE_SECONDS,
            Self::Stroll | Self::Backwards => 0.8,
            Self::Sneak | Self::Limp => 0.9,
            Self::Tiptoe => 0.36,
            Self::Sprint => 0.34,
            Self::Leap => 1.0,
        }
    }
    /// True for a single jump rather than a repeating step cycle.
    pub fn is_leap(self) -> bool {
        self == Self::Leap
    }
}

/// MotionScript move easing names, in the order editors should present them.
pub const MOVE_EASES: [&str; 4] = ["linear", "in", "out", "in_out"];
/// Fraction of a move spent speeding up (`in`) or slowing down (`out`).
const MOVE_RAMP: f64 = 0.25;

/// How a move's position changes over its duration. Ramps accelerate and
/// decelerate at a constant rate (a trapezoidal speed profile), so the body
/// never jumps to full speed and the cruise speed stays steady in between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum MoveEase {
    /// Constant speed (the legacy behaviour).
    #[default]
    Linear,
    /// Speed up from a standstill.
    In,
    /// Slow down to a stop.
    Out,
    /// Speed up, cruise, slow down.
    InOut,
}
impl MoveEase {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "linear" => Self::Linear,
            "in" => Self::In,
            "out" => Self::Out,
            "in_out" => Self::InOut,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        MOVE_EASES[self as usize]
    }
    /// Fraction of the distance covered at time fraction `u`; monotonic,
    /// `0` at `0` and exactly `1` at `1`. Nonfinite input counts as `0`.
    pub fn apply(self, u: f64) -> f64 {
        if !u.is_finite() {
            return 0.0;
        }
        let u = u.clamp(0.0, 1.0);
        let r = MOVE_RAMP;
        let (start, end) = match self {
            Self::Linear => return u,
            Self::In => (true, false),
            Self::Out => (false, true),
            Self::InOut => (true, true),
        };
        // Cruise speed so the area under the speed profile is one.
        let ramps = f64::from(u8::from(start) + u8::from(end));
        let v = 1.0 / (1.0 - ramps * r / 2.0);
        let head = if start { r / 2.0 } else { 0.0 };
        if start && u < r {
            v * u * u / (2.0 * r)
        } else if end && u > 1.0 - r {
            let rest = 1.0 - u;
            1.0 - v * rest * rest / (2.0 * r)
        } else {
            v * (u - head)
        }
    }
}

/// MotionScript gesture names, in the order editors should present them.
pub const GESTURES: [&str; 12] = [
    "wave",
    "point",
    "nod",
    "shrug",
    "shake_head",
    "facepalm",
    "arms_crossed",
    "thumbs_up",
    "bow",
    "jump",
    "clap",
    "think",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    Wave,
    Point,
    Nod,
    Shrug,
    ShakeHead,
    Facepalm,
    ArmsCrossed,
    ThumbsUp,
    Bow,
    Jump,
    Clap,
    Think,
}
impl Gesture {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "wave" => Self::Wave,
            "point" => Self::Point,
            "nod" => Self::Nod,
            "shrug" => Self::Shrug,
            "shake_head" => Self::ShakeHead,
            "facepalm" => Self::Facepalm,
            "arms_crossed" => Self::ArmsCrossed,
            "thumbs_up" => Self::ThumbsUp,
            "bow" => Self::Bow,
            "jump" => Self::Jump,
            "clap" => Self::Clap,
            "think" => Self::Think,
            _ => return None,
        })
    }
    pub(crate) fn index(self) -> usize {
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
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct HumanoidState<'a> {
    /// Seconds since the active move began, or `None` when standing.
    pub walk_seconds: Option<f64>,
    /// Cycle used while `walk_seconds` is set.
    pub gait: Gait,
    /// Normalized progress through the active move; drives a leap.
    pub move_progress: Option<f64>,
    /// Local horizontal travel direction: `1` forward (+x), `-1` backward, `0`
    /// unknown. Only jog and run lean with it; the walk stays upright.
    pub travel: f64,
    /// Emotion changes oldest first, each with the seconds since it took effect.
    /// Posture starts neutral; each change blends from the posture in flight at
    /// that moment, so closely spaced changes ease continuously.
    pub emotion_changes: &'a [(&'a str, f64)],
    /// Active gesture, its normalized progress and its weight in `0..=1`.
    pub gesture: Option<(Gesture, f64, f64)>,
}

struct Library {
    rig: Rig,
    /// Step cycles indexed by `Gait as usize`; the leap is procedural.
    gaits: Vec<Vec<Option<Track>>>,
    gestures: [Vec<Option<Track>>; 12],
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
        // Jog and run: longer strides, bent arms carried higher, a bigger bob.
        let cycle = |seconds: f64, feet: f64, hands: f64, lift: f64, bob: f64| {
            let q = seconds / 4.0;
            let stride = |a: f64, y: f64| -> [(f64, f64, f64); 5] {
                [
                    (0.0, 0.0, y),
                    (q, a, y),
                    (2.0 * q, 0.0, y),
                    (3.0 * q, -a, y),
                    (4.0 * q, 0.0, y),
                ]
            };
            let bob = [
                (0.0, 0.0, 0.0),
                (q, 0.0, -bob),
                (2.0 * q, 0.0, 0.0),
                (3.0 * q, 0.0, -bob),
                (4.0 * q, 0.0, 0.0),
            ];
            clip(&[
                (HIP, &bob),
                (LEFT_FOOT, &stride(feet, 0.0)),
                (RIGHT_FOOT, &stride(-feet, 0.0)),
                (LEFT_HAND, &stride(-hands, -lift)),
                (RIGHT_HAND, &stride(hands, -lift)),
            ])
        };
        let jog = cycle(JOG_CYCLE_SECONDS, 18.0, 10.0, 10.0, 4.0);
        let run = cycle(RUN_CYCLE_SECONDS, 26.0, 14.0, 16.0, 7.0);
        // Shaped cycles for the other gaits. The hip drops by `crouch`
        // (negative rises onto the toes) while the feet, its children, stay
        // on the floor. `limp` shortens the right stride and deepens the dip
        // when the weight lands on it.
        let shaped =
            |seconds: f64, feet: f64, hands: f64, lift: f64, bob: f64, crouch: f64, limp: f64| {
                let q = seconds / 4.0;
                let hip = [
                    (0.0, 0.0, crouch),
                    (q, 0.0, crouch - bob),
                    (2.0 * q, 0.0, crouch),
                    (3.0 * q, 0.0, crouch - bob + limp * 6.0),
                    (4.0 * q, 0.0, crouch),
                ];
                let stride = |a: f64, y: f64| -> [(f64, f64, f64); 5] {
                    [
                        (0.0, 0.0, y),
                        (q, a, y),
                        (2.0 * q, 0.0, y),
                        (3.0 * q, -a, y),
                        (4.0 * q, 0.0, y),
                    ]
                };
                clip(&[
                    (HIP, &hip),
                    (LEFT_FOOT, &stride(feet, -crouch)),
                    (RIGHT_FOOT, &stride(-feet * (1.0 - 0.5 * limp), -crouch)),
                    (LEFT_HAND, &stride(-hands, -lift)),
                    (RIGHT_HAND, &stride(hands, -lift)),
                ])
            };
        let gaits = vec![
            walk,
            jog,
            run,
            shaped(0.8, 9.0, 5.0, 0.0, 1.5, 0.0, 0.0),
            shaped(0.9, 10.0, 3.0, 22.0, 1.0, 8.0, 0.0),
            shaped(0.36, 6.0, 2.0, 16.0, 1.5, -3.0, 0.0),
            shaped(0.9, 12.0, 6.0, 0.0, 2.0, 0.0, 1.0),
            shaped(0.34, 32.0, 18.0, 20.0, 9.0, 0.0, 0.0),
            shaped(0.8, 9.0, 5.0, 0.0, 1.5, 0.0, 0.0),
            // The leap is evaluated from move progress, not from a cycle.
            vec![None; 7],
        ];
        debug_assert_eq!(gaits.len(), GAITS.len());
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
        let shake_head = clip(&[(
            HEAD,
            &[
                (0.0, 0.0, 0.0),
                (0.17, -5.0, 0.0),
                (0.33, 5.0, 0.0),
                (0.5, -5.0, 0.0),
                (0.67, 5.0, 0.0),
                (0.83, -5.0, 0.0),
                (1.0, 0.0, 0.0),
            ],
        )]);
        let facepalm = clip(&[
            (RIGHT_HAND, &[(0.0, -28.0, -62.0)]),
            (HEAD, &[(0.0, 0.0, 4.0)]),
        ]);
        let arms_crossed = clip(&[
            (LEFT_HAND, &[(0.0, 20.0, -4.0)]),
            (RIGHT_HAND, &[(0.0, -20.0, -4.0)]),
        ]);
        let thumbs_up = clip(&[(RIGHT_HAND, &[(0.0, 14.0, -40.0)])]);
        let bow = clip(&[(HEAD, &[(0.0, 0.0, 14.0)]), (CHEST, &[(0.0, 0.0, 8.0)])]);
        let jump = clip(&[
            (
                HIP,
                &[
                    (0.0, 0.0, 0.0),
                    (0.15, 0.0, 5.0),
                    (0.5, 0.0, -30.0),
                    (0.85, 0.0, 5.0),
                    (1.0, 0.0, 0.0),
                ],
            ),
            (LEFT_HAND, &[(0.0, -6.0, -20.0)]),
            (RIGHT_HAND, &[(0.0, 6.0, -20.0)]),
        ]);
        let clap = clip(&[
            (
                LEFT_HAND,
                &[
                    (0.0, 14.0, -4.0),
                    (0.25, 20.0, -4.0),
                    (0.5, 14.0, -4.0),
                    (0.75, 20.0, -4.0),
                    (1.0, 14.0, -4.0),
                ],
            ),
            (
                RIGHT_HAND,
                &[
                    (0.0, -14.0, -4.0),
                    (0.25, -20.0, -4.0),
                    (0.5, -14.0, -4.0),
                    (0.75, -20.0, -4.0),
                    (1.0, -14.0, -4.0),
                ],
            ),
        ]);
        let think = clip(&[
            (RIGHT_HAND, &[(0.0, -24.0, -48.0)]),
            (HEAD, &[(0.0, 2.0, 0.0)]),
        ]);
        Library {
            rig,
            gaits,
            gestures: [
                wave,
                point,
                nod,
                shrug,
                shake_head,
                facepalm,
                arms_crossed,
                thumbs_up,
                bow,
                jump,
                clap,
                think,
            ],
        }
    })
}

/// Leap timing at move `progress`: flight height in `0..=1`, crouch depth in
/// `0..=1` (take-off and landing) and leg split in `0..=1` (mid-air).
/// Zero outside the move, so the leap starts and ends at rest.
pub fn leap_shape(progress: f64) -> (f64, f64, f64) {
    const TAKE_OFF: f64 = 0.18;
    const LANDING: f64 = 0.82;
    if !progress.is_finite() || progress <= 0.0 || progress >= 1.0 {
        return (0.0, 0.0, 0.0);
    }
    let pi = std::f64::consts::PI;
    if progress < TAKE_OFF {
        (0.0, (pi * progress / TAKE_OFF).sin(), 0.0)
    } else if progress < LANDING {
        let u = (progress - TAKE_OFF) / (LANDING - TAKE_OFF);
        (4.0 * u * (1.0 - u), 0.0, (pi * u).sin())
    } else {
        (
            0.0,
            (pi * (progress - LANDING) / (1.0 - LANDING)).sin(),
            0.0,
        )
    }
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
    evaluate_humanoid_with_offsets(state, &[]).expect("bounded built-in pose")
}

/// Apply authored parent-local offsets after rule clips and before hierarchy evaluation.
/// Duplicate or unknown joints and invalid/beyond-budget offsets are rejected.
pub fn evaluate_humanoid_with_offsets(
    state: &HumanoidState,
    offsets: &[(usize, Transform2)],
) -> Result<[Vec2; 7], crate::Error> {
    evaluate_humanoid_with_weights_and_offsets(state, offsets, ClipWeights::default())
}

/// Weight rule layers before applying independent authored parent-local offsets.
/// Invalid weights fail without evaluating a partial pose.
pub fn evaluate_humanoid_with_weights_and_offsets(
    state: &HumanoidState,
    offsets: &[(usize, Transform2)],
    weights: ClipWeights,
) -> Result<[Vec2; 7], crate::Error> {
    if !weights.is_valid() {
        return Err(crate::Error::InvalidWeight);
    }
    let lib = library();
    let progress = |seconds: f64| {
        if seconds.is_finite() && seconds >= 0.0 {
            (seconds / POSTURE_BLEND_SECONDS).min(1.0)
        } else {
            1.0
        }
    };
    let mut pose = lib.rig.rest_pose();
    for (i, &(emotion, since)) in state.emotion_changes.iter().enumerate() {
        // Blend toward this change only until the next one starts.
        let until = state.emotion_changes.get(i + 1).map_or(0.0, |next| next.1);
        pose = blend_pose(
            &lib.rig,
            &pose,
            &posture_pose(&lib.rig, emotion),
            progress(since - until),
        )
        .expect("built-in postures are bounded");
    }
    // Preserve legacy geometry bit-for-bit at the default intensity.
    if weights.posture != 1.0 {
        pose = blend_pose(&lib.rig, &lib.rig.rest_pose(), &pose, weights.posture)
            .expect("bounded posture intensity");
    }
    if let Some(t) = state.walk_seconds.filter(|t| t.is_finite() && *t >= 0.0) {
        if state.gait.is_leap() {
            let progress = state.move_progress.unwrap_or(0.0);
            let (lift, crouch, split) = leap_shape(progress);
            let w = weights.walk;
            let travel = if state.travel < 0.0 { -1.0 } else { 1.0 };
            pose[HIP].translation.y += (crouch * 8.0 - lift * 60.0) * w;
            pose[LEFT_FOOT].translation.x += 14.0 * split * travel * w;
            pose[RIGHT_FOOT].translation.x -= 14.0 * split * travel * w;
            pose[LEFT_HAND].translation.y -= 30.0 * split * w;
            pose[RIGHT_HAND].translation.y -= 30.0 * split * w;
        } else {
            add(
                &mut pose,
                &lib.gaits[state.gait as usize],
                t.rem_euclid(state.gait.cycle_seconds()),
                weights.walk,
            );
        }
        let lean = match state.gait {
            Gait::Walk | Gait::Stroll | Gait::Limp | Gait::Leap => 0.0,
            Gait::Jog => 3.0,
            Gait::Run => 6.0,
            Gait::Sneak => 5.0,
            Gait::Tiptoe => 1.0,
            Gait::Sprint => 9.0,
            // Leans slightly against the travel direction.
            Gait::Backwards => -1.5,
        };
        if lean != 0.0 && state.travel.is_finite() {
            let lean = lean * state.travel.clamp(-1.0, 1.0) * weights.walk;
            pose[CHEST].translation.x += lean;
            pose[HEAD].translation.x += lean;
        }
    }
    if let Some((gesture, progress, weight)) = state.gesture {
        if progress.is_finite() && weight.is_finite() && weight > 0.0 {
            add(
                &mut pose,
                &lib.gestures[gesture.index()],
                progress.clamp(0.0, 1.0),
                weight.min(1.0) * weights.gesture,
            );
        }
    }
    let mut seen = [false; 7];
    for &(joint, delta) in offsets {
        if joint >= 7 || seen[joint] || !valid_humanoid_offset(delta) {
            return Err(crate::Error::InvalidPose);
        }
        seen[joint] = true;
        let local = &mut pose[joint];
        local.translation.x += delta.translation.x;
        local.translation.y += delta.translation.y;
        local.rotation += delta.rotation;
        local.scale.x *= delta.scale.x;
        local.scale.y *= delta.scale.y;
    }
    let world = lib
        .rig
        .evaluate(&pose)
        .map_err(|_| crate::Error::InvalidPose)?;
    Ok(std::array::from_fn(|i| world[i].point(Vec2::default())))
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
    fn gaits_round_trip_and_differ_while_moving() {
        for name in GAITS {
            assert_eq!(Gait::parse(name).unwrap().name(), name);
        }
        assert_eq!(Gait::parse("moonwalk"), None);
        let pose = |gait, travel| {
            evaluate_humanoid(&HumanoidState {
                walk_seconds: Some(0.1),
                gait,
                travel,
                ..Default::default()
            })
        };
        let walk = pose(Gait::Walk, 1.0);
        let legacy = evaluate_humanoid(&HumanoidState {
            walk_seconds: Some(0.1),
            ..Default::default()
        });
        assert_eq!(walk, legacy, "walk ignores travel and keeps legacy output");
        let (jog, run) = (pose(Gait::Jog, 1.0), pose(Gait::Run, 1.0));
        assert_ne!(jog, walk);
        let idle = evaluate_humanoid(&HumanoidState::default());
        let stride = |pose: [Vec2; 7]| (pose[LEFT_FOOT].x - idle[LEFT_FOOT].x).abs();
        assert!(stride(run) > stride(walk), "run strides further");
        assert!(
            run[HEAD].x > pose(Gait::Run, -1.0)[HEAD].x,
            "run leans toward travel"
        );
        // Cycles repeat exactly.
        let at = |t| {
            evaluate_humanoid(&HumanoidState {
                walk_seconds: Some(t),
                gait: Gait::Run,
                ..Default::default()
            })
        };
        for (a, b) in at(0.05).iter().zip(at(0.05 + 2.0 * RUN_CYCLE_SECONDS)) {
            assert!((a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9);
        }
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
        let sad = |seconds| {
            evaluate_humanoid(&HumanoidState {
                emotion_changes: &[("sad", seconds)],
                ..Default::default()
            })
        };
        let neutral = evaluate_humanoid(&HumanoidState::default());
        let mid = sad(POSTURE_BLEND_SECONDS / 2.0);
        let done = sad(POSTURE_BLEND_SECONDS);
        assert_eq!(sad(0.0), neutral);
        assert!(mid[HEAD].y > neutral[HEAD].y && mid[HEAD].y < done[HEAD].y);
        assert_eq!(done, sad(10.0));
        assert_eq!(done[HEAD].y, neutral[HEAD].y + 9.0);
        assert_eq!(sad(f64::NAN), done);
    }

    #[test]
    fn interrupted_posture_blend_continues_from_in_flight_pose() {
        let d = POSTURE_BLEND_SECONDS;
        let state = |changes: &[(&str, f64)]| {
            evaluate_humanoid(&HumanoidState {
                emotion_changes: changes,
                ..Default::default()
            })
        };
        // Sad starts 0.4 d after happy: at that instant the pose is unchanged.
        let in_flight = state(&[("happy", 0.4 * d)]);
        assert_eq!(state(&[("happy", 0.4 * d), ("sad", 0.0)]), in_flight);
        // Slightly later it moves from there toward sad, not from full happy.
        let next = state(&[("happy", 0.45 * d), ("sad", 0.05 * d)]);
        for (a, b) in next.iter().zip(&in_flight) {
            assert!((a.x - b.x).abs() < 1.0 && (a.y - b.y).abs() < 1.0);
        }
        assert_eq!(
            state(&[("happy", 5.0 * d), ("sad", 2.0 * d)]),
            state(&[("sad", d)])
        );
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

#[cfg(test)]
mod authored_tests {
    use super::*;
    #[test]
    fn custom_parent_rotation_propagates_and_rules_remain_additive() {
        let state = HumanoidState {
            gesture: Some((Gesture::Wave, 0.5, 1.0)),
            ..Default::default()
        };
        let baseline = evaluate_humanoid(&state);
        assert_eq!(
            baseline,
            evaluate_humanoid_with_offsets(&state, &[]).unwrap()
        );
        let moved = evaluate_humanoid_with_offsets(
            &state,
            &[(
                CHEST,
                Transform2 {
                    rotation: std::f64::consts::FRAC_PI_2,
                    ..Default::default()
                },
            )],
        )
        .unwrap();
        assert_eq!(moved[CHEST], baseline[CHEST]);
        assert_ne!(moved[HEAD], baseline[HEAD]);
        assert_ne!(moved[RIGHT_HAND], baseline[RIGHT_HAND]);
        assert_eq!(moved[LEFT_FOOT], baseline[LEFT_FOOT]);
        let delta = [(
            RIGHT_HAND,
            Transform2 {
                translation: Vec2::new(3.0, 4.0),
                ..Default::default()
            },
        )];
        let custom = evaluate_humanoid_with_offsets(&state, &delta).unwrap();
        assert_eq!(
            custom[RIGHT_HAND],
            Vec2::new(baseline[RIGHT_HAND].x + 3.0, baseline[RIGHT_HAND].y + 4.0)
        );
        assert_eq!(
            custom,
            evaluate_humanoid_with_offsets(&state, &delta).unwrap()
        );
    }
    #[test]
    fn invalid_offsets_reject_and_max_hierarchy_stays_bounded() {
        let state = HumanoidState::default();
        assert!(evaluate_humanoid_with_offsets(&state, &[(7, Transform2::default())]).is_err());
        assert!(evaluate_humanoid_with_offsets(
            &state,
            &[(HIP, Transform2::default()), (HIP, Transform2::default())]
        )
        .is_err());
        for delta in [
            Transform2 {
                rotation: f64::NAN,
                ..Default::default()
            },
            Transform2 {
                scale: Vec2::new(4.1, 1.0),
                ..Default::default()
            },
        ] {
            assert!(evaluate_humanoid_with_offsets(&state, &[(HIP, delta)]).is_err());
        }
        let offsets: Vec<_> = (0..7)
            .map(|i| {
                (
                    i,
                    Transform2 {
                        translation: Vec2::new(1000.0, 1000.0),
                        rotation: 1000.0,
                        scale: Vec2::new(4.0, 4.0),
                    },
                )
            })
            .collect();
        assert!(evaluate_humanoid_with_offsets(&state, &offsets).is_ok());
    }
}

#[cfg(test)]
mod intensity_tests {
    use super::*;

    fn weighted(state: &HumanoidState, weights: ClipWeights) -> [Vec2; 7] {
        evaluate_humanoid_with_weights_and_offsets(state, &[], weights).unwrap()
    }

    #[test]
    fn default_intensities_keep_compatibility_wrapper_points_identical() {
        let offsets = [(
            HEAD,
            Transform2 {
                rotation: 0.2,
                ..Default::default()
            },
        )];
        for seconds in [0.0, 0.01, 0.07, 0.15, 0.24, 0.25, 1.3] {
            let state = HumanoidState {
                walk_seconds: Some(seconds),
                emotion_changes: &[("happy", seconds + 0.05), ("sad", seconds)],
                gesture: Some((Gesture::Wave, 0.4, 0.8)),
                ..Default::default()
            };
            let before = evaluate_humanoid_with_offsets(&state, &offsets).unwrap();
            let after = evaluate_humanoid_with_weights_and_offsets(
                &state,
                &offsets,
                ClipWeights::default(),
            )
            .unwrap();
            for (a, b) in before.iter().zip(after) {
                assert_eq!(
                    (a.x.to_bits(), a.y.to_bits()),
                    (b.x.to_bits(), b.y.to_bits())
                );
            }
        }
    }

    #[test]
    fn each_layer_can_be_disabled_or_halved_without_frame_history() {
        let neutral = evaluate_humanoid(&HumanoidState::default());
        let states = [
            HumanoidState {
                emotion_changes: &[("happy", 1.0)],
                ..Default::default()
            },
            HumanoidState {
                walk_seconds: Some(0.3),
                ..Default::default()
            },
            HumanoidState {
                gesture: Some((Gesture::Shrug, 0.5, 1.0)),
                ..Default::default()
            },
        ];
        for (i, state) in states.into_iter().enumerate() {
            let full = evaluate_humanoid(&state);
            assert_ne!(full, neutral);
            let mut off = ClipWeights::default();
            let mut half = off;
            match i {
                0 => {
                    off.posture = 0.0;
                    half.posture = 0.5;
                }
                1 => {
                    off.walk = 0.0;
                    half.walk = 0.5;
                }
                _ => {
                    off.gesture = 0.0;
                    half.gesture = 0.5;
                }
            }
            assert_eq!(weighted(&state, off), neutral);
            let actual = weighted(&state, half);
            for j in 0..7 {
                assert!((actual[j].x - (neutral[j].x + full[j].x) * 0.5).abs() < 1e-9);
                assert!((actual[j].y - (neutral[j].y + full[j].y) * 0.5).abs() < 1e-9);
            }
            assert_eq!(weighted(&state, half), actual);
        }
    }

    #[test]
    fn disabled_rules_preserve_authored_offsets_and_reject_bad_weights() {
        let state = HumanoidState {
            emotion_changes: &[("sad", 1.0)],
            walk_seconds: Some(0.3),
            gesture: Some((Gesture::Wave, 0.5, 1.0)),
            ..Default::default()
        };
        let offsets = [(
            RIGHT_HAND,
            Transform2 {
                translation: Vec2 { x: 12.0, y: 3.0 },
                ..Default::default()
            },
        )];
        let off = ClipWeights {
            posture: 0.0,
            walk: 0.0,
            gesture: 0.0,
        };
        assert_eq!(
            evaluate_humanoid_with_weights_and_offsets(&state, &offsets, off).unwrap(),
            evaluate_humanoid_with_offsets(&HumanoidState::default(), &offsets).unwrap()
        );
        for bad in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            for weights in [
                ClipWeights {
                    posture: bad,
                    ..off
                },
                ClipWeights { walk: bad, ..off },
                ClipWeights {
                    gesture: bad,
                    ..off
                },
            ] {
                assert!(!weights.is_valid());
                assert!(
                    evaluate_humanoid_with_weights_and_offsets(&state, &offsets, weights).is_err()
                );
            }
        }
    }

    #[test]
    fn close_emotion_changes_remain_continuous_with_partial_posture() {
        let weights = ClipWeights {
            posture: 0.4,
            ..Default::default()
        };
        let before = weighted(
            &HumanoidState {
                emotion_changes: &[("happy", 0.05)],
                ..Default::default()
            },
            weights,
        );
        let onset = weighted(
            &HumanoidState {
                emotion_changes: &[("happy", 0.05), ("sad", 0.0)],
                ..Default::default()
            },
            weights,
        );
        assert_eq!(before, onset);
        let seek = HumanoidState {
            emotion_changes: &[("happy", 0.12), ("sad", 0.07)],
            ..Default::default()
        };
        let expected = weighted(&seek, weights);
        let _ = weighted(&HumanoidState::default(), weights);
        assert_eq!(weighted(&seek, weights), expected);
    }
}

#[cfg(test)]
mod humanoid_locomotion_tests {
    use super::*;

    #[test]
    fn built_in_humanoid_steps_with_every_gait_and_leaps_from_progress() {
        let idle = evaluate_humanoid(&HumanoidState::default());
        for name in GAITS {
            let gait = Gait::parse(name).unwrap();
            let pose = evaluate_humanoid(&HumanoidState {
                walk_seconds: Some(gait.cycle_seconds() * 0.25),
                gait,
                move_progress: Some(0.5),
                travel: 1.0,
                ..Default::default()
            });
            assert_ne!(pose, idle, "{name}");
            assert!(pose.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
        }
        let leap = |progress| {
            evaluate_humanoid(&HumanoidState {
                walk_seconds: Some(0.2),
                gait: Gait::Leap,
                move_progress: Some(progress),
                ..Default::default()
            })
        };
        assert!(leap(0.5)[HIP].y < idle[HIP].y - 50.0, "airborne");
        assert_eq!(leap(0.0), idle);
        // Sneaking lowers the hip while the feet stay on the floor.
        let sneak = evaluate_humanoid(&HumanoidState {
            walk_seconds: Some(0.0),
            gait: Gait::Sneak,
            ..Default::default()
        });
        assert!(sneak[HIP].y > idle[HIP].y);
        assert!((sneak[LEFT_FOOT].y - idle[LEFT_FOOT].y).abs() < 1e-9);
    }
}
