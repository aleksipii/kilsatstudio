//! Procedural gait and gesture offsets for custom rigs that use the standard
//! limb joint names (`root`, `torso`, `head`, `upper_arm_l`, `forearm_l`,
//! `thigh_l`, `shin_l` and their `_r` mirrors).
//!
//! Offsets are parent-local and additive over the rest pose, like authored
//! joint keys: translations and rotations add. Rotations follow the rig
//! convention (y down): a positive rotation swings a hanging limb toward -x.
//! Every value is a pure function of its inputs, so seeking is exact.
use crate::{Gait, Gesture, Transform2, Vec2};
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

/// Gait offsets `seconds` into an active move. `travel` is the local
/// horizontal direction (+1 toward +x, -1 toward -x); zero counts as +x.
/// `weight` scales the whole layer and is clamped to `0..=1`.
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
    let phase = TAU * seconds.rem_euclid(gait.cycle_seconds()) / gait.cycle_seconds();
    let (sin, cos) = phase.sin_cos();
    // Left leg forward while sin > 0; a forward-swinging leg bends its knee.
    motion[THIGH_L] = rotation(-s.leg * sin * weight);
    motion[THIGH_R] = rotation(s.leg * sin * weight);
    motion[SHIN_L] = rotation((0.08 + s.knee * cos.max(0.0)) * weight);
    motion[SHIN_R] = rotation((0.08 + s.knee * (-cos).max(0.0)) * weight);
    // Arms swing opposite to the legs.
    motion[UPPER_ARM_L] = rotation(s.arm * sin * weight);
    motion[UPPER_ARM_R] = rotation(-s.arm * sin * weight);
    motion[FOREARM_L] = rotation((s.elbow - 0.15 * sin.max(0.0)) * weight);
    motion[FOREARM_R] = rotation((s.elbow - 0.15 * (-sin).max(0.0)) * weight);
    // Highest at passing (legs together), lowest at contact.
    motion[ROOT] = translation(0.0, -s.bob * (1.0 - sin.abs()) * weight);
    motion[TORSO] = rotation(s.lean * weight);
    motion[HEAD] = rotation(-0.5 * s.lean * weight);
    if travel < 0.0 {
        mirror(&mut motion);
    }
    motion
}

/// Gesture offsets at normalized `progress` with envelope `weight` in `0..=1`.
pub fn rig_gesture(gesture: Gesture, progress: f64, weight: f64) -> RigMotion {
    let mut motion = rest();
    if !progress.is_finite() || !weight.is_finite() || weight <= 0.0 {
        return motion;
    }
    let w = weight.min(1.0);
    let p = progress.clamp(0.0, 1.0);
    match gesture {
        Gesture::Wave => {
            motion[UPPER_ARM_R] = rotation(-2.5 * w);
            motion[FOREARM_R] = rotation((-0.35 + 0.45 * (TAU * 3.0 * p).sin()) * w);
        }
        Gesture::Point => {
            motion[UPPER_ARM_R] = rotation(-1.45 * w);
            motion[FOREARM_R] = rotation(0.1 * w);
        }
        Gesture::Nod => {
            let dip = (TAU * p).sin().powi(2);
            motion[HEAD] = translation(0.0, 7.0 * dip * w);
        }
        Gesture::Shrug => {
            motion[UPPER_ARM_L] = Transform2 {
                translation: Vec2::new(0.0, -8.0 * w),
                rotation: 0.35 * w,
                ..Default::default()
            };
            motion[UPPER_ARM_R] = Transform2 {
                translation: Vec2::new(0.0, -8.0 * w),
                rotation: -0.35 * w,
                ..Default::default()
            };
            motion[FOREARM_L] = rotation(1.2 * w);
            motion[FOREARM_R] = rotation(-1.2 * w);
            motion[HEAD] = translation(0.0, 4.0 * w);
        }
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
            let a = rig_gait(gait, 0.1, 1.0, 1.0);
            let b = rig_gait(gait, 0.1 + gait.cycle_seconds() * 3.0, 1.0, 1.0);
            for (x, y) in a.iter().zip(&b) {
                assert!((x.rotation - y.rotation).abs() < 1e-9);
                assert!((x.translation.y - y.translation.y).abs() < 1e-9);
            }
        }
        let quarter = |gait: Gait| rig_gait(gait, gait.cycle_seconds() / 4.0, 1.0, 1.0);
        assert!(quarter(Gait::Run)[THIGH_L].rotation < quarter(Gait::Jog)[THIGH_L].rotation);
        assert!(quarter(Gait::Jog)[THIGH_L].rotation < quarter(Gait::Walk)[THIGH_L].rotation);
        // Left leg forward (toward +x means negative rotation) at a quarter cycle.
        assert!(quarter(Gait::Walk)[THIGH_L].rotation < 0.0);
        assert!(quarter(Gait::Walk)[UPPER_ARM_L].rotation > 0.0);
    }

    #[test]
    fn travelling_backward_mirrors_the_cycle() {
        let forward = rig_gait(Gait::Run, 0.13, 1.0, 1.0);
        let backward = rig_gait(Gait::Run, 0.13, -1.0, 1.0);
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
}
