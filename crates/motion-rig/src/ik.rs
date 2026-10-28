//! Analytic two-bone inverse kinematics (law of cosines).
//!
//! `solve_two_bone` is a pure function of its arguments: the same input always
//! gives the same output, and every accepted input gives finite values. An
//! unreachable target yields a straight limb pointing at it (too far) or a
//! fully folded limb (too close). Timeline tracks for IK targets are resolved by
//! `motion-scene` (KS-003d); this module only takes the target as a value.
use crate::attachment::{origin, LimbJoints};
use crate::{Affine2, Error, PreparedRig, Transform2, Vec2};
use std::f64::consts::{PI, TAU};

/// Largest accepted coordinate or bone length, matching rig world bounds.
pub const MAX_IK_EXTENT: f64 = 1_000_000.0;
/// Shortest accepted bone length.
pub const MIN_BONE_LENGTH: f64 = 1e-6;

/// Which side of the root → target line the middle joint (elbow or knee) lies
/// on: the sign of the cross product `(target - root) × (middle - root)`. In
/// y-down screen space `Positive` bends clockwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BendDirection {
    #[default]
    Positive,
    Negative,
}
impl BendDirection {
    fn sign(self) -> f64 {
        match self {
            Self::Positive => 1.0,
            Self::Negative => -1.0,
        }
    }
}

/// World-space result. Angles are bone directions, `atan2(dy, dx)` in `(-π, π]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoBoneSolution {
    pub upper_angle: f64,
    pub lower_angle: f64,
    /// Elbow or knee position.
    pub middle: Vec2,
    /// Hand or foot position.
    pub end: Vec2,
    /// True when the target lies within the limb's reach (`end` equals it).
    pub reached: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IkError {
    #[error("IK root and target must be finite and within ±1e6")]
    InvalidPoint,
    #[error("IK bone lengths must be finite and between 1e-6 and 1e6")]
    InvalidLength,
    #[error("IK limb joints must be distinct and their bones longer than 1e-6")]
    DegenerateLimb,
    #[error("IK needs uniform, unsheared scale on the limb's parent and upper joints")]
    NonUniformScale,
    #[error("IK pose evaluation failed: {0}")]
    Pose(#[from] Error),
}

/// Wrap an angle to `(-π, π]`.
pub fn wrap_angle(angle: f64) -> f64 {
    let wrapped = angle.rem_euclid(TAU);
    if wrapped > PI {
        wrapped - TAU
    } else {
        wrapped
    }
}

fn finite_point(p: Vec2) -> bool {
    [p.x, p.y]
        .iter()
        .all(|v| v.is_finite() && v.abs() <= MAX_IK_EXTENT)
}

/// Solve a two-bone chain rooted at `root` for `target`.
pub fn solve_two_bone(
    root: Vec2,
    upper: f64,
    lower: f64,
    target: Vec2,
    bend: BendDirection,
) -> Result<TwoBoneSolution, IkError> {
    if !finite_point(root) || !finite_point(target) {
        return Err(IkError::InvalidPoint);
    }
    if ![upper, lower]
        .iter()
        .all(|l| l.is_finite() && (MIN_BONE_LENGTH..=MAX_IK_EXTENT).contains(l))
    {
        return Err(IkError::InvalidLength);
    }
    let (dx, dy) = (target.x - root.x, target.y - root.y);
    let distance = dx.hypot(dy);
    // A target on the root has no direction; hang the limb straight down (+y).
    let base = if distance > 0.0 {
        dy.atan2(dx)
    } else {
        PI / 2.0
    };
    let (min_reach, max_reach) = ((upper - lower).abs(), upper + lower);
    let reach = distance.clamp(min_reach, max_reach);
    let tolerance = 1e-9 * max_reach.max(1.0);
    let reached = distance >= min_reach - tolerance && distance <= max_reach + tolerance;
    // Interior angles: at the root between target and middle joint, and at the
    // middle joint between the two bones. Clamping keeps acos finite.
    let (root_angle, middle_angle) = if reach > 0.0 {
        let cos_root = (upper * upper + reach * reach - lower * lower) / (2.0 * upper * reach);
        let cos_middle = (upper * upper + lower * lower - reach * reach) / (2.0 * upper * lower);
        (
            cos_root.clamp(-1.0, 1.0).acos(),
            cos_middle.clamp(-1.0, 1.0).acos(),
        )
    } else {
        // Equal bones folded onto the root.
        (0.0, 0.0)
    };
    let s = bend.sign();
    let upper_angle = wrap_angle(base + s * root_angle);
    let lower_angle = wrap_angle(upper_angle - s * (PI - middle_angle));
    let middle = Vec2::new(
        root.x + upper * upper_angle.cos(),
        root.y + upper * upper_angle.sin(),
    );
    let end = Vec2::new(
        middle.x + lower * lower_angle.cos(),
        middle.y + lower * lower_angle.sin(),
    );
    Ok(TwoBoneSolution {
        upper_angle,
        lower_angle,
        middle,
        end,
        reached,
    })
}

/// Rig-level IK result: replacement offsets for the upper and lower joints and
/// where the end joint actually lands after joint limits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimbIk {
    /// `(joint, offset)` pairs to append after the input offsets; each keeps
    /// the input offset's translation and scale and changes only rotation.
    pub offsets: [(usize, Transform2); 2],
    pub solution: TwoBoneSolution,
    /// End joint world position after evaluation with `offsets` applied.
    pub end: Vec2,
    /// True when a joint limit changed the solved rotation.
    pub limited: bool,
}

fn similarity(t: Affine2) -> bool {
    let (sx, sy) = (t.a.hypot(t.b), t.c.hypot(t.d));
    let tolerance = 1e-9 * sx.max(sy).max(1.0);
    (sx - sy).abs() <= tolerance && (t.a * t.c + t.b * t.d).abs() <= tolerance * sx.max(1.0)
}

impl PreparedRig {
    /// Rotate `limb.upper` and `limb.lower` so `limb.end` reaches the world
    /// `target`, starting from the pose given by `offsets`. Joint limits apply
    /// as in ordinary evaluation; `LimbIk::end` reports the resulting position.
    pub fn solve_limb_ik(
        &self,
        limb: LimbJoints,
        target: Vec2,
        bend: BendDirection,
        offsets: &[(usize, Transform2)],
    ) -> Result<LimbIk, IkError> {
        let joints = self.rig().joints();
        if limb.upper == limb.lower
            || limb.lower == limb.end
            || [limb.upper, limb.lower, limb.end]
                .iter()
                .any(|&j| j >= joints.len())
        {
            return Err(IkError::DegenerateLimb);
        }
        let world = self.world_transforms(offsets)?;
        let parent_ok = joints[limb.upper]
            .parent
            .is_none_or(|p| similarity(world[p]));
        if !parent_ok || !similarity(world[limb.upper]) || !similarity(world[limb.lower]) {
            return Err(IkError::NonUniformScale);
        }
        let (shoulder, middle, end) = (
            origin(world[limb.upper]),
            origin(world[limb.lower]),
            origin(world[limb.end]),
        );
        let upper_length = (middle.x - shoulder.x).hypot(middle.y - shoulder.y);
        let lower_length = (end.x - middle.x).hypot(end.y - middle.y);
        if upper_length < MIN_BONE_LENGTH || lower_length < MIN_BONE_LENGTH {
            return Err(IkError::DegenerateLimb);
        }
        let solution = solve_two_bone(shoulder, upper_length, lower_length, target, bend)?;
        let current_upper = (middle.y - shoulder.y).atan2(middle.x - shoulder.x);
        let current_lower = (end.y - middle.y).atan2(end.x - middle.x);
        let upper_delta = wrap_angle(solution.upper_angle - current_upper);
        let lower_delta = wrap_angle(solution.lower_angle - current_lower - upper_delta);
        let mut limited = false;
        let mut offset_for = |joint: usize, delta: f64| {
            let offset = offsets
                .iter()
                .rev()
                .find(|(j, _)| *j == joint)
                .map_or_else(Transform2::default, |(_, o)| *o);
            let rest = joints[joint].rest.rotation;
            let mut rotation = rest + offset.rotation + delta;
            // Pick the equivalent turn nearest the limit centre (or the rest
            // rotation) so offsets never accumulate whole turns.
            let centre = self
                .joint_limit(joint)
                .map_or(rest, |l| 0.5 * (l.min_rotation + l.max_rotation));
            rotation = centre + wrap_angle(rotation - centre);
            if let Some(limit) = self.joint_limit(joint) {
                let clamped = limit.clamp(rotation);
                limited |= clamped != rotation;
                rotation = clamped;
            }
            (
                joint,
                Transform2 {
                    rotation: rotation - rest,
                    ..offset
                },
            )
        };
        let solved = [
            offset_for(limb.upper, upper_delta),
            offset_for(limb.lower, lower_delta),
        ];
        let mut posed = offsets.to_vec();
        posed.extend_from_slice(&solved);
        let end = origin(self.world_transforms(&posed)?[limb.end]);
        Ok(LimbIk {
            offsets: solved,
            solution,
            end,
            limited,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn distance(a: Vec2, b: Vec2) -> f64 {
        (a.x - b.x).hypot(a.y - b.y)
    }
    fn solve(target: Vec2, bend: BendDirection) -> TwoBoneSolution {
        solve_two_bone(Vec2::new(0.0, 0.0), 60.0, 50.0, target, bend).unwrap()
    }

    #[test]
    fn reachable_target_is_hit_with_both_bend_directions() {
        let root = Vec2::default();
        let target = Vec2::new(40.0, 70.0);
        let positive = solve(target, BendDirection::Positive);
        let negative = solve(target, BendDirection::Negative);
        for s in [positive, negative] {
            assert!(s.reached);
            assert!(distance(s.end, target) < 1e-9);
            assert!((distance(root, s.middle) - 60.0).abs() < 1e-9);
            assert!((distance(s.middle, s.end) - 50.0).abs() < 1e-9);
        }
        // The middle joints lie on opposite sides of the root → target line.
        let side = |m: Vec2| target.x * m.y - target.y * m.x;
        assert!(side(positive.middle) > 0.0 && side(negative.middle) < 0.0);
    }

    #[test]
    fn too_far_target_gives_a_straight_limb_pointing_at_it() {
        let target = Vec2::new(300.0, -400.0);
        let s = solve(target, BendDirection::Positive);
        assert!(!s.reached);
        assert!((s.upper_angle - s.lower_angle).abs() < 1e-12);
        assert!((distance(Vec2::default(), s.end) - 110.0).abs() < 1e-9);
        let direction = target.y.atan2(target.x);
        assert!((s.upper_angle - direction).abs() < 1e-12);
    }

    #[test]
    fn too_close_and_coincident_targets_fold_without_nan() {
        for target in [
            Vec2::new(3.0, 4.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(1e-300, 0.0),
        ] {
            let s = solve(target, BendDirection::Negative);
            assert!(!s.reached || distance(Vec2::default(), target) >= 10.0);
            for v in [
                s.upper_angle,
                s.lower_angle,
                s.middle.x,
                s.middle.y,
                s.end.x,
                s.end.y,
            ] {
                assert!(v.is_finite(), "{target:?}");
            }
            assert!((distance(Vec2::default(), s.end) - 10.0).abs() < 1e-9);
        }
        // Equal bones fold exactly onto the root.
        let s = solve_two_bone(
            Vec2::default(),
            5.0,
            5.0,
            Vec2::default(),
            BendDirection::Positive,
        )
        .unwrap();
        assert!(s.reached && distance(s.end, Vec2::default()) < 1e-12);
    }

    #[test]
    fn reach_boundaries_count_as_reached() {
        for d in [10.0, 110.0] {
            let s = solve(Vec2::new(d, 0.0), BendDirection::Positive);
            assert!(s.reached, "{d}");
            assert!(distance(s.end, Vec2::new(d, 0.0)) < 1e-9, "{d}");
        }
    }

    #[test]
    fn invalid_inputs_are_typed_errors() {
        let ok = Vec2::new(1.0, 1.0);
        let bad = [f64::NAN, f64::INFINITY, -f64::INFINITY, 2e6];
        for v in bad {
            let p = Vec2::new(v, 0.0);
            assert_eq!(
                solve_two_bone(p, 1.0, 1.0, ok, BendDirection::Positive),
                Err(IkError::InvalidPoint)
            );
            assert_eq!(
                solve_two_bone(ok, 1.0, 1.0, p, BendDirection::Positive),
                Err(IkError::InvalidPoint)
            );
            assert_eq!(
                solve_two_bone(ok, v, 1.0, ok, BendDirection::Positive),
                Err(IkError::InvalidLength)
            );
        }
        for length in [0.0, -1.0, 1e-7] {
            assert_eq!(
                solve_two_bone(ok, 1.0, length, ok, BendDirection::Positive),
                Err(IkError::InvalidLength)
            );
        }
    }

    #[test]
    fn grid_of_targets_is_finite_and_deterministic() {
        let lengths = [(1e-6, 1e-6), (60.0, 50.0), (5.0, 500.0), (1e6, 1e6)];
        let coords = [-1e6, -1234.5, -50.0, -1e-9, 0.0, 1e-9, 7.0, 99.9, 1e6];
        for &(upper, lower) in &lengths {
            for &x in &coords {
                for &y in &coords {
                    for bend in [BendDirection::Positive, BendDirection::Negative] {
                        let target = Vec2::new(x, y);
                        let a =
                            solve_two_bone(Vec2::default(), upper, lower, target, bend).unwrap();
                        let b =
                            solve_two_bone(Vec2::default(), upper, lower, target, bend).unwrap();
                        assert_eq!(a, b);
                        for v in [
                            a.upper_angle,
                            a.lower_angle,
                            a.middle.x,
                            a.middle.y,
                            a.end.x,
                            a.end.y,
                        ] {
                            assert!(v.is_finite(), "{upper} {lower} {x} {y}");
                        }
                        assert!(a.upper_angle > -PI - 1e-12 && a.upper_angle <= PI);
                    }
                }
            }
        }
    }

    #[test]
    fn wrap_angle_maps_into_half_open_range() {
        assert_eq!(wrap_angle(PI), PI);
        assert!((wrap_angle(-PI) - PI).abs() < 1e-12);
        assert!((wrap_angle(3.0 * TAU + 0.5) - 0.5).abs() < 1e-9);
        assert!((wrap_angle(-0.5) + 0.5).abs() < 1e-12);
    }
}
