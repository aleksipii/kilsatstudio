//! Procedural gait and gesture offsets for custom rigs that use the standard
//! limb joint names (`root`, `torso`, `head`, `upper_arm_l`, `forearm_l`,
//! `thigh_l`, `shin_l` and their `_r` mirrors).
//!
//! Offsets are parent-local and additive over the rest pose, like authored
//! joint keys: translations and rotations add. Rotations follow the rig
//! convention (y down): a positive rotation swings a hanging limb toward -x.
//! Every value is a pure function of its inputs, so seeking is exact.
use crate::principles::squash_scale;
use crate::{Gait, Gesture, Principles, Transform2, Vec2};
use std::f64::consts::TAU;

/// Standard joint names this module drives. A rig may define any subset.
pub const RIG_MOTION_JOINTS: [&str; 11] = [
    "root",
    "torso",
    "head",
    "upper_arm_l",
    "forearm_l",
    "upper_arm_r",
    "forearm_r",
    "thigh_l",
    "shin_l",
    "thigh_r",
    "shin_r",
];

const ROOT: usize = 0;
const TORSO: usize = 1;
const HEAD: usize = 2;
const UPPER_ARM_L: usize = 3;
const FOREARM_L: usize = 4;
const UPPER_ARM_R: usize = 5;
const FOREARM_R: usize = 6;
const THIGH_L: usize = 7;
const SHIN_L: usize = 8;
const THIGH_R: usize = 9;
const SHIN_R: usize = 10;

/// One offset per entry of [`RIG_MOTION_JOINTS`].
pub type RigMotion = [Transform2; 11];

fn rest() -> RigMotion {
    [Transform2::default(); 11]
}

struct GaitShape {
    /// Thigh swing amplitude, radians.
    leg: f64,
    /// Extra knee bend while the leg swings forward.
    knee: f64,
    /// Upper arm swing amplitude.
    arm: f64,
    /// Constant elbow bend (negative brings the hand forward).
    elbow: f64,
    /// Vertical bob of the root in rig units.
    bob: f64,
    /// Forward torso lean.
    lean: f64,
    /// Bent-knee crouch in `0..=1`: the root drops and both knees bend so
    /// the feet stay under the body.
    crouch: f64,
    /// Constant forward raise of both upper arms, radians.
    reach: f64,
    /// Rise onto the toes in rig units.
    toe: f64,
    /// Right-leg favouring in `0..=1`: shorter right swing, a dip and a
    /// sideways torso tilt when the weight lands on it.
    limp: f64,
}

const fn gait_shape(leg: f64, knee: f64, arm: f64, elbow: f64, bob: f64, lean: f64) -> GaitShape {
    GaitShape {
        leg,
        knee,
        arm,
        elbow,
        bob,
        lean,
        crouch: 0.0,
        reach: 0.0,
        toe: 0.0,
        limp: 0.0,
    }
}

fn shape(gait: Gait) -> GaitShape {
    match gait {
        Gait::Walk => gait_shape(0.42, 0.55, 0.35, -0.25, 4.0, 0.03),
        Gait::Jog => gait_shape(0.55, 0.95, 0.5, -1.2, 8.0, 0.09),
        Gait::Run => gait_shape(0.75, 1.35, 0.7, -1.5, 12.0, 0.18),
        Gait::Stroll => gait_shape(0.3, 0.4, 0.22, -0.15, 2.5, 0.0),
        Gait::Sneak => GaitShape {
            crouch: 1.0,
            reach: 0.9,
            ..gait_shape(0.3, 0.45, 0.08, -1.7, 2.0, 0.32)
        },
        Gait::Tiptoe => GaitShape {
            reach: 0.45,
            toe: 7.0,
            ..gait_shape(0.22, 0.4, 0.08, -1.9, 3.0, 0.06)
        },
        Gait::Limp => GaitShape {
            limp: 1.0,
            ..gait_shape(0.36, 0.45, 0.3, -0.2, 3.0, 0.05)
        },
        Gait::Sprint => gait_shape(0.95, 1.7, 0.95, -1.6, 14.0, 0.3),
        // Leans slightly against the travel direction.
        Gait::Backwards => gait_shape(0.3, 0.4, 0.2, -0.3, 3.0, -0.06),
        // Not cyclic; see `rig_leap`.
        Gait::Leap => gait_shape(0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        Gait::Carry => GaitShape {
            crouch: 0.35,
            reach: 0.75,
            ..gait_shape(0.26, 0.4, 0.04, -1.2, 3.0, 0.2)
        },
    }
}

/// Sneak crouch at full depth: root drop and the bend that keeps the feet
/// under the hips (thigh 70, shin 62 rig units in the stick-studio rigs).
const CROUCH_DROP: f64 = 16.0;
const CROUCH_THIGH: f64 = -0.45;
const CROUCH_SHIN: f64 = 1.0;

fn rotation(value: f64) -> Transform2 {
    Transform2 {
        rotation: value,
        ..Default::default()
    }
}

fn translation(x: f64, y: f64) -> Transform2 {
    Transform2 {
        translation: Vec2::new(x, y),
        ..Default::default()
    }
}

/// Mirror offsets for travel toward -x: rotations and x translations flip.
fn mirror(motion: &mut RigMotion) {
    for offset in motion {
        offset.rotation = -offset.rotation;
        offset.translation.x = -offset.translation.x;
    }
}

/// Fraction of a gait cycle over which a move eases in from the rest pose.
const GAIT_EASE_IN_CYCLES: f64 = 0.5;
/// Phase lag (radians) of forearms and shins behind the limb that drives them.
const GAIT_DRAG: f64 = 0.5;

fn smoothstep(u: f64) -> f64 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// How an emotion colours a gait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum GaitMood {
    #[default]
    Neutral,
    /// Drags: small arm swing, head and shoulders down, little bounce.
    Sad,
    /// Stomps: heavy bob, stiff pumping arms, leaning in.
    Angry,
    /// Bounces: springy bob, loose arms, head up.
    Happy,
}
impl GaitMood {
    /// Mood for a MotionScript emotion; unknown or neutral emotions are neutral.
    pub fn from_emotion(emotion: &str) -> Self {
        match emotion {
            "sad" => Self::Sad,
            "angry" => Self::Angry,
            "happy" => Self::Happy,
            _ => Self::Neutral,
        }
    }
}

/// Gait offsets `seconds` into an active move. `travel` is the local
/// horizontal direction (+1 toward +x, -1 toward -x); zero counts as +x.
/// `weight` scales the whole layer and is clamped to `0..=1`.
///
/// The first half cycle eases in from rest (slow in), and forearms trail
/// their upper arms (overlapping action), so a move never pops into full
/// stride.
pub fn rig_gait(gait: Gait, seconds: f64, travel: f64, weight: f64) -> RigMotion {
    rig_gait_with_mood(gait, seconds, travel, weight, GaitMood::Neutral)
}

/// [`rig_gait`] coloured by `mood`. A neutral mood gives exactly `rig_gait`.
pub fn rig_gait_with_mood(
    gait: Gait,
    seconds: f64,
    travel: f64,
    weight: f64,
    mood: GaitMood,
) -> RigMotion {
    let mut motion = rest();
    let weight = if weight.is_finite() {
        weight.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if !seconds.is_finite() || seconds < 0.0 || weight == 0.0 {
        return motion;
    }
    if gait.is_leap() {
        // A leap needs move progress, not elapsed seconds.
        return motion;
    }
    let mut s = shape(gait);
    // Mood scales the swing and bounce; the head is set below.
    let head_drop = match mood {
        GaitMood::Neutral => 0.0,
        GaitMood::Sad => {
            s.arm *= 0.4;
            s.bob *= 0.5;
            s.leg *= 0.8;
            s.lean += 0.08;
            0.18
        }
        GaitMood::Angry => {
            s.bob *= 1.7;
            s.arm *= 1.3;
            s.elbow -= 0.5;
            s.lean += 0.08;
            0.06
        }
        GaitMood::Happy => {
            s.bob *= 1.9;
            s.arm *= 1.25;
            -0.08
        }
    };
    let cycle = gait.cycle_seconds();
    let weight = weight * smoothstep(seconds / (cycle * GAIT_EASE_IN_CYCLES));
    let phase = TAU * seconds.rem_euclid(cycle) / cycle;
    let (sin, cos) = phase.sin_cos();
    let drag = (phase - GAIT_DRAG).sin();
    // The favoured (right) leg swings less.
    let right = 1.0 - 0.45 * s.limp;
    // Left leg forward while sin > 0; a forward-swinging leg bends its knee.
    motion[THIGH_L] = rotation((-s.leg * sin + CROUCH_THIGH * s.crouch) * weight);
    motion[THIGH_R] = rotation((s.leg * right * sin + CROUCH_THIGH * s.crouch) * weight);
    motion[SHIN_L] = rotation((0.08 + s.knee * cos.max(0.0) + CROUCH_SHIN * s.crouch) * weight);
    motion[SHIN_R] =
        rotation((0.08 + s.knee * right * (-cos).max(0.0) + CROUCH_SHIN * s.crouch) * weight);
    // Arms swing opposite to the legs; forearms follow a beat later.
    motion[UPPER_ARM_L] = rotation((s.arm * sin - s.reach) * weight);
    motion[UPPER_ARM_R] = rotation((-s.arm * sin - s.reach) * weight);
    motion[FOREARM_L] = rotation((s.elbow - 0.15 * drag.max(0.0)) * weight);
    motion[FOREARM_R] = rotation((s.elbow - 0.15 * (-drag).max(0.0)) * weight);
    // Highest at passing (legs together), lowest at contact. A limp sinks
    // further while the weight is on the right leg (sin > 0 puts it behind).
    let dip = s.limp * 6.0 * sin.max(0.0);
    let drop = CROUCH_DROP * s.crouch - s.toe;
    motion[ROOT] = translation(0.0, (drop - s.bob * (1.0 - sin.abs()) + dip) * weight);
    motion[TORSO] = rotation((s.lean + 0.06 * s.limp * sin.max(0.0)) * weight);
    // The head counters the lean and lags the bob slightly.
    motion[HEAD] = Transform2 {
        translation: Vec2::new(0.0, 0.15 * s.bob * (2.0 * phase - GAIT_DRAG).cos() * weight),
        rotation: (-0.5 * s.lean + head_drop) * weight,
        ..Default::default()
    };
    if travel < 0.0 {
        mirror(&mut motion);
    }
    motion
}

/// Seconds to sit down or stand up.
pub const SIT_SECONDS: f64 = 0.4;

/// Sitting offsets blended by `weight` in `0..=1`. In profile the thighs come
/// forward to the horizontal and the shins hang; seen from the front the knees
/// part and the hips drop a little. Rotations follow the rig convention, so a
/// mirrored (left-facing) actor sits the same way.
pub fn rig_sit(weight: f64, profile: bool) -> RigMotion {
    let mut motion = rest();
    if !weight.is_finite() || weight <= 0.0 {
        return motion;
    }
    let w = smoothstep(weight.min(1.0));
    if profile {
        motion[ROOT] = translation(0.0, 60.0 * w);
        motion[THIGH_L] = rotation(-1.45 * w);
        motion[THIGH_R] = rotation(-1.4 * w);
        motion[SHIN_L] = rotation(1.45 * w);
        motion[SHIN_R] = rotation(1.4 * w);
        motion[TORSO] = rotation(-0.05 * w);
    } else {
        motion[ROOT] = translation(0.0, 22.0 * w);
        motion[THIGH_L] = rotation(0.75 * w);
        motion[THIGH_R] = rotation(-0.75 * w);
        motion[SHIN_L] = rotation(-0.75 * w);
        motion[SHIN_R] = rotation(0.75 * w);
    }
    motion
}

/// Leap offsets at move `progress` in `0..=1`: a crouch, a flight with the
/// legs split and the arms reaching forward, and a soft landing. `travel`
/// as in [`rig_gait`]; `weight` scales the layer.
pub fn rig_leap(progress: f64, travel: f64, weight: f64) -> RigMotion {
    let mut motion = rest();
    let weight = if weight.is_finite() {
        weight.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if weight == 0.0 {
        return motion;
    }
    let (lift, crouch, split) = crate::leap_shape(progress);
    if lift == 0.0 && crouch == 0.0 && split == 0.0 {
        return motion;
    }
    let w = weight;
    // Crouch: knees bend and the arms swing back; flight: front leg forward,
    // back leg trailing, arms reaching.
    motion[ROOT] = translation(0.0, (22.0 * crouch - 110.0 * lift) * w);
    motion[THIGH_L] = rotation((-0.6 * crouch - 0.85 * split) * w);
    motion[SHIN_L] = rotation((1.1 * crouch + 0.35 * split) * w);
    motion[THIGH_R] = rotation((-0.6 * crouch + 0.7 * split) * w);
    motion[SHIN_R] = rotation((1.1 * crouch + 1.1 * split) * w);
    motion[UPPER_ARM_L] = rotation((0.7 * crouch - 1.9 * split) * w);
    motion[UPPER_ARM_R] = rotation((0.7 * crouch - 1.5 * split) * w);
    motion[FOREARM_L] = rotation(-0.3 * split * w);
    motion[FOREARM_R] = rotation(-0.3 * split * w);
    motion[TORSO] = rotation((0.25 * crouch + 0.15 * split) * w);
    motion[HEAD] = rotation((-0.1 * crouch - 0.08 * split) * w);
    if travel < 0.0 {
        mirror(&mut motion);
    }
    motion
}

/// MotionScript body poses, in the order editors should present them.
pub const BODY_POSES: [&str; 6] = ["stand", "sit", "crouch", "kneel", "lie", "fall"];

/// A body pose that lasts until the next pose change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum BodyPose {
    #[default]
    Stand,
    Sit,
    Crouch,
    Kneel,
    /// Lying on the floor.
    Lie,
    /// Falling flat: ends lying on the floor, reached quickly.
    Fall,
}
impl BodyPose {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "stand" => Self::Stand,
            "sit" => Self::Sit,
            "crouch" => Self::Crouch,
            "kneel" => Self::Kneel,
            "lie" => Self::Lie,
            "fall" => Self::Fall,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        BODY_POSES[self as usize]
    }
    /// Seconds to move into this pose from the previous one.
    pub fn seconds(self) -> f64 {
        match self {
            Self::Stand | Self::Sit => SIT_SECONDS,
            Self::Crouch => 0.3,
            Self::Kneel => 0.5,
            Self::Lie => 0.9,
            Self::Fall => 0.35,
        }
    }
}

/// Full offsets of a body pose. `profile` picks the side view; rotations
/// follow the rig convention, so a mirrored actor poses the same way.
/// Leg geometry assumes the stick-studio proportions (thigh 70, shin 62).
pub fn rig_body_pose(pose: BodyPose, profile: bool) -> RigMotion {
    let mut motion = rest();
    match (pose, profile) {
        (BodyPose::Stand, _) => {}
        (BodyPose::Sit, _) => motion = rig_sit(1.0, profile),
        (BodyPose::Crouch, true) => {
            // Knees forward, feet under the hips, torso leaning in.
            motion[ROOT] = translation(0.0, 30.0);
            motion[THIGH_L] = rotation(-0.7);
            motion[SHIN_L] = rotation(1.5);
            motion[THIGH_R] = rotation(-0.6);
            motion[SHIN_R] = rotation(1.49);
            motion[TORSO] = rotation(0.35);
            motion[HEAD] = rotation(-0.2);
            motion[UPPER_ARM_L] = rotation(-0.5);
            motion[UPPER_ARM_R] = rotation(-0.4);
        }
        (BodyPose::Crouch, false) => {
            // Knees apart, feet stay under the shoulders.
            motion[ROOT] = translation(0.0, 17.0);
            motion[THIGH_L] = rotation(0.6);
            motion[SHIN_L] = rotation(-1.1);
            motion[THIGH_R] = rotation(-0.6);
            motion[SHIN_R] = rotation(1.1);
            motion[UPPER_ARM_L] = rotation(-0.3);
            motion[UPPER_ARM_R] = rotation(0.3);
        }
        (BodyPose::Kneel, true) => {
            // Front foot planted, back knee and shin on the floor.
            motion[ROOT] = translation(0.0, 62.0);
            motion[THIGH_L] = rotation(-1.52);
            motion[SHIN_L] = rotation(1.52);
            motion[THIGH_R] = rotation(0.7);
            motion[SHIN_R] = rotation(0.93);
            motion[TORSO] = rotation(0.05);
        }
        (BodyPose::Kneel, false) => {
            // Both knees down, shins folded out to the sides.
            motion[ROOT] = translation(0.0, 64.0);
            motion[THIGH_L] = rotation(0.25);
            motion[SHIN_L] = rotation(1.32);
            motion[THIGH_R] = rotation(-0.25);
            motion[SHIN_R] = rotation(-1.32);
        }
        (BodyPose::Lie | BodyPose::Fall, _) => {
            // Flat on the floor, head toward the facing direction and
            // slightly raised so it rests on the ground line.
            motion[ROOT] = Transform2 {
                translation: Vec2::new(0.0, 96.0),
                rotation: std::f64::consts::FRAC_PI_2 - 0.06,
                ..Default::default()
            };
            motion[HEAD] = rotation(-0.25);
            motion[UPPER_ARM_L] = rotation(-0.15);
            motion[UPPER_ARM_R] = rotation(0.15);
            motion[THIGH_L] = rotation(-0.35);
            motion[THIGH_R] = rotation(0.05);
        }
    }
    motion
}

/// Blend two offset sets: translations and rotations interpolate, scales
/// interpolate too (unit scale stays unit). `t` is clamped to `0..=1`.
pub fn blend_rig_motion(from: &RigMotion, to: &RigMotion, t: f64) -> RigMotion {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let lerp = |a: f64, b: f64| a + (b - a) * t;
    std::array::from_fn(|i| {
        let (a, b) = (from[i], to[i]);
        Transform2 {
            translation: Vec2::new(
                lerp(a.translation.x, b.translation.x),
                lerp(a.translation.y, b.translation.y),
            ),
            rotation: lerp(a.rotation, b.rotation),
            scale: Vec2::new(lerp(a.scale.x, b.scale.x), lerp(a.scale.y, b.scale.y)),
        }
    })
}

/// Body pose offsets at `time` from timed pose changes `(at, pose)` sorted by
/// time. Each change eases (smoothstep) from the pose in flight at that moment
/// over the new pose's [`BodyPose::seconds`], so interrupted changes stay
/// continuous. A pure function of its inputs.
pub fn rig_pose_at(changes: &[(f64, BodyPose)], time: f64, profile: bool) -> RigMotion {
    let mut from = rest();
    let mut to = rest();
    let mut progress = 1.0;
    let mut current: Option<(f64, BodyPose)> = None;
    for &(at, pose) in changes {
        if !at.is_finite() || at > time {
            break;
        }
        if let Some((start, previous)) = current {
            // The pose in flight when this change starts.
            let p = (at - start) / previous.seconds();
            from = blend_rig_motion(&from, &to, smoothstep(p));
        }
        to = rig_body_pose(pose, profile);
        current = Some((at, pose));
    }
    if let Some((start, pose)) = current {
        progress = (time - start) / pose.seconds();
    }
    let mut motion = blend_rig_motion(&from, &to, smoothstep(progress));
    // Tipping over pivots on the feet: while the body is part way down, the
    // root drops only as far as the rotation lowers it, so the feet never
    // sink through the floor (the final lying pose is already on it).
    let tilt = motion[ROOT].rotation;
    if tilt != 0.0 {
        // Lowest sole corner after tilting; the root moves so it stays on
        // the floor line (it may rise a little early in the tip).
        let lowest = FOOT_SPAN * tilt.sin().abs() + SOLE_DEPTH * tilt.cos();
        motion[ROOT].translation.y = motion[ROOT].translation.y.min(SOLE_DEPTH - lowest);
    }
    motion
}

/// Stick-studio feet relative to the root: soles this far below it (thigh
/// 70, shin 62, foot 10) and outer toe or heel edges this far to either side.
const SOLE_DEPTH: f64 = 142.0;
const FOOT_SPAN: f64 = 33.0;

/// Seconds of stillness before the moving hold starts to fade in, so short
/// pauses between actions stay crisp.
pub const MOVING_HOLD_DELAY: f64 = 0.5;
/// Seconds over which the moving hold fades in after the delay.
const MOVING_HOLD_FADE: f64 = 1.0;
/// Breathing cycle of the moving hold.
const BREATH_SECONDS: f64 = 3.6;
/// Slower, out-of-phase head sway of the moving hold.
const SWAY_SECONDS: f64 = 5.3;

/// Moving-hold offsets `still_seconds` into a hold: the chest rises and falls
/// with a slow breath, the shoulders follow it and the head sways slightly.
/// `strength` in `0..=1` scales the layer. Deterministic (no randomness) and
/// zero at the start of the hold, so it never pops in.
pub fn rig_moving_hold(still_seconds: f64, strength: f64) -> RigMotion {
    let mut motion = rest();
    if !still_seconds.is_finite() || !strength.is_finite() || strength <= 0.0 {
        return motion;
    }
    let w = strength.min(1.0) * smoothstep((still_seconds - MOVING_HOLD_DELAY) / MOVING_HOLD_FADE);
    if w == 0.0 {
        return motion;
    }
    let t = still_seconds - MOVING_HOLD_DELAY;
    // 0 at exhale, 1 at full inhale.
    let breath = 0.5 * (1.0 - (TAU * t / BREATH_SECONDS).cos());
    let sway = (TAU * t / SWAY_SECONDS).sin();
    motion[TORSO] = translation(0.0, -1.5 * breath * w);
    motion[HEAD] = rotation(0.03 * sway * w);
    motion[UPPER_ARM_L] = rotation(-0.025 * breath * w);
    motion[UPPER_ARM_R] = rotation(0.025 * breath * w);
    motion
}

/// End of the anticipation (a small move against the action).
const ANTICIPATION_END: f64 = 0.12;
/// End of the attack, which overshoots and settles into the held pose.
const ATTACK_END: f64 = 0.38;
/// Start of the release back to rest.
const RELEASE_START: f64 = 0.7;
// Anticipation depth, overshoot, follow-through and overlap are tunable; see
// `Principles` (its defaults are the values used here before).

/// Acting curve for one joint at normalized progress with the default
/// principles; see [`acting_curve_with`].
pub fn acting_curve(progress: f64) -> f64 {
    acting_curve_with(progress, &Principles::DEFAULT)
}

/// Acting curve for one joint at normalized progress: anticipation, an
/// overshooting attack that settles, a hold, and an eased release with a small
/// follow-through. Exactly zero at and outside both ends. Depths come from
/// `principles`, which callers validate.
pub fn acting_curve_with(progress: f64, principles: &Principles) -> f64 {
    let p = progress;
    if !p.is_finite() || p <= 0.0 || p >= 1.0 {
        return 0.0;
    }
    if p < ANTICIPATION_END {
        return -principles.anticipation * (std::f64::consts::PI * p / ANTICIPATION_END).sin();
    }
    if p < ATTACK_END {
        let overshoot = principles.overshoot;
        let u = (p - ANTICIPATION_END) / (ATTACK_END - ANTICIPATION_END) - 1.0;
        return 1.0 + (overshoot + 1.0) * u * u * u + overshoot * u * u;
    }
    if p < RELEASE_START {
        return 1.0;
    }
    let u = (p - RELEASE_START) / (1.0 - RELEASE_START);
    1.0 - smoothstep(u) - principles.follow_through * (std::f64::consts::PI * u).sin() * u * u
}

/// Driver joints finish early; children start late. Both span the full curve,
/// so children trail into the pose and keep moving after the driver settles.
fn driver(progress: f64, principles: &Principles) -> f64 {
    acting_curve_with(progress / (1.0 - principles.overlap), principles)
}
fn follower(progress: f64, principles: &Principles) -> f64 {
    let overlap = principles.overlap;
    acting_curve_with((progress - overlap) / (1.0 - overlap), principles)
}

/// Progress step for the slope that drives squash and stretch.
const SLOPE_STEP: f64 = 1.0 / 1024.0;

/// Signed torso stretch for a gesture: the torso stretches while the driver
/// moves into the pose and squashes as it settles back or is released. A pure
/// function of progress, so seeking matches frame-by-frame playback.
fn gesture_stretch(gesture: Gesture, p: f64, w: f64, principles: &Principles) -> f64 {
    let amount = principles.squash_stretch;
    if gesture == Gesture::Jump {
        // Squash in the crouch and on landing, stretch while rising.
        return amount * w * jump_stretch(p);
    }
    let slope = (driver((p + SLOPE_STEP).min(1.0), principles)
        - driver((p - SLOPE_STEP).max(0.0), principles))
        / (2.0 * SLOPE_STEP);
    amount * w * (slope / 8.0).tanh()
}

/// Jump stretch in `-1..=1` at progress `p`.
fn jump_stretch(p: f64) -> f64 {
    let crouch = |u: f64| (std::f64::consts::PI * u).sin().max(0.0);
    if p < 0.2 {
        -crouch(p / 0.2)
    } else if p < 0.8 {
        // Vertical speed of the parabolic flight, normalized: +1 at take-off,
        // -1 just before landing.
        let u = (p - 0.2) / 0.6;
        1.0 - 2.0 * u
    } else {
        -crouch((p - 0.8) / 0.2)
    }
}

/// Gesture offsets at normalized `progress` with the default principles; see
/// [`rig_gesture_with`].
pub fn rig_gesture(gesture: Gesture, progress: f64, weight: f64) -> RigMotion {
    rig_gesture_with(gesture, progress, weight, &Principles::DEFAULT)
}

/// Gesture offsets at normalized `progress`, scaled by the layer `weight` in
/// `0..=1`. Timing (anticipation, overshoot, overlap, follow-through) comes
/// from `progress` shaped by `principles`; `weight` only blends the layer.
/// Invalid principles fall back to the defaults.
pub fn rig_gesture_with(
    gesture: Gesture,
    progress: f64,
    weight: f64,
    principles: &Principles,
) -> RigMotion {
    let mut motion = rest();
    if !progress.is_finite() || !weight.is_finite() || weight <= 0.0 {
        return motion;
    }
    let principles = if principles.validate().is_ok() {
        principles
    } else {
        &Principles::DEFAULT
    };
    let w = weight.min(1.0);
    let p = progress.clamp(0.0, 1.0);
    let (lead, trail) = (driver(p, principles) * w, follower(p, principles) * w);
    match gesture {
        Gesture::Wave => {
            // The hand only waves once the arm is up.
            let wag = 0.45 * (TAU * 3.0 * p).sin() * trail.max(0.0);
            motion[UPPER_ARM_R] = rotation(-2.5 * lead);
            motion[FOREARM_R] = rotation(-0.35 * trail + wag);
            motion[TORSO] = rotation(0.05 * lead);
        }
        Gesture::Point => {
            motion[UPPER_ARM_R] = rotation(-1.45 * lead);
            motion[FOREARM_R] = rotation(0.1 * trail);
            motion[TORSO] = rotation(-0.06 * lead);
            motion[HEAD] = rotation(-0.08 * trail);
        }
        Gesture::Nod => {
            // Two dips; the anticipation lifts the head first.
            let dip = (TAU * p).sin().powi(2);
            motion[HEAD] = translation(0.0, 7.0 * dip * lead.max(0.0) - 3.0 * (-lead).max(0.0));
            motion[TORSO] = rotation(0.03 * dip * trail.max(0.0));
        }
        Gesture::Shrug => {
            motion[UPPER_ARM_L] = Transform2 {
                translation: Vec2::new(0.0, -8.0 * lead),
                rotation: 0.35 * lead,
                ..Default::default()
            };
            motion[UPPER_ARM_R] = Transform2 {
                translation: Vec2::new(0.0, -8.0 * lead),
                rotation: -0.35 * lead,
                ..Default::default()
            };
            motion[FOREARM_L] = rotation(1.2 * trail);
            motion[FOREARM_R] = rotation(-1.2 * trail);
            motion[HEAD] = translation(0.0, 4.0 * trail);
        }
        Gesture::ShakeHead => {
            // Three side-to-side turns once the head is engaged.
            let turn = 0.16 * (TAU * 3.0 * p).sin() * trail.max(0.0);
            motion[HEAD] = rotation(turn);
            motion[TORSO] = rotation(-0.02 * turn);
        }
        Gesture::Facepalm => {
            // Right hand up and across to the face; the head drops into it.
            motion[UPPER_ARM_R] = rotation(1.9 * lead);
            motion[FOREARM_R] = rotation(1.6 * trail);
            motion[HEAD] = Transform2 {
                translation: Vec2::new(0.0, 5.0 * trail),
                rotation: -0.12 * trail,
                ..Default::default()
            };
            motion[TORSO] = rotation(0.04 * lead);
        }
        Gesture::ArmsCrossed => {
            motion[UPPER_ARM_R] = rotation(0.5 * lead);
            motion[UPPER_ARM_L] = rotation(-0.5 * lead);
            motion[FOREARM_R] = rotation(2.0 * trail);
            motion[FOREARM_L] = rotation(-2.0 * trail);
            motion[HEAD] = rotation(-0.05 * trail);
        }
        Gesture::ThumbsUp => {
            motion[UPPER_ARM_R] = rotation(-0.3 * lead);
            motion[FOREARM_R] = rotation(-2.8 * trail);
            motion[TORSO] = rotation(-0.03 * lead);
            motion[HEAD] = rotation(0.06 * trail);
        }
        Gesture::Bow => {
            // Knees give a little; head and shoulders dip.
            motion[ROOT] = translation(0.0, 6.0 * lead);
            motion[TORSO] = translation(0.0, 6.0 * lead);
            motion[HEAD] = translation(0.0, 16.0 * trail);
            motion[UPPER_ARM_L] = rotation(-0.12 * trail);
            motion[UPPER_ARM_R] = rotation(0.12 * trail);
        }
        Gesture::Jump => {
            // Crouch, a parabolic flight with legs tucked, and a soft landing.
            let crouch = |u: f64| (std::f64::consts::PI * u).sin().max(0.0);
            let (lift, bend) = if p < 0.2 {
                (-10.0 * crouch(p / 0.2), crouch(p / 0.2))
            } else if p < 0.8 {
                let u = (p - 0.2) / 0.6;
                (
                    70.0 * 4.0 * u * (1.0 - u),
                    0.6 * (std::f64::consts::PI * u).sin(),
                )
            } else {
                let u = (p - 0.8) / 0.2;
                (-8.0 * crouch(u), crouch(u))
            };
            let w = w.min(1.0);
            motion[ROOT] = translation(0.0, -lift * w);
            // Knees bend outward (front view), feet tuck back under the hips.
            motion[THIGH_L] = rotation(0.5 * bend * w);
            motion[THIGH_R] = rotation(-0.5 * bend * w);
            motion[SHIN_L] = rotation(-0.9 * bend * w);
            motion[SHIN_R] = rotation(0.9 * bend * w);
            let arms = if (0.2..0.8).contains(&p) { 1.0 } else { 0.0 } * w;
            motion[UPPER_ARM_L] = rotation(0.9 * arms * bend.max(0.3));
            motion[UPPER_ARM_R] = rotation(-0.9 * arms * bend.max(0.3));
        }
        Gesture::Clap => {
            // Hands meet in front of the chest four times.
            let beat = 0.5 * (1.0 - (TAU * 4.0 * p).cos()) * trail.max(0.0);
            motion[UPPER_ARM_R] = rotation(0.55 * lead);
            motion[UPPER_ARM_L] = rotation(-0.55 * lead);
            motion[FOREARM_R] = rotation((1.45 + 0.25 * beat) * trail);
            motion[FOREARM_L] = rotation(-(1.45 + 0.25 * beat) * trail);
        }
        Gesture::Stumble => {
            // The toe catches: the back leg kicks up, the body pitches
            // forward and the arms fling out, then it recovers.
            let trip = (std::f64::consts::PI * p).sin().powi(2) * w;
            let catch = (std::f64::consts::PI * (p - 0.15).max(0.0) / 0.85)
                .sin()
                .max(0.0)
                * w;
            motion[ROOT] = translation(0.0, 10.0 * catch);
            motion[TORSO] = rotation(0.45 * trip);
            motion[HEAD] = rotation(-0.25 * trip);
            motion[THIGH_R] = rotation(0.7 * trip);
            motion[SHIN_R] = rotation(0.9 * trip);
            motion[THIGH_L] = rotation(-0.35 * catch);
            motion[SHIN_L] = rotation(0.5 * catch);
            motion[UPPER_ARM_L] = rotation(-1.7 * lead);
            motion[UPPER_ARM_R] = rotation(-1.3 * trail);
            motion[FOREARM_L] = rotation(-0.4 * trail);
        }
        Gesture::Think => {
            // Hand to the chin, head tilted.
            motion[UPPER_ARM_R] = rotation(1.4 * lead);
            motion[FOREARM_R] = rotation(2.07 * trail);
            motion[HEAD] = rotation(0.1 * trail);
        }
    }
    if principles.squash_stretch > 0.0 {
        let (x, y) = squash_scale(gesture_stretch(gesture, p, w, principles));
        motion[TORSO].scale = Vec2::new(x, y);
    }
    motion
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{gesture_envelope, GESTURES};

    #[test]
    fn standing_and_zero_weight_are_rest() {
        assert_eq!(rig_gait(Gait::Run, 0.3, 1.0, 0.0), rest());
        assert_eq!(rig_gait(Gait::Walk, f64::NAN, 1.0, 1.0), rest());
        assert_eq!(rig_gesture(Gesture::Wave, 0.5, 0.0), rest());
        assert_eq!(rig_gesture(Gesture::Wave, f64::NAN, 1.0), rest());
    }

    #[test]
    fn gait_cycles_repeat_and_scale_with_gait() {
        for gait in [Gait::Walk, Gait::Jog, Gait::Run] {
            let start = gait.cycle_seconds() + 0.1;
            let a = rig_gait(gait, start, 1.0, 1.0);
            let b = rig_gait(gait, start + gait.cycle_seconds() * 3.0, 1.0, 1.0);
            for (x, y) in a.iter().zip(&b) {
                assert!((x.rotation - y.rotation).abs() < 1e-9);
                assert!((x.translation.y - y.translation.y).abs() < 1e-9);
            }
        }
        let quarter = |gait: Gait| rig_gait(gait, gait.cycle_seconds() * 1.25, 1.0, 1.0);
        assert!(quarter(Gait::Run)[THIGH_L].rotation < quarter(Gait::Jog)[THIGH_L].rotation);
        assert!(quarter(Gait::Jog)[THIGH_L].rotation < quarter(Gait::Walk)[THIGH_L].rotation);
        // Left leg forward (toward +x means negative rotation) at a quarter cycle.
        assert!(quarter(Gait::Walk)[THIGH_L].rotation < 0.0);
        assert!(quarter(Gait::Walk)[UPPER_ARM_L].rotation > 0.0);
    }

    #[test]
    fn travelling_backward_mirrors_the_cycle() {
        let forward = rig_gait(Gait::Run, 1.13, 1.0, 1.0);
        let backward = rig_gait(Gait::Run, 1.13, -1.0, 1.0);
        for (f, b) in forward.iter().zip(&backward) {
            assert_eq!(f.rotation, -b.rotation);
            assert_eq!(f.translation.y, b.translation.y);
        }
        assert!(forward[TORSO].rotation > 0.0, "leans toward travel");
    }

    #[test]
    fn every_gesture_moves_and_is_bounded() {
        for name in GESTURES {
            let gesture = Gesture::parse(name).unwrap();
            let mid = rig_gesture(gesture, 0.4, gesture_envelope(0.4));
            assert_ne!(mid, rest(), "{name}");
            assert!(mid.iter().all(|o| o.is_valid() && o.rotation.abs() < 3.2));
        }
        let wave = rig_gesture(Gesture::Wave, 0.5, 1.0);
        assert!(wave[UPPER_ARM_R].rotation < -2.0, "right arm raised");
    }
    #[test]
    fn moves_ease_in_from_rest() {
        assert_eq!(rig_gait(Gait::Walk, 0.0, 1.0, 1.0), rest());
        let early = rig_gait(Gait::Run, 0.02, 1.0, 1.0);
        let full = rig_gait(Gait::Run, 0.02 + 2.0 * Gait::Run.cycle_seconds(), 1.0, 1.0);
        assert!(early[SHIN_L].rotation.abs() < 0.1 * full[SHIN_L].rotation.abs());
    }

    #[test]
    fn acting_curve_anticipates_overshoots_and_follows_through() {
        assert_eq!(acting_curve(0.0), 0.0);
        assert_eq!(acting_curve(1.0), 0.0);
        assert_eq!(acting_curve(f64::NAN), 0.0);
        assert!(acting_curve(ANTICIPATION_END / 2.0) < 0.0, "anticipation");
        let peak = (0..1000)
            .map(|i| acting_curve(i as f64 / 1000.0))
            .fold(f64::MIN, f64::max);
        assert!(peak > 1.02 && peak < 1.2, "overshoot {peak}");
        assert_eq!(acting_curve(0.5), 1.0, "hold");
        assert!(acting_curve(0.97) < 0.0, "follow-through");
        // Continuous: no step larger than the steepest slope allows.
        for i in 0..2000 {
            let (a, b) = (i as f64 / 2000.0, (i + 1) as f64 / 2000.0);
            assert!((acting_curve(a) - acting_curve(b)).abs() < 0.01, "{a}");
        }
    }

    #[test]
    fn forearm_overlaps_its_upper_arm() {
        // Early on the arm leads; late on the forearm is still moving.
        let early = rig_gesture(Gesture::Point, 0.2, 1.0);
        assert!(early[UPPER_ARM_R].rotation.abs() > 0.0);
        let d = Principles::DEFAULT;
        assert!(follower(0.2, &d) < driver(0.2, &d));
        let late = rig_gesture(Gesture::Shrug, 0.95, 1.0);
        assert!(driver(0.95, &d) == 0.0 && late[UPPER_ARM_L].rotation == 0.0);
        assert!(late[FOREARM_L].rotation != 0.0, "forearm follows through");
    }

    /// The acting curve exactly as it was with fixed constants.
    fn legacy_acting_curve(p: f64) -> f64 {
        if !p.is_finite() || p <= 0.0 || p >= 1.0 {
            return 0.0;
        }
        if p < 0.12 {
            return -0.12 * (std::f64::consts::PI * p / 0.12).sin();
        }
        if p < 0.38 {
            let u = (p - 0.12) / (0.38 - 0.12) - 1.0;
            return 1.0 + (1.3 + 1.0) * u * u * u + 1.3 * u * u;
        }
        if p < 0.7 {
            return 1.0;
        }
        let u = (p - 0.7) / (1.0 - 0.7);
        1.0 - smoothstep(u) - 0.25 * (std::f64::consts::PI * u).sin() * u * u
    }

    #[test]
    fn default_principles_match_the_old_constants_bit_for_bit() {
        for i in -10..=10_010 {
            let p = i as f64 / 10_000.0;
            assert_eq!(
                acting_curve(p).to_bits(),
                legacy_acting_curve(p).to_bits(),
                "{p}"
            );
            let legacy_driver = legacy_acting_curve(p / (1.0 - 0.08));
            let legacy_follower = legacy_acting_curve((p - 0.08) / (1.0 - 0.08));
            let d = Principles::DEFAULT;
            assert_eq!(driver(p, &d).to_bits(), legacy_driver.to_bits());
            assert_eq!(follower(p, &d).to_bits(), legacy_follower.to_bits());
        }
        // Default gestures keep unit scale everywhere.
        for name in GESTURES {
            let gesture = Gesture::parse(name).unwrap();
            for i in 0..=100 {
                let motion = rig_gesture(gesture, i as f64 / 100.0, 1.0);
                assert!(motion.iter().all(|o| o.scale == Vec2::new(1.0, 1.0)));
            }
        }
    }

    #[test]
    fn principles_change_the_curve() {
        let peak = |p: &Principles| {
            (0..1000)
                .map(|i| acting_curve_with(i as f64 / 1000.0, p))
                .fold(f64::MIN, f64::max)
        };
        let none = Principles {
            anticipation: 0.0,
            overshoot: 0.0,
            follow_through: 0.0,
            ..Principles::DEFAULT
        };
        assert!(peak(&none) <= 1.0, "no overshoot");
        assert!(acting_curve_with(0.06, &none) == 0.0, "no anticipation");
        assert!(acting_curve_with(0.97, &none) >= 0.0, "no follow-through");
        let cartoon = Principles::preset("cartoon").unwrap();
        assert!(peak(&cartoon) > peak(&Principles::DEFAULT));
        assert!(acting_curve_with(0.06, &cartoon) < acting_curve(0.06));
        assert!(acting_curve_with(0.97, &cartoon) < acting_curve(0.97));
        // Invalid principles fall back to the defaults instead of breaking the rig.
        let bad = Principles {
            overlap: 2.0,
            ..Principles::DEFAULT
        };
        assert_eq!(
            rig_gesture_with(Gesture::Point, 0.3, 1.0, &bad),
            rig_gesture(Gesture::Point, 0.3, 1.0)
        );
    }

    #[test]
    fn squash_and_stretch_preserve_area_and_stay_bounded() {
        let cartoon = Principles {
            squash_stretch: 0.3,
            ..Principles::preset("cartoon").unwrap()
        };
        let (mut stretched, mut squashed) = (false, false);
        for name in GESTURES {
            let gesture = Gesture::parse(name).unwrap();
            for i in 0..=400 {
                let motion = rig_gesture_with(gesture, i as f64 / 400.0, 1.0, &cartoon);
                let scale = motion[TORSO].scale;
                assert!((scale.x * scale.y - 1.0).abs() < 1e-9, "{name}");
                assert!((0.7 - 1e-9..=1.3 + 1e-9).contains(&scale.y), "{name}");
                stretched |= scale.y > 1.05;
                squashed |= scale.y < 0.95;
                // Limbs only rotate, so the stick figure stays attached.
                for joint in [UPPER_ARM_L, FOREARM_L, UPPER_ARM_R, FOREARM_R] {
                    assert_eq!(motion[joint].scale, Vec2::new(1.0, 1.0));
                }
                assert!(motion.iter().all(|o| o.is_valid()));
            }
        }
        assert!(stretched && squashed);
        // The jump stretches on take-off and squashes on landing.
        assert!(
            rig_gesture_with(Gesture::Jump, 0.22, 1.0, &cartoon)[TORSO]
                .scale
                .y
                > 1.1
        );
        assert!(
            rig_gesture_with(Gesture::Jump, 0.9, 1.0, &cartoon)[TORSO]
                .scale
                .y
                < 0.9
        );
    }

    #[test]
    fn moving_hold_breathes_deterministically_and_can_be_off() {
        assert_eq!(rig_moving_hold(10.0, 0.0), rest(), "off");
        assert_eq!(rig_moving_hold(f64::NAN, 1.0), rest());
        assert_eq!(rig_moving_hold(MOVING_HOLD_DELAY, 1.0), rest(), "fades in");
        let a = rig_moving_hold(7.3, 1.0);
        assert_eq!(a, rig_moving_hold(7.3, 1.0), "no randomness");
        assert_ne!(a, rest());
        // Small: a breath, not an action.
        assert!(a[TORSO].translation.y.abs() <= 1.5);
        assert!(a.iter().all(|o| o.rotation.abs() <= 0.03 + 1e-12));
        // Continuous from the start of the hold.
        for i in 0..4000 {
            let (x, y) = (i as f64 / 400.0, (i + 1) as f64 / 400.0);
            let (m, n) = (rig_moving_hold(x, 1.0), rig_moving_hold(y, 1.0));
            assert!((m[TORSO].translation.y - n[TORSO].translation.y).abs() < 0.05);
            assert!((m[HEAD].rotation - n[HEAD].rotation).abs() < 0.002);
        }
        let half = rig_moving_hold(7.3, 0.5);
        assert!((half[HEAD].rotation - 0.5 * a[HEAD].rotation).abs() < 1e-12);
    }

    #[test]
    fn seeking_matches_frame_by_frame_playback() {
        // Every layer is a pure function of time: a jump to any frame equals
        // reaching it by stepping through all earlier frames.
        let cartoon = Principles::preset("cartoon").unwrap();
        let fps = 24.0;
        let mut played = Vec::new();
        for frame in 0..=48 {
            let t = frame as f64 / fps;
            played.push((
                rig_gesture_with(Gesture::Jump, t / 2.0, 1.0, &cartoon),
                rig_moving_hold(t + 0.4, cartoon.moving_hold),
            ));
        }
        for frame in [48, 3, 31, 0, 17] {
            let t = frame as f64 / fps;
            let seeked = (
                rig_gesture_with(Gesture::Jump, t / 2.0, 1.0, &cartoon),
                rig_moving_hold(t + 0.4, cartoon.moving_hold),
            );
            assert_eq!(seeked, played[frame]);
        }
    }
}

#[cfg(test)]
mod locomotion_tests {
    use super::*;
    use crate::{MoveEase, GAITS};

    fn gaits() -> impl Iterator<Item = Gait> {
        GAITS.iter().map(|name| Gait::parse(name).unwrap())
    }

    #[test]
    fn every_cyclic_gait_moves_repeats_and_stays_bounded() {
        for gait in gaits().filter(|g| !g.is_leap()) {
            let cycle = gait.cycle_seconds();
            let a = rig_gait(gait, 2.0 * cycle + 0.07, 1.0, 1.0);
            let b = rig_gait(gait, 5.0 * cycle + 0.07, 1.0, 1.0);
            assert_ne!(a, rest(), "{gait:?}");
            for (x, y) in a.iter().zip(&b) {
                assert!((x.rotation - y.rotation).abs() < 1e-9, "{gait:?}");
                assert!((x.translation.y - y.translation.y).abs() < 1e-9, "{gait:?}");
            }
            for i in 0..=200 {
                let m = rig_gait(gait, i as f64 * cycle / 50.0, -1.0, 1.0);
                assert!(m.iter().all(|o| o.is_valid() && o.rotation.abs() < 3.2));
            }
        }
    }

    #[test]
    fn gait_shapes_read_as_their_names() {
        let at =
            |gait: Gait, phase: f64| rig_gait(gait, gait.cycle_seconds() * (3.0 + phase), 1.0, 1.0);
        // Sneak: crouched (root lower than a walk) with the hands up front.
        let (sneak, walk) = (at(Gait::Sneak, 0.0), at(Gait::Walk, 0.0));
        assert!(sneak[ROOT].translation.y > walk[ROOT].translation.y + 10.0);
        assert!(sneak[UPPER_ARM_L].rotation < -0.5 && sneak[UPPER_ARM_R].rotation < -0.5);
        // Tiptoe rises above the walk.
        assert!(at(Gait::Tiptoe, 0.5)[ROOT].translation.y < walk[ROOT].translation.y);
        // Sprint swings the legs further and leans more than a run.
        let (sprint, run) = (at(Gait::Sprint, 0.25), at(Gait::Run, 0.25));
        assert!(sprint[THIGH_L].rotation < run[THIGH_L].rotation);
        assert!(sprint[TORSO].rotation > run[TORSO].rotation);
        // Limp: the right leg swings less than the left, and the root sinks
        // deeper on one step than on the other.
        let limp = at(Gait::Limp, 0.25);
        assert!(limp[THIGH_R].rotation.abs() < limp[THIGH_L].rotation.abs());
        let (one, other) = (at(Gait::Limp, 0.25), at(Gait::Limp, 0.75));
        assert!((one[ROOT].translation.y - other[ROOT].translation.y).abs() > 3.0);
        // Backwards leans against the travel direction.
        assert!(at(Gait::Backwards, 0.0)[TORSO].rotation < 0.0);
        // The leap is not a step cycle.
        assert_eq!(rig_gait(Gait::Leap, 1.3, 1.0, 1.0), rest());
    }

    #[test]
    fn leap_crouches_flies_and_lands_at_rest() {
        assert_eq!(rig_leap(0.0, 1.0, 1.0), rest());
        assert_eq!(rig_leap(1.0, 1.0, 1.0), rest());
        assert_eq!(rig_leap(0.5, 1.0, 0.0), rest());
        assert_eq!(rig_leap(f64::NAN, 1.0, 1.0), rest());
        assert!(
            rig_leap(0.09, 1.0, 1.0)[ROOT].translation.y > 10.0,
            "crouch"
        );
        let top = rig_leap(0.5, 1.0, 1.0);
        assert!(top[ROOT].translation.y < -100.0, "flight");
        assert!(
            top[THIGH_L].rotation < 0.0 && top[THIGH_R].rotation > 0.0,
            "legs split"
        );
        let back = rig_leap(0.5, -1.0, 1.0);
        assert_eq!(back[THIGH_L].rotation, -top[THIGH_L].rotation);
        // Continuous across take-off and landing.
        for i in 0..1000 {
            let (a, b) = (
                rig_leap(i as f64 / 1000.0, 1.0, 1.0),
                rig_leap((i + 1) as f64 / 1000.0, 1.0, 1.0),
            );
            assert!(
                (a[ROOT].translation.y - b[ROOT].translation.y).abs() < 2.0,
                "{i}"
            );
        }
    }

    #[test]
    fn move_ease_is_monotonic_and_exact_at_the_ends() {
        for name in crate::MOVE_EASES {
            let ease = MoveEase::parse(name).unwrap();
            assert_eq!(ease.name(), name);
            assert_eq!(ease.apply(0.0), 0.0, "{name}");
            assert!((ease.apply(1.0) - 1.0).abs() < 1e-12, "{name}");
            let mut last = 0.0;
            for i in 1..=1000 {
                let p = ease.apply(i as f64 / 1000.0);
                assert!(p >= last && p - last < 0.003, "{name} at {i}");
                last = p;
            }
        }
        assert_eq!(MoveEase::Linear.apply(0.3), 0.3);
        assert!(MoveEase::InOut.apply(0.1) < 0.1 && MoveEase::InOut.apply(0.9) > 0.9);
        assert!(MoveEase::In.apply(0.1) < 0.1 && MoveEase::Out.apply(0.1) > 0.1);
        assert!((MoveEase::InOut.apply(0.5) - 0.5).abs() < 1e-12);
        assert_eq!(MoveEase::InOut.apply(f64::NAN), 0.0);
        assert_eq!(MoveEase::parse("bounce"), None);
    }

    #[test]
    fn body_poses_blend_continuously_and_sit_matches_the_old_layer() {
        for name in BODY_POSES {
            let pose = BodyPose::parse(name).unwrap();
            assert_eq!(pose.name(), name);
            for profile in [true, false] {
                let m = rig_body_pose(pose, profile);
                assert!(m.iter().all(|o| o.is_valid()));
                assert_eq!(m == rest(), pose == BodyPose::Stand, "{name}");
            }
        }
        assert_eq!(rig_body_pose(BodyPose::Sit, true), rig_sit(1.0, true));
        // Sitting down from standing matches the legacy sit layer.
        for i in 0..=10 {
            let t = i as f64 * SIT_SECONDS / 10.0;
            let new = rig_pose_at(&[(0.0, BodyPose::Sit)], t, true);
            let old = rig_sit(t / SIT_SECONDS, true);
            for (a, b) in new.iter().zip(&old) {
                assert!((a.rotation - b.rotation).abs() < 1e-12);
                assert!((a.translation.y - b.translation.y).abs() < 1e-12);
            }
        }
        // Lying down rotates the whole body onto the floor.
        let lie = rig_pose_at(&[(1.0, BodyPose::Lie)], 5.0, true);
        assert!(lie[ROOT].rotation > 1.4 && lie[ROOT].translation.y > 80.0);
        // A change that interrupts another starts from the pose in flight.
        let changes = [(0.0, BodyPose::Crouch), (0.1, BodyPose::Kneel)];
        let before = rig_pose_at(&changes[..1], 0.1, true);
        let at_change = rig_pose_at(&changes, 0.1, true);
        for (a, b) in before.iter().zip(&at_change) {
            assert!((a.rotation - b.rotation).abs() < 1e-12);
            assert!((a.translation.y - b.translation.y).abs() < 1e-12);
        }
        // Later changes are ignored until their time.
        assert_eq!(
            rig_pose_at(
                &[(0.0, BodyPose::Kneel), (9.0, BodyPose::Stand)],
                3.0,
                false
            ),
            rig_body_pose(BodyPose::Kneel, false)
        );
        assert_eq!(rig_pose_at(&[], 3.0, false), rest());
    }
}

#[cfg(test)]
mod mood_tests {
    use super::*;

    #[test]
    fn moods_colour_the_gait_and_neutral_changes_nothing() {
        let at = |mood| rig_gait_with_mood(Gait::Walk, 1.0 + 0.25 * 2.0 / 3.0, 1.0, 1.0, mood);
        assert_eq!(
            at(GaitMood::Neutral),
            rig_gait(Gait::Walk, 1.0 + 0.25 * 2.0 / 3.0, 1.0, 1.0)
        );
        let (neutral, sad, angry, happy) = (
            at(GaitMood::Neutral),
            at(GaitMood::Sad),
            at(GaitMood::Angry),
            at(GaitMood::Happy),
        );
        assert!(sad[UPPER_ARM_L].rotation.abs() < neutral[UPPER_ARM_L].rotation.abs());
        assert!(sad[HEAD].rotation > neutral[HEAD].rotation, "head hangs");
        assert!(angry[UPPER_ARM_L].rotation.abs() > neutral[UPPER_ARM_L].rotation.abs());
        assert!(happy[HEAD].rotation < neutral[HEAD].rotation, "head up");
        // Bounce: compare the root at passing (legs together).
        let passing = |mood| {
            rig_gait_with_mood(Gait::Walk, 2.0, 1.0, 1.0, mood)[ROOT]
                .translation
                .y
        };
        assert!(passing(GaitMood::Happy) < passing(GaitMood::Neutral));
        assert!(passing(GaitMood::Sad) > passing(GaitMood::Neutral));
        assert_eq!(GaitMood::from_emotion("sad"), GaitMood::Sad);
        assert_eq!(GaitMood::from_emotion("surprised"), GaitMood::Neutral);
    }

    #[test]
    fn stumble_pitches_forward_and_fall_ends_lying_quickly() {
        let mid = rig_gesture(Gesture::Stumble, 0.4, 1.0);
        assert!(mid[TORSO].rotation > 0.2 && mid[UPPER_ARM_L].rotation < -0.5);
        assert_eq!(rig_gesture(Gesture::Stumble, 0.0, 1.0)[TORSO].rotation, 0.0);
        assert_eq!(
            rig_body_pose(BodyPose::Fall, true),
            rig_body_pose(BodyPose::Lie, true)
        );
        assert!(BodyPose::Fall.seconds() < BodyPose::Lie.seconds());
        let fallen = rig_pose_at(&[(0.0, BodyPose::Fall)], BodyPose::Fall.seconds(), true);
        assert_eq!(fallen, rig_body_pose(BodyPose::Lie, true));
    }
}
