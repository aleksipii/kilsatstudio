//! Limb attachment contract for stick characters.
//!
//! The pelvis joint (`root`) is the hierarchy root. Legs hang from it
//! (`root → thigh → shin → foot`); arms hang from the shoulders on the torso
//! (`torso → upper_arm → forearm → hand`). A limb stays attached while its
//! joints only rotate: a translation or scale offset on a limb joint would move
//! it away from the end of its parent bone. See `docs/rig-attachment-ik.md`.
use crate::{Affine2, PreparedRig, Transform2, Vec2};

/// Names of one two-bone limb and the joint it hangs from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimbChain {
    pub name: &'static str,
    /// Pelvis (`root`) for legs, `torso` (shoulders) for arms.
    pub anchor: &'static str,
    pub upper: &'static str,
    pub lower: &'static str,
    /// Hand or foot: the end effector that IK targets and props attach to.
    pub end: &'static str,
}

/// Pelvis joint id shared by the stick characters.
pub const PELVIS: &str = "root";
/// Torso joint id; the shoulders are the arm chains' rest offsets on it.
pub const TORSO: &str = "torso";

/// The four limbs of the stick character convention, arms first.
pub const STICK_LIMBS: [LimbChain; 4] = [
    LimbChain {
        name: "arm_l",
        anchor: TORSO,
        upper: "upper_arm_l",
        lower: "forearm_l",
        end: "hand_l",
    },
    LimbChain {
        name: "arm_r",
        anchor: TORSO,
        upper: "upper_arm_r",
        lower: "forearm_r",
        end: "hand_r",
    },
    LimbChain {
        name: "leg_l",
        anchor: PELVIS,
        upper: "thigh_l",
        lower: "shin_l",
        end: "foot_l",
    },
    LimbChain {
        name: "leg_r",
        anchor: PELVIS,
        upper: "thigh_r",
        lower: "shin_r",
        end: "foot_r",
    },
];

/// Joint indices of a limb whose parent links were verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimbJoints {
    pub anchor: usize,
    pub upper: usize,
    pub lower: usize,
    pub end: usize,
}
impl LimbJoints {
    /// Joints that must only rotate for the limb to stay attached.
    pub fn moving(self) -> [usize; 3] {
        [self.upper, self.lower, self.end]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AttachmentError {
    #[error("limb {limb} needs joint {joint}")]
    MissingJoint {
        limb: &'static str,
        joint: &'static str,
    },
    #[error("limb {limb}: joint {joint} must be a child of {parent}")]
    WrongParent {
        limb: &'static str,
        joint: &'static str,
        parent: &'static str,
    },
    #[error("pelvis joint {PELVIS} must be the hierarchy root")]
    PelvisNotRoot,
    #[error("limb joint {joint} may only rotate; a translation or scale offset detaches it")]
    DetachingOffset { joint: String },
}

impl PreparedRig {
    /// Resolve a limb and verify `anchor → upper → lower → end` parent links.
    pub fn limb(&self, chain: &LimbChain) -> Result<LimbJoints, AttachmentError> {
        let index = |joint: &'static str| {
            self.joint_index(joint)
                .ok_or(AttachmentError::MissingJoint {
                    limb: chain.name,
                    joint,
                })
        };
        let limb = LimbJoints {
            anchor: index(chain.anchor)?,
            upper: index(chain.upper)?,
            lower: index(chain.lower)?,
            end: index(chain.end)?,
        };
        let joints = self.rig().joints();
        for (child, parent, joint, parent_name) in [
            (limb.upper, limb.anchor, chain.upper, chain.anchor),
            (limb.lower, limb.upper, chain.lower, chain.upper),
            (limb.end, limb.lower, chain.end, chain.lower),
        ] {
            if joints[child].parent != Some(parent) {
                return Err(AttachmentError::WrongParent {
                    limb: chain.name,
                    joint,
                    parent: parent_name,
                });
            }
        }
        Ok(limb)
    }
    /// Resolve all four stick limbs and check that the pelvis is the root and
    /// the torso hangs from it.
    pub fn stick_limbs(&self) -> Result<[LimbJoints; 4], AttachmentError> {
        let pelvis = self
            .joint_index(PELVIS)
            .ok_or(AttachmentError::MissingJoint {
                limb: "body",
                joint: PELVIS,
            })?;
        let joints = self.rig().joints();
        if joints[pelvis].parent.is_some() {
            return Err(AttachmentError::PelvisNotRoot);
        }
        let torso = self
            .joint_index(TORSO)
            .ok_or(AttachmentError::MissingJoint {
                limb: "body",
                joint: TORSO,
            })?;
        if joints[torso].parent != Some(pelvis) {
            return Err(AttachmentError::WrongParent {
                limb: "body",
                joint: TORSO,
                parent: PELVIS,
            });
        }
        let mut out = [LimbJoints {
            anchor: 0,
            upper: 0,
            lower: 0,
            end: 0,
        }; 4];
        for (slot, chain) in out.iter_mut().zip(&STICK_LIMBS) {
            *slot = self.limb(chain)?;
        }
        Ok(out)
    }
    /// Reject offsets that translate or scale a limb joint. Rotation-only limb
    /// offsets keep every limb attached by construction.
    pub fn check_limb_offsets(
        &self,
        limbs: &[LimbJoints],
        offsets: &[(usize, Transform2)],
    ) -> Result<(), AttachmentError> {
        for (joint, offset) in offsets {
            let detaching =
                offset.translation != Vec2::default() || offset.scale != Vec2::new(1.0, 1.0);
            if detaching && limbs.iter().any(|l| l.moving().contains(joint)) {
                return Err(AttachmentError::DetachingOffset {
                    joint: self
                        .rig()
                        .joints()
                        .get(*joint)
                        .map_or_else(|| joint.to_string(), |j| j.name.clone()),
                });
            }
        }
        Ok(())
    }
    /// Distance between a joint's world origin and the point where its parent's
    /// world transform places the joint's rest offset: 0 while attached. Roots
    /// have no parent and report 0.
    pub fn attachment_gap(&self, world: &[Affine2], joint: usize) -> f64 {
        let Some(j) = self.rig().joints().get(joint) else {
            return f64::INFINITY;
        };
        let Some(parent) = j.parent else {
            return 0.0;
        };
        let expected = world[parent].point(j.rest.translation);
        let actual = origin(world[joint]);
        (actual.x - expected.x).hypot(actual.y - expected.y)
    }
}

/// World position of a joint (its transform's origin).
pub fn origin(transform: Affine2) -> Vec2 {
    Vec2::new(transform.tx, transform.ty)
}
