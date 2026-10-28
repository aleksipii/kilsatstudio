//! Bounded two-dimensional joint hierarchies, independent of rendering.
pub mod attachment;
pub mod definition;
pub mod ik;
pub mod props;
pub mod screen;
pub use attachment::{AttachmentError, LimbChain, LimbJoints, PELVIS, STICK_LIMBS, TORSO};
pub use definition::{
    round_corners, ActingInput, BlinkDefinition, EllipseDefinition, EvaluatedOutline,
    EvaluatedPart, EvaluatedTexture, ExpressionVariant, EyesDefinition, JointLimitDefinition,
    MouthShapes, OutlineDefinition, OutlineMode, PartTextureDefinition, PartVariantDefinition,
    PreparedRig, RigDefinition, RigJointDefinition, RigidPartDefinition, TextureFit, VertexWeight,
    VisemeMapDefinition, ELLIPSE_SEGMENTS, EMOTION_NAMES, MAX_EVALUATED_POINTS, MAX_JOINT_LIMIT,
    MAX_OUTLINE_WIDTH, MAX_SMOOTH, VISEME_NAMES,
};
pub use ik::{solve_two_bone, BendDirection, IkError, LimbIk, TwoBoneSolution};
pub use props::{
    attach_hand_prop, detach_hand_prop, grip_parts, grip_screen, hand_availability, GripPlacement,
    HandPropCatalog, HandPropItem, HandPropScreen, HandSide, PropError, GRIP_JOINT,
    MAX_HAND_PROP_CATALOG_BYTES,
};
pub use screen::{
    ScreenContent, ScreenSlotDefinition, MAX_SCREEN_SLOTS, SCREEN_LETTERBOX_COLOR, SCREEN_OFF_COLOR,
};
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}
impl Vec2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2 {
    pub translation: Vec2,
    pub rotation: f64,
    pub scale: Vec2,
}
impl Default for Transform2 {
    fn default() -> Self {
        Self {
            translation: Vec2::default(),
            rotation: 0.0,
            scale: Vec2::new(1.0, 1.0),
        }
    }
}
impl Transform2 {
    pub fn is_valid(self) -> bool {
        [
            self.translation.x,
            self.translation.y,
            self.rotation,
            self.scale.x,
            self.scale.y,
        ]
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
            && self.scale.x > 0.0
            && self.scale.y > 0.0
    }
    pub fn affine(self) -> Affine2 {
        let (s, c) = self.rotation.sin_cos();
        Affine2 {
            a: c * self.scale.x,
            b: s * self.scale.x,
            c: -s * self.scale.y,
            d: c * self.scale.y,
            tx: self.translation.x,
            ty: self.translation.y,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine2 {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}
impl Affine2 {
    pub fn point(self, p: Vec2) -> Vec2 {
        Vec2::new(
            self.a * p.x + self.c * p.y + self.tx,
            self.b * p.x + self.d * p.y + self.ty,
        )
    }
    pub fn compose(self, r: Self) -> Self {
        Self {
            a: self.a * r.a + self.c * r.b,
            b: self.b * r.a + self.d * r.b,
            c: self.a * r.c + self.c * r.d,
            d: self.b * r.c + self.d * r.d,
            tx: self.a * r.tx + self.c * r.ty + self.tx,
            ty: self.b * r.tx + self.d * r.ty + self.ty,
        }
    }
    /// Inverse of an invertible transform (always the case for positive scales).
    pub fn inverse(self) -> Self {
        let det = self.a * self.d - self.b * self.c;
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Self {
            a,
            b,
            c,
            d,
            tx: -(a * self.tx + c * self.ty),
            ty: -(b * self.tx + d * self.ty),
        }
    }
    fn bounded(self) -> bool {
        [self.a, self.b, self.c, self.d, self.tx, self.ty]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Joint {
    pub name: String,
    pub parent: Option<usize>,
    pub rest: Transform2,
}
#[derive(Debug, Clone)]
pub struct Rig {
    joints: Vec<Joint>,
}
#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("custom rig requires version 1, unique stable IDs, valid parent references, bounded rigid polygon attachments and consistent variant, viseme, expression and eye references")]
    InvalidDefinition,
    #[error(
        "rig needs 1..=256 unique named joints in parent-before-child order with finite transforms"
    )]
    InvalidRig,
    #[error(
        "pose must contain one valid transform per joint and produce bounded world transforms"
    )]
    InvalidPose,
}
impl Rig {
    pub fn new(joints: Vec<Joint>) -> Result<Self, Error> {
        if joints.is_empty() || joints.len() > 256 {
            return Err(Error::InvalidRig);
        }
        let mut names = std::collections::BTreeSet::new();
        for (i, j) in joints.iter().enumerate() {
            if j.name.is_empty()
                || j.name.len() > 256
                || !names.insert(j.name.clone())
                || j.parent.is_some_and(|p| p >= i)
                || !j.rest.is_valid()
            {
                return Err(Error::InvalidRig);
            }
        }
        let rig = Self { joints };
        rig.evaluate(&rig.rest_pose())
            .map_err(|_| Error::InvalidRig)?;
        Ok(rig)
    }
    pub fn joints(&self) -> &[Joint] {
        &self.joints
    }
    pub fn rest_pose(&self) -> Vec<Transform2> {
        self.joints.iter().map(|j| j.rest).collect()
    }
    pub fn evaluate(&self, pose: &[Transform2]) -> Result<Vec<Affine2>, Error> {
        if pose.len() != self.joints.len() {
            return Err(Error::InvalidPose);
        }
        let mut world: Vec<Affine2> = Vec::with_capacity(pose.len());
        for (j, t) in self.joints.iter().zip(pose) {
            if !t.is_valid() {
                return Err(Error::InvalidPose);
            }
            let local = t.affine();
            let transform = j.parent.map_or(local, |p| world[p].compose(local));
            if !transform.bounded() {
                return Err(Error::InvalidPose);
            }
            world.push(transform)
        }
        Ok(world)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn joint(name: &str, parent: Option<usize>, x: f64) -> Joint {
        Joint {
            name: name.into(),
            parent,
            rest: Transform2 {
                translation: Vec2::new(x, 0.0),
                ..Default::default()
            },
        }
    }
    #[test]
    fn rotated_scaled_parent_propagates() {
        let mut root = joint("root", None, 10.0);
        root.rest.rotation = std::f64::consts::FRAC_PI_2;
        root.rest.scale = Vec2::new(2.0, 2.0);
        let rig = Rig::new(vec![root, joint("hand", Some(0), 3.0)]).unwrap();
        let world = rig.evaluate(&rig.rest_pose()).unwrap();
        assert!((world[1].tx - 10.0).abs() < 1e-10);
        assert!((world[1].ty - 6.0).abs() < 1e-10);
    }
    #[test]
    fn rejects_cycles_duplicates_nonfinite_and_overflow() {
        assert!(Rig::new(vec![joint("root", Some(0), 0.0)]).is_err());
        assert!(Rig::new(vec![joint("root", None, 0.0), joint("root", Some(0), 0.0)]).is_err());
        let rig = Rig::new(vec![joint("root", None, 0.0)]).unwrap();
        assert!(rig.evaluate(&[]).is_err());
        let mut pose = rig.rest_pose();
        pose[0].rotation = f64::NAN;
        assert!(rig.evaluate(&pose).is_err());
        let mut big = joint("big", None, 0.0);
        big.rest.scale = Vec2::new(1e6, 1e6);
        assert!(Rig::new(vec![big, joint("child", Some(0), 10.0)]).is_err());
    }
}
