//! Acting layers for custom rigs with the standard limb joints: the body
//! posture an emotion gives at a given intensity, a character's own standing
//! posture, and gesture energy.
//!
//! Like the rest of `rig_motion`, every layer is parent-local, additive and a
//! pure function of its inputs, so seeking matches playback. Postures only
//! move the torso, head and arms; the root and legs stay planted.
use crate::rig_motion::{blend_rig_motion, RigMotion};
use crate::{Transform2, Vec2, POSTURE_BLEND_SECONDS};

const TORSO: usize = 1;
const HEAD: usize = 2;
const UPPER_ARM_L: usize = 3;
const FOREARM_L: usize = 4;
const UPPER_ARM_R: usize = 5;
const FOREARM_R: usize = 6;

/// Largest effective intensity: a full emotion (1) on a very expressive character.
pub const MAX_ACTING_INTENSITY: f64 = 1.5;
/// Below this effective intensity the face keeps its neutral drawing: a hint
/// of an emotion shows in the body only.
pub const FACE_INTENSITY_THRESHOLD: f64 = 0.3;

fn rest() -> RigMotion {
    [Transform2::default(); 11]
}

fn offset(x: f64, y: f64, rotation: f64) -> Transform2 {
    Transform2 {
        translation: Vec2::new(x, y),
        rotation,
        ..Default::default()
    }
}

/// Posture of `emotion` at intensity 1. Rotations follow the rig convention: a
/// positive rotation swings a hanging limb toward -x, so `+` takes the left
/// arm outward and the right arm inward.
fn emotion_posture(emotion: &str) -> RigMotion {
    let mut m = rest();
    match emotion {
        "happy" => {
            // Chest and head lift, arms open a little.
            m[TORSO] = offset(0.0, -3.0, 0.0);
            m[HEAD] = offset(0.0, -3.0, -0.03);
            m[UPPER_ARM_L] = offset(0.0, 0.0, 0.12);
            m[UPPER_ARM_R] = offset(0.0, 0.0, -0.12);
        }
        "sad" => {
            // Shoulders sink, head drops and tilts, arms hang close.
            m[TORSO] = offset(0.0, 4.0, 0.0);
            m[HEAD] = offset(0.0, 8.0, 0.08);
            m[UPPER_ARM_L] = offset(0.0, 0.0, -0.1);
            m[UPPER_ARM_R] = offset(0.0, 0.0, 0.1);
        }
        "angry" => {
            // Head down and forward, elbows out, forearms drawn in (fists).
            m[TORSO] = offset(0.0, 2.0, 0.0);
            m[HEAD] = offset(0.0, 4.0, 0.0);
            m[UPPER_ARM_L] = offset(0.0, 0.0, 0.24);
            m[UPPER_ARM_R] = offset(0.0, 0.0, -0.24);
            m[FOREARM_L] = offset(0.0, 0.0, -0.4);
            m[FOREARM_R] = offset(0.0, 0.0, 0.4);
        }
        "surprised" => {
            // Whole upper body pulls up and back, arms fly out.
            m[TORSO] = offset(0.0, -4.0, 0.0);
            m[HEAD] = offset(0.0, -5.0, 0.0);
            m[UPPER_ARM_L] = offset(0.0, 0.0, 0.38);
            m[UPPER_ARM_R] = offset(0.0, 0.0, -0.38);
            m[FOREARM_L] = offset(0.0, 0.0, 0.35);
            m[FOREARM_R] = offset(0.0, 0.0, -0.35);
        }
        _ => {}
    }
    m
}

/// Scale the translations and rotations of a layer (squash/stretch scales are
/// left alone). Non-finite factors give the layer unchanged.
pub fn scale_rig_motion(motion: &RigMotion, factor: f64) -> RigMotion {
    if !factor.is_finite() {
        return *motion;
    }
    std::array::from_fn(|i| {
        let t = motion[i];
        Transform2 {
            translation: Vec2::new(t.translation.x * factor, t.translation.y * factor),
            rotation: t.rotation * factor,
            scale: t.scale,
        }
    })
}

/// Emotion posture from timed changes `(emotion, seconds since, intensity)`,
/// oldest first, like the built-in humanoid's posture: each change blends
/// from the posture in flight toward its own over `POSTURE_BLEND_SECONDS`
/// until the next change starts. Intensity is clamped to
/// `0..=MAX_ACTING_INTENSITY`; neutral blends back to rest.
pub fn rig_emotion_posture(changes: &[(&str, f64, f64)]) -> RigMotion {
    let progress = |seconds: f64| {
        if seconds.is_finite() && seconds >= 0.0 {
            (seconds / POSTURE_BLEND_SECONDS).min(1.0)
        } else {
            1.0
        }
    };
    let mut motion = rest();
    for (i, &(emotion, since, intensity)) in changes.iter().enumerate() {
        let until = changes.get(i + 1).map_or(0.0, |next| next.1);
        let intensity = if intensity.is_finite() {
            intensity.clamp(0.0, MAX_ACTING_INTENSITY)
        } else {
            1.0
        };
        let target = scale_rig_motion(&emotion_posture(emotion), intensity);
        motion = blend_rig_motion(&motion, &target, progress(since - until));
    }
    motion
}

/// A character's own standing posture, `-1..=1`: negative slouches (head and
/// shoulders down, arms in), positive stands tall (chest and head up, arms
/// slightly open). Zero, or a non-finite value, is the rig's rest pose.
pub fn rig_character_posture(posture: f64) -> RigMotion {
    let mut m = rest();
    if !posture.is_finite() || posture == 0.0 {
        return m;
    }
    let p = posture.clamp(-1.0, 1.0);
    if p > 0.0 {
        m[TORSO] = offset(0.0, -3.0 * p, 0.0);
        m[HEAD] = offset(0.0, -3.0 * p, -0.02 * p);
        m[UPPER_ARM_L] = offset(0.0, 0.0, 0.06 * p);
        m[UPPER_ARM_R] = offset(0.0, 0.0, -0.06 * p);
    } else {
        let s = -p;
        m[TORSO] = offset(0.0, 4.0 * s, 0.0);
        m[HEAD] = offset(0.0, 7.0 * s, 0.05 * s);
        m[UPPER_ARM_L] = offset(0.0, 0.0, -0.07 * s);
        m[UPPER_ARM_R] = offset(0.0, 0.0, 0.07 * s);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_rest(m: &RigMotion) -> bool {
        m.iter().all(|t| *t == Transform2::default())
    }

    #[test]
    fn neutral_and_zero_intensity_stay_at_rest() {
        assert!(is_rest(&rig_emotion_posture(&[])));
        assert!(is_rest(&rig_emotion_posture(&[("neutral", 3.0, 1.0)])));
        assert!(is_rest(&rig_emotion_posture(&[("angry", 3.0, 0.0)])));
        assert!(is_rest(&rig_character_posture(0.0)));
        assert!(is_rest(&rig_character_posture(f64::NAN)));
    }

    #[test]
    fn intensity_scales_the_settled_posture() {
        let half = rig_emotion_posture(&[("sad", 2.0, 0.5)]);
        let full = rig_emotion_posture(&[("sad", 2.0, 1.0)]);
        assert!((full[HEAD].translation.y - 8.0).abs() < 1e-12);
        assert!((half[HEAD].translation.y - 4.0).abs() < 1e-12);
        // Clamped to the maximum.
        let over = rig_emotion_posture(&[("sad", 2.0, 9.0)]);
        assert!((over[HEAD].translation.y - 8.0 * MAX_ACTING_INTENSITY).abs() < 1e-12);
    }

    #[test]
    fn a_change_blends_in_continuously() {
        let at = |since: f64| {
            rig_emotion_posture(&[("happy", since, 1.0)])[HEAD]
                .translation
                .y
        };
        assert_eq!(at(0.0), 0.0);
        let mid = at(POSTURE_BLEND_SECONDS / 2.0);
        assert!(mid < 0.0 && mid > -3.0);
        assert!((at(POSTURE_BLEND_SECONDS) + 3.0).abs() < 1e-12);
        // A second change continues from the posture in flight.
        let before = rig_emotion_posture(&[("happy", 0.1, 1.0)]);
        let after = rig_emotion_posture(&[("happy", 0.1, 1.0), ("sad", 0.0, 1.0)]);
        assert_eq!(before, after);
    }

    #[test]
    fn angry_draws_the_elbows_out_and_surprise_lifts_the_body() {
        let angry = rig_emotion_posture(&[("angry", 1.0, 1.0)]);
        assert!(angry[UPPER_ARM_L].rotation > 0.0 && angry[UPPER_ARM_R].rotation < 0.0);
        let surprised = rig_emotion_posture(&[("surprised", 1.0, 1.0)]);
        assert!(surprised[TORSO].translation.y < 0.0);
    }

    #[test]
    fn slouch_and_tall_postures_mirror_each_other() {
        let tall = rig_character_posture(1.0);
        let slouch = rig_character_posture(-1.0);
        assert!(tall[HEAD].translation.y < 0.0);
        assert!(slouch[HEAD].translation.y > 0.0);
        assert_eq!(rig_character_posture(4.0), tall);
    }

    #[test]
    fn scaling_keeps_squash_and_handles_bad_factors() {
        let mut m = rig_emotion_posture(&[("angry", 1.0, 1.0)]);
        m[TORSO].scale = Vec2::new(1.1, 0.9);
        let doubled = scale_rig_motion(&m, 2.0);
        assert_eq!(doubled[TORSO].scale, m[TORSO].scale);
        assert!((doubled[UPPER_ARM_L].rotation - 0.48).abs() < 1e-12);
        assert_eq!(scale_rig_motion(&m, f64::INFINITY), m);
    }
}
