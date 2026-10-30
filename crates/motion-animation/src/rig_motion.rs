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
}

fn shape(gait: Gait) -> GaitShape {
    match gait {
        Gait::Walk => GaitShape {
            leg: 0.42,
            knee: 0.55,
            arm: 0.35,
            elbow: -0.25,
            bob: 4.0,
            lean: 0.03,
        },
        Gait::Jog => GaitShape {
            leg: 0.55,
            knee: 0.95,
            arm: 0.5,
            elbow: -1.2,
            bob: 8.0,
            lean: 0.09,
        },
        Gait::Run => GaitShape {
            leg: 0.75,
            knee: 1.35,
            arm: 0.7,
            elbow: -1.5,
            bob: 12.0,
            lean: 0.18,
        },
    }
}

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

/// Gait offsets `seconds` into an active move. `travel` is the local
/// horizontal direction (+1 toward +x, -1 toward -x); zero counts as +x.
/// `weight` scales the whole layer and is clamped to `0..=1`.
///
/// The first half cycle eases in from rest (slow in), and forearms trail
/// their upper arms (overlapping action), so a move never pops into full
/// stride.
pub fn rig_gait(gait: Gait, seconds: f64, travel: f64, weight: f64) -> RigMotion {
    let mut motion = rest();
    let weight = if weight.is_finite() {
        weight.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if !seconds.is_finite() || seconds < 0.0 || weight == 0.0 {
        return motion;
    }
    let s = shape(gait);
    let cycle = gait.cycle_seconds();
    let weight = weight * smoothstep(seconds / (cycle * GAIT_EASE_IN_CYCLES));
    let phase = TAU * seconds.rem_euclid(cycle) / cycle;
    let (sin, cos) = phase.sin_cos();
    let drag = (phase - GAIT_DRAG).sin();
    // Left leg forward while sin > 0; a forward-swinging leg bends its knee.
    motion[THIGH_L] = rotation(-s.leg * sin * weight);
    motion[THIGH_R] = rotation(s.leg * sin * weight);
    motion[SHIN_L] = rotation((0.08 + s.knee * cos.max(0.0)) * weight);
    motion[SHIN_R] = rotation((0.08 + s.knee * (-cos).max(0.0)) * weight);
    // Arms swing opposite to the legs; forearms follow a beat later.
    motion[UPPER_ARM_L] = rotation(s.arm * sin * weight);
    motion[UPPER_ARM_R] = rotation(-s.arm * sin * weight);
    motion[FOREARM_L] = rotation((s.elbow - 0.15 * drag.max(0.0)) * weight);
    motion[FOREARM_R] = rotation((s.elbow - 0.15 * (-drag).max(0.0)) * weight);
    // Highest at passing (legs together), lowest at contact.
    motion[ROOT] = translation(0.0, -s.bob * (1.0 - sin.abs()) * weight);
    motion[TORSO] = rotation(s.lean * weight);
    // The head counters the lean and lags the bob slightly.
    motion[HEAD] = Transform2 {
        translation: Vec2::new(0.0, 0.15 * s.bob * (2.0 * phase - GAIT_DRAG).cos() * weight),
        rotation: -0.5 * s.lean * weight,
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
