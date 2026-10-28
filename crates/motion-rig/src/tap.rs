//! Finger taps on screen slots.
//!
//! When a screen shows a recording with tap markers, the character's free hand
//! (the hand not holding the device) reaches to the screen and presses near
//! the slot's centre: it approaches over `TAP_APPROACH_SECONDS`, touches the
//! centre exactly at the tap time, holds for `TAP_HOLD_SECONDS` and releases
//! over `TAP_RELEASE_SECONDS`. The press is solved with the ordinary two-bone
//! IK on top of whatever pose the arm already has, so outside the window the
//! arm is untouched. Every value is a pure function of its inputs (scene time,
//! tap times and the joint offsets), so seeking equals playing.
use crate::attachment::{origin, LimbJoints, STICK_LIMBS};
use crate::ik::{BendDirection, IkError};
use crate::props::{HandSide, CHARACTER_PROP_PREFIXES};
use crate::{PreparedRig, Transform2, Vec2};

/// Seconds from rest to contact.
pub const TAP_APPROACH_SECONDS: f64 = 0.22;
/// Seconds the fingertip stays on the glass after the tap time.
pub const TAP_HOLD_SECONDS: f64 = 0.06;
/// Seconds from the end of contact back to the arm's own pose.
pub const TAP_RELEASE_SECONDS: f64 = 0.18;

fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// How far the press has travelled toward the screen at `time`, in `[0, 1]`:
/// exactly 1 at every tap time, 0 outside every
/// `[tap - TAP_APPROACH_SECONDS, tap + TAP_HOLD_SECONDS + TAP_RELEASE_SECONDS]`
/// window. Overlapping presses take the stronger one, so close taps keep the
/// hand near the glass instead of snapping. Non-finite inputs give 0.
pub fn tap_envelope(time: f64, taps: &[f64]) -> f64 {
    if !time.is_finite() {
        return 0.0;
    }
    taps.iter()
        .filter(|t| t.is_finite())
        .map(|&tap| {
            let d = time - tap;
            if !(-TAP_APPROACH_SECONDS..=TAP_HOLD_SECONDS + TAP_RELEASE_SECONDS).contains(&d) {
                0.0
            } else if d <= 0.0 {
                // `(0 + A) / A` is exactly 1, so contact is exact at the tap.
                smoothstep((d + TAP_APPROACH_SECONDS) / TAP_APPROACH_SECONDS)
            } else if d <= TAP_HOLD_SECONDS {
                1.0
            } else {
                smoothstep(1.0 - (d - TAP_HOLD_SECONDS) / TAP_RELEASE_SECONDS)
            }
        })
        .fold(0.0, f64::max)
}

/// The arm that taps a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TapArm {
    pub hand: HandSide,
    pub limb: LimbJoints,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TapError {
    #[error("rig has no screen slot {0}")]
    UnknownSlot(String),
    /// No arm chain is free to press the screen; the taps are not animated.
    #[error("screen slot {slot}: no free arm can tap it ({reason})")]
    NoFreeArm { slot: String, reason: String },
    #[error("tap press for slot {slot}: {source}")]
    Ik { slot: String, source: IkError },
}

impl PreparedRig {
    /// Which hand, if any, carries joint `joint` (the joint itself or an
    /// ancestor is `hand_l` / `hand_r`).
    fn holding_hand(&self, joint: usize) -> Option<HandSide> {
        let joints = self.rig().joints();
        let mut current = Some(joint);
        while let Some(index) = current {
            let name = joints.get(index)?.name.as_str();
            for hand in [HandSide::Left, HandSide::Right] {
                if name == hand.joint() {
                    return Some(hand);
                }
            }
            current = joints[index].parent;
        }
        None
    }

    fn free_arm(&self, hand: HandSide) -> Result<LimbJoints, String> {
        let chain = match hand {
            HandSide::Left => &STICK_LIMBS[0],
            HandSide::Right => &STICK_LIMBS[1],
        };
        let limb = self.limb(chain).map_err(|e| e.to_string())?;
        let holding = self.part_ids_on(limb.end).any(|id| {
            id.starts_with(hand.part_prefix())
                || CHARACTER_PROP_PREFIXES
                    .iter()
                    .any(|prefix| id.starts_with(prefix))
        });
        if holding {
            return Err(format!("{} already holds a prop", hand.joint()));
        }
        Ok(limb)
    }

    /// The arm that presses slot `slot`: the other arm when a hand holds the
    /// device, otherwise the right arm, then the left. The arm must be a
    /// standard stick chain (`torso → upper_arm → forearm → hand`) whose hand
    /// holds nothing.
    pub fn tap_arm(&self, slot: &str) -> Result<TapArm, TapError> {
        let joint = self
            .screen_joint(slot)
            .ok_or_else(|| TapError::UnknownSlot(slot.into()))?;
        let candidates = match self.holding_hand(joint) {
            Some(holder) => vec![holder.other()],
            None => vec![HandSide::Right, HandSide::Left],
        };
        let mut reasons = Vec::new();
        for hand in candidates {
            match self.free_arm(hand) {
                Ok(limb) => return Ok(TapArm { hand, limb }),
                Err(reason) => reasons.push(reason),
            }
        }
        Err(TapError::NoFreeArm {
            slot: slot.into(),
            reason: reasons.join("; "),
        })
    }

    /// Actor-space centre of slot `slot` for the given offsets.
    pub fn screen_centre(
        &self,
        offsets: &[(usize, Transform2)],
        slot: &str,
    ) -> Result<Vec2, crate::Error> {
        let [a, _, c, _] = self.screen_corners(offsets, slot)?;
        Ok(Vec2::new(0.5 * (a.x + c.x), 0.5 * (a.y + c.y)))
    }

    /// Offsets that move `arm`'s hand `weight` of the way from where `offsets`
    /// put it to the centre of slot `slot` (see `tap_envelope`). Append them
    /// after `offsets`; they replace only the arm's upper and lower rotations.
    /// `None` when `weight` is not positive. The elbow stays on the side it
    /// already bends toward, so the arm does not flip.
    pub fn tap_press(
        &self,
        arm: TapArm,
        slot: &str,
        weight: f64,
        offsets: &[(usize, Transform2)],
    ) -> Result<Option<[(usize, Transform2); 2]>, TapError> {
        if weight.is_nan() || weight <= 0.0 {
            return Ok(None);
        }
        let weight = weight.min(1.0);
        let ik = |source: IkError| TapError::Ik {
            slot: slot.into(),
            source,
        };
        let contact = self
            .screen_centre(offsets, slot)
            .map_err(|e| ik(IkError::Pose(e)))?;
        let world = self
            .world_transforms(offsets)
            .map_err(|e| ik(IkError::Pose(e)))?;
        let (shoulder, elbow, hand) = (
            origin(world[arm.limb.upper]),
            origin(world[arm.limb.lower]),
            origin(world[arm.limb.end]),
        );
        let side = (contact.x - shoulder.x) * (elbow.y - shoulder.y)
            - (contact.y - shoulder.y) * (elbow.x - shoulder.x);
        let bend = if side >= 0.0 {
            BendDirection::Positive
        } else {
            BendDirection::Negative
        };
        let target = Vec2::new(
            hand.x + (contact.x - hand.x) * weight,
            hand.y + (contact.y - hand.y) * weight,
        );
        let solved = self
            .solve_limb_ik(arm.limb, target, bend, offsets)
            .map_err(ik)?;
        Ok(Some(solved.offsets))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RigDefinition, RigJointDefinition, RigidPartDefinition, ScreenSlotDefinition};

    fn joint(id: &str, parent: Option<&str>, x: f64, y: f64) -> RigJointDefinition {
        RigJointDefinition {
            id: id.into(),
            parent: parent.map(Into::into),
            x,
            y,
            rotation: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
        }
    }
    fn part(id: &str, joint: &str) -> RigidPartDefinition {
        RigidPartDefinition {
            id: id.into(),
            joint: joint.into(),
            points: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 20.0], [0.0, 20.0]],
            color: [200, 200, 200, 255],
            ..Default::default()
        }
    }
    /// A stick body with a phone glass on the right hand.
    fn rig() -> RigDefinition {
        RigDefinition {
            version: 1,
            id: "tapper".into(),
            joints: vec![
                joint("root", None, 0.0, 0.0),
                joint("torso", Some("root"), 0.0, -80.0),
                joint("upper_arm_l", Some("torso"), -20.0, -60.0),
                joint("forearm_l", Some("upper_arm_l"), 0.0, 50.0),
                joint("hand_l", Some("forearm_l"), 0.0, 45.0),
                joint("upper_arm_r", Some("torso"), 20.0, -60.0),
                joint("forearm_r", Some("upper_arm_r"), 0.0, 50.0),
                joint("hand_r", Some("forearm_r"), 0.0, 45.0),
            ],
            parts: vec![part("heldprop_r_puhelin_screen", "hand_r")],
            screens: vec![ScreenSlotDefinition {
                id: "phone".into(),
                part: "heldprop_r_puhelin_screen".into(),
                origin: [0.0, 0.0],
                x_axis: [10.0, 0.0],
                y_axis: [0.0, 20.0],
                overlays: Vec::new(),
            }],
            ..Default::default()
        }
    }
    fn distance(a: Vec2, b: Vec2) -> f64 {
        (a.x - b.x).hypot(a.y - b.y)
    }

    #[test]
    fn envelope_is_one_at_the_tap_and_zero_outside_its_window() {
        let taps = [1.0, 3.0];
        assert_eq!(tap_envelope(1.0, &taps), 1.0);
        assert_eq!(tap_envelope(3.0, &taps), 1.0);
        assert_eq!(tap_envelope(1.0 + TAP_HOLD_SECONDS, &taps), 1.0);
        let end = TAP_HOLD_SECONDS + TAP_RELEASE_SECONDS;
        for t in [
            0.0,
            1.0 - TAP_APPROACH_SECONDS - 1e-6,
            1.0 + end + 1e-6,
            2.0,
            3.0 + end + 1e-6,
        ] {
            assert_eq!(tap_envelope(t, &taps), 0.0, "{t}");
        }
        // Rising toward contact, falling after it.
        let a = tap_envelope(1.0 - 0.15, &taps);
        let b = tap_envelope(1.0 - 0.05, &taps);
        assert!(0.0 < a && a < b && b < 1.0);
        let c = tap_envelope(1.0 + TAP_HOLD_SECONDS + 0.05, &taps);
        assert!(0.0 < c && c < 1.0);
        assert_eq!(tap_envelope(f64::NAN, &taps), 0.0);
        assert_eq!(tap_envelope(1.0, &[f64::NAN]), 0.0);
        // Close taps overlap without dropping to zero between them.
        assert!(tap_envelope(1.15, &[1.0, 1.3]) > 0.5);
    }

    #[test]
    fn the_free_hand_reaches_the_slot_centre_at_contact() {
        let prepared = rig().prepare().unwrap();
        let arm = prepared.tap_arm("phone").unwrap();
        assert_eq!(arm.hand, HandSide::Left);
        // Hold the phone out in front of the chest.
        let offsets = vec![
            (
                prepared.joint_index("upper_arm_r").unwrap(),
                Transform2 {
                    rotation: 1.2,
                    ..Default::default()
                },
            ),
            (
                prepared.joint_index("forearm_r").unwrap(),
                Transform2 {
                    rotation: 0.6,
                    ..Default::default()
                },
            ),
        ];
        let centre = prepared.screen_centre(&offsets, "phone").unwrap();
        let press = prepared
            .tap_press(arm, "phone", 1.0, &offsets)
            .unwrap()
            .unwrap();
        let mut posed = offsets.clone();
        posed.extend(press);
        let world = prepared.world_transforms(&posed).unwrap();
        assert!(distance(origin(world[arm.limb.end]), centre) < 1e-6);
        // The phone did not move.
        assert_eq!(prepared.screen_centre(&posed, "phone").unwrap(), centre);
        // Zero weight leaves the pose alone; half weight lies between.
        assert_eq!(prepared.tap_press(arm, "phone", 0.0, &offsets), Ok(None));
        let rest = origin(prepared.world_transforms(&offsets).unwrap()[arm.limb.end]);
        let mut half = offsets.clone();
        half.extend(
            prepared
                .tap_press(arm, "phone", 0.5, &offsets)
                .unwrap()
                .unwrap(),
        );
        let mid = origin(prepared.world_transforms(&half).unwrap()[arm.limb.end]);
        let expected = Vec2::new(0.5 * (rest.x + centre.x), 0.5 * (rest.y + centre.y));
        assert!(distance(mid, expected) < 1e-6);
        // Pure: the same input gives the same output.
        assert_eq!(
            prepared.tap_press(arm, "phone", 0.7, &offsets),
            prepared.tap_press(arm, "phone", 0.7, &offsets)
        );
    }

    #[test]
    fn missing_or_busy_free_arm_is_a_typed_error() {
        let mut no_left = rig();
        no_left
            .joints
            .retain(|j| !j.id.ends_with("_l") || j.id == "hand_l");
        no_left.joints.iter_mut().for_each(|j| {
            if j.id == "hand_l" {
                j.parent = Some("torso".into());
            }
        });
        let prepared = no_left.prepare().unwrap();
        assert!(matches!(
            prepared.tap_arm("phone"),
            Err(TapError::NoFreeArm { slot, reason }) if slot == "phone" && reason.contains("upper_arm_l")
        ));
        let mut busy = rig();
        busy.parts
            .push(part("heldprop_l_kahvikuppi_body", "hand_l"));
        assert!(matches!(
            busy.prepare().unwrap().tap_arm("phone"),
            Err(TapError::NoFreeArm { reason, .. }) if reason.contains("hand_l already holds")
        ));
        assert_eq!(
            rig().prepare().unwrap().tap_arm("tv"),
            Err(TapError::UnknownSlot("tv".into()))
        );
        // A slot not held in a hand is tapped by the right arm.
        let mut table = rig();
        table.parts[0].joint = "torso".into();
        let arm = table.prepare().unwrap().tap_arm("phone").unwrap();
        assert_eq!(arm.hand, HandSide::Right);
    }
}
