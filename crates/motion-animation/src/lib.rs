//! Deterministic keyframes and an original rule-driven seven-joint humanoid.
mod clips;
pub use clips::{
    evaluate_humanoid, evaluate_humanoid_with_offsets, evaluate_humanoid_with_weights_and_offsets,
    gesture_envelope, leap_shape, valid_humanoid_offset, ClipWeights, Gait, Gesture, HumanoidState,
    MoveEase, GAITS, GESTURES, HUMANOID_BONES, JOG_CYCLE_SECONDS, MOVE_EASES,
    POSTURE_BLEND_SECONDS, RUN_CYCLE_SECONDS, WALK_CYCLE_SECONDS,
};
mod principles;
pub use principles::{Principles, PrinciplesError, PRINCIPLE_PRESETS};
mod rig_motion;
pub use rig_motion::{
    acting_curve, acting_curve_with, blend_rig_motion, rig_body_pose, rig_gait, rig_gait_with_mood,
    rig_gesture, rig_gesture_with, rig_leap, rig_moving_hold, rig_pose_at, rig_sit, BodyPose,
    GaitMood, RigMotion, BODY_POSES, MOVING_HOLD_DELAY, RIG_MOTION_JOINTS, SIT_SECONDS,
};
pub mod exposure;
pub use exposure::{Exposure, ExposureError, Interpolation, MAX_EXPOSURE};
pub mod spring;
#[cfg(test)]
use motion_rig::Joint;
use motion_rig::Rig;
pub use motion_rig::{Transform2, Vec2};
pub use spring::{
    apply_dangle, dangle_joint, release_state, DampingRegime, DangleError, ForceProfile,
    ForcedSpring, Gust, ReleaseSample, Spring, SpringError, SpringParams, SpringProfile,
    SpringState, Turbulence,
};
pub const HIP: usize = 0;
pub const CHEST: usize = 1;
pub const HEAD: usize = 2;
pub const LEFT_HAND: usize = 3;
pub const RIGHT_HAND: usize = 4;
pub const LEFT_FOOT: usize = 5;
pub const RIGHT_FOOT: usize = 6;
/// Outgoing interpolation of a key: it shapes the interval up to the next key.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    Linear,
    SmoothStep,
    /// Stepped: the key's value holds until the next key.
    Hold,
    /// Cubic, slow start.
    EaseIn,
    /// Cubic, slow arrival.
    EaseOut,
    /// Cubic, slow start and arrival.
    EaseInOut,
    /// Ease-out that passes the next key and settles back onto it. `overshoot`
    /// is the conventional back constant (1.70158 gives about 10 % overshoot);
    /// it must be finite and within `0..=MAX_BACK_OVERSHOOT`.
    Back {
        overshoot: f64,
    },
}
/// Largest accepted `Easing::Back` overshoot constant.
pub const MAX_BACK_OVERSHOOT: f64 = 10.0;
/// The conventional back constant (about 10 % overshoot).
pub const DEFAULT_BACK_OVERSHOOT: f64 = 1.70158;
impl Easing {
    /// Whether the parameters are finite and bounded.
    pub fn is_valid(self) -> bool {
        match self {
            Easing::Back { overshoot } => {
                overshoot.is_finite() && (0.0..=MAX_BACK_OVERSHOOT).contains(&overshoot)
            }
            _ => true,
        }
    }
    /// Map normalized interval progress `t` in `0..=1` to blend progress.
    /// Every curve maps 0 to 0 and 1 to 1 exactly (`Hold` stays at 0 until the
    /// next key takes over). Only `Back` leaves `0..=1` in between.
    pub fn apply(self, t: f64) -> f64 {
        match self {
            Easing::Linear => t,
            Easing::SmoothStep => t * t * (3.0 - 2.0 * t),
            Easing::Hold => 0.0,
            Easing::EaseIn => t * t * t,
            Easing::EaseOut => {
                let u = 1.0 - t;
                1.0 - u * u * u
            }
            Easing::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let u = 2.0 - 2.0 * t;
                    1.0 - u * u * u / 2.0
                }
            }
            // Exact endpoints: the polynomial leaves rounding residue at 0.
            Easing::Back { .. } if t <= 0.0 => 0.0,
            Easing::Back { .. } if t >= 1.0 => 1.0,
            Easing::Back { overshoot } => {
                let u = t - 1.0;
                1.0 + (overshoot + 1.0) * u * u * u + overshoot * u * u
            }
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Keyframe {
    pub time: f64,
    pub value: Transform2,
    pub easing: Easing,
}
#[derive(Debug, Clone)]
pub struct Track {
    keys: Vec<Keyframe>,
}
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error(
        "track needs 1..=4096 strictly increasing finite nonnegative keys with valid transforms"
    )]
    InvalidTrack,
    #[error("animation time must be finite and nonnegative")]
    InvalidTime,
    #[error("pose tracks must match the rig and contain at most 65536 keys in total")]
    InvalidPoseTrack,
    #[error("pose must match the rig and produce bounded world transforms")]
    InvalidPose,
    #[error("pose blend weight must be finite and in 0..=1")]
    InvalidWeight,
}

/// Seekable local transforms in the rig's parent-before-child joint order.
/// Missing tracks retain the joint's rest transform. Tracks replace local rest
/// transforms; they are not additive offsets. The owned rig fixes joint identity.
#[derive(Debug, Clone)]
pub struct PoseTrack {
    rig: Rig,
    tracks: Vec<Option<Track>>,
}
impl PoseTrack {
    pub fn new(rig: Rig, tracks: Vec<Option<Track>>) -> Result<Self, Error> {
        if tracks.len() != rig.joints().len()
            || tracks.iter().flatten().map(|t| t.keys.len()).sum::<usize>() > 65536
        {
            return Err(Error::InvalidPoseTrack);
        }
        Ok(Self { rig, tracks })
    }

    pub fn rig(&self) -> &Rig {
        &self.rig
    }

    /// Validates the complete hierarchy before returning a new local pose.
    /// A locally valid key can still exceed world bounds when composed.
    pub fn sample(&self, time: f64) -> Result<Vec<Transform2>, Error> {
        if !time.is_finite() || time < 0.0 {
            return Err(Error::InvalidTime);
        }
        let mut pose = self.rig.rest_pose();
        for (local, track) in pose.iter_mut().zip(&self.tracks) {
            if let Some(track) = track {
                *local = track.sample(time)?;
            }
        }
        self.rig.evaluate(&pose).map_err(|_| Error::InvalidPose)?;
        Ok(pose)
    }
}

/// Blends two complete local poses, using the shortest angular arc per joint.
/// Weight zero returns `a` exactly, one returns `b` exactly. Inputs and output
/// must fit the rig's world bounds; errors never modify either input.
pub fn blend_pose(
    rig: &Rig,
    a: &[Transform2],
    b: &[Transform2],
    weight: f64,
) -> Result<Vec<Transform2>, Error> {
    if !weight.is_finite() || !(0.0..=1.0).contains(&weight) {
        return Err(Error::InvalidWeight);
    }
    rig.evaluate(a).map_err(|_| Error::InvalidPose)?;
    rig.evaluate(b).map_err(|_| Error::InvalidPose)?;
    if weight == 0.0 {
        return Ok(a.to_vec());
    }
    if weight == 1.0 {
        return Ok(b.to_vec());
    }
    let pose = a
        .iter()
        .zip(b)
        .map(|(&a, &b)| {
            // Reuse the existing track's scale, translation and rotation rules.
            Track::new(vec![
                Keyframe {
                    time: 0.0,
                    value: a,
                    easing: Easing::Linear,
                },
                Keyframe {
                    time: 1.0,
                    value: b,
                    easing: Easing::Linear,
                },
            ])
            .and_then(|track| track.sample(weight))
        })
        .collect::<Result<Vec<_>, _>>()?;
    rig.evaluate(&pose).map_err(|_| Error::InvalidPose)?;
    Ok(pose)
}
impl Track {
    pub fn new(keys: Vec<Keyframe>) -> Result<Self, Error> {
        if keys.is_empty()
            || keys.len() > 4096
            || keys.iter().any(|k| {
                !k.time.is_finite() || k.time < 0.0 || !k.value.is_valid() || !k.easing.is_valid()
            })
            || keys.windows(2).any(|k| k[0].time >= k[1].time)
        {
            return Err(Error::InvalidTrack);
        }
        Ok(Self { keys })
    }
    pub fn sample(&self, time: f64) -> Result<Transform2, Error> {
        self.sample_with(time, Interpolation::Spline)
    }
    /// Sample with an evaluation-time interpolation mode. `Stepped` treats every
    /// key as `Hold` (pose-to-pose blocking) without changing the stored keys.
    pub fn sample_with(
        &self,
        time: f64,
        interpolation: Interpolation,
    ) -> Result<Transform2, Error> {
        if !time.is_finite() || time < 0.0 {
            return Err(Error::InvalidTime);
        }
        let i = self.keys.partition_point(|k| k.time <= time);
        if i == 0 {
            return Ok(self.keys[0].value);
        }
        if i == self.keys.len() {
            return Ok(self.keys[i - 1].value);
        }
        let (a, b) = (self.keys[i - 1], self.keys[i]);
        let t = (time - a.time) / (b.time - a.time);
        let easing = match interpolation {
            Interpolation::Spline => a.easing,
            Interpolation::Stepped => Easing::Hold,
        };
        Ok(interpolate_transform(a.value, b.value, easing.apply(t)))
    }
}
/// Interpolate validated local authored offsets without constructing temporary tracks.
/// Exact endpoints preserve their representation; interior angles use the same
/// shortest arc (negative half-turn tie) as keyframe tracks.
pub fn blend_humanoid_offset(
    a: Transform2,
    b: Transform2,
    weight: f64,
) -> Result<Transform2, Error> {
    if !weight.is_finite() || !(0.0..=1.0).contains(&weight) {
        return Err(Error::InvalidWeight);
    }
    if !valid_humanoid_offset(a) || !valid_humanoid_offset(b) {
        return Err(Error::InvalidPose);
    }
    let value = if weight == 0.0 {
        a
    } else if weight == 1.0 {
        b
    } else {
        interpolate_transform(a, b, weight)
    };
    if !valid_humanoid_offset(value) {
        return Err(Error::InvalidPose);
    }
    Ok(value)
}

/// Add a signed parent-local authored delta to an original-rig offset.
/// Translation and rotation add; scale multiplies by the weighted deviation
/// from identity. Inputs are checked even at zero weight, which returns the
/// original base exactly. Combined offsets must still fit authored bounds.
pub fn add_humanoid_offset(
    base: Transform2,
    delta: Transform2,
    weight: f64,
) -> Result<Transform2, Error> {
    if !weight.is_finite() || !(0.0..=1.0).contains(&weight) {
        return Err(Error::InvalidWeight);
    }
    if !valid_humanoid_offset(base) || !valid_humanoid_offset(delta) {
        return Err(Error::InvalidPose);
    }
    if weight == 0.0 {
        return Ok(base);
    }
    let value = Transform2 {
        translation: Vec2::new(
            base.translation.x + weight * delta.translation.x,
            base.translation.y + weight * delta.translation.y,
        ),
        rotation: base.rotation + weight * delta.rotation,
        scale: Vec2::new(
            base.scale.x * (1.0 + weight * (delta.scale.x - 1.0)),
            base.scale.y * (1.0 + weight * (delta.scale.y - 1.0)),
        ),
    };
    if !valid_humanoid_offset(value) {
        return Err(Error::InvalidPose);
    }
    Ok(value)
}

fn interpolate_transform(a: Transform2, b: Transform2, t: f64) -> Transform2 {
    let lerp = |a: f64, b: f64| a + (b - a) * t;
    let rotation_delta = (b.rotation - a.rotation + std::f64::consts::PI)
        .rem_euclid(std::f64::consts::TAU)
        - std::f64::consts::PI;
    Transform2 {
        translation: Vec2::new(
            lerp(a.translation.x, b.translation.x),
            lerp(a.translation.y, b.translation.y),
        ),
        rotation: (a.rotation + rotation_delta * t).rem_euclid(std::f64::consts::TAU),
        scale: Vec2::new(lerp(a.scale.x, b.scale.x), lerp(a.scale.y, b.scale.y)),
    }
}
/// Local skeleton points with neutral posture. Walk phase is elapsed seconds
/// since the move began; `wave` weights the wave clip at its midpoint.
pub fn humanoid(walk_seconds: Option<f64>, wave: f64) -> [Vec2; 7] {
    let wave = if wave.is_finite() {
        wave.clamp(0.0, 1.0)
    } else {
        0.0
    };
    evaluate_humanoid(&HumanoidState {
        walk_seconds,
        gesture: Some((Gesture::Wave, 0.5, wave)),
        ..Default::default()
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn key(t: f64, x: f64, r: f64, easing: Easing) -> Keyframe {
        Keyframe {
            time: t,
            value: Transform2 {
                translation: Vec2::new(x, 0.0),
                rotation: r,
                ..Default::default()
            },
            easing,
        }
    }
    #[test]
    fn interpolation_boundaries_shortest_rotation_and_easing() {
        let track = Track::new(vec![
            key(0.0, 0.0, 170f64.to_radians(), Easing::Linear),
            key(2.0, 10.0, (-170f64).to_radians(), Easing::Linear),
        ])
        .unwrap();
        let mid = track.sample(1.0).unwrap();
        assert_eq!(mid.translation.x, 5.0);
        assert!((mid.rotation - std::f64::consts::PI).abs() < 1e-10);
        assert_eq!(track.sample(2.0).unwrap().translation.x, 10.0);
        assert_eq!(track.sample(9.0).unwrap().translation.x, 10.0);
        let smooth = Track::new(vec![
            key(0.0, 0.0, 0.0, Easing::SmoothStep),
            key(1.0, 1.0, 0.0, Easing::Linear),
        ])
        .unwrap();
        assert_eq!(smooth.sample(0.25).unwrap().translation.x, 0.15625);
        let hold = Track::new(vec![
            key(0.0, 0.0, 0.0, Easing::Hold),
            key(1.0, 1.0, 0.0, Easing::Linear),
        ])
        .unwrap();
        assert_eq!(hold.sample(0.9).unwrap().translation.x, 0.0);
        assert_eq!(hold.sample(1.0).unwrap().translation.x, 1.0);
    }
    #[test]
    fn easing_curves_hit_endpoints_and_are_monotonic_except_back() {
        let monotonic = [
            Easing::Linear,
            Easing::SmoothStep,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
        ];
        for easing in monotonic
            .into_iter()
            .chain([Easing::Back { overshoot: 1.3 }])
        {
            assert_eq!(easing.apply(0.0), 0.0, "{easing:?}");
            assert_eq!(easing.apply(1.0), 1.0, "{easing:?}");
        }
        for easing in monotonic {
            let mut previous = easing.apply(0.0);
            for i in 1..=1000 {
                let value = easing.apply(i as f64 / 1000.0);
                assert!(
                    value >= previous && (0.0..=1.0).contains(&value),
                    "{easing:?}"
                );
                previous = value;
            }
        }
        assert_eq!(Easing::EaseInOut.apply(0.5), 0.5);
        assert!(Easing::EaseIn.apply(0.25) < 0.25 && Easing::EaseOut.apply(0.25) > 0.25);
        // Back passes the target and settles; zero overshoot is a cubic ease-out.
        let back = Easing::Back { overshoot: 1.70158 };
        let peak = (0..=1000)
            .map(|i| back.apply(i as f64 / 1000.0))
            .fold(f64::MIN, f64::max);
        assert!(peak > 1.09 && peak < 1.11, "{peak}");
        let flat = Easing::Back { overshoot: 0.0 };
        assert!((0..=100).all(|i| {
            let t = i as f64 / 100.0;
            (flat.apply(t) - Easing::EaseOut.apply(t)).abs() < 1e-12
        }));
    }
    #[test]
    fn back_overshoot_is_validated_and_stepped_sampling_holds() {
        for overshoot in [f64::NAN, f64::INFINITY, -0.1, MAX_BACK_OVERSHOOT + 0.1] {
            let bad = Easing::Back { overshoot };
            assert!(!bad.is_valid());
            assert_eq!(
                Track::new(vec![key(0.0, 0.0, 0.0, bad), key(1.0, 1.0, 0.0, bad)]).unwrap_err(),
                Error::InvalidTrack
            );
        }
        let track = Track::new(vec![
            key(0.0, 0.0, 0.0, Easing::EaseInOut),
            key(1.0, 10.0, 1.0, Easing::Back { overshoot: 2.0 }),
            key(2.0, 20.0, 0.0, Easing::Linear),
        ])
        .unwrap();
        let stepped = |t| track.sample_with(t, Interpolation::Stepped).unwrap();
        assert_eq!(stepped(0.7).translation.x, 0.0);
        assert_eq!(stepped(1.0).translation.x, 10.0);
        assert_eq!(stepped(1.999).translation.x, 10.0);
        assert_eq!(stepped(2.5).translation.x, 20.0);
        assert_eq!(
            track.sample(0.5).unwrap(),
            track.sample_with(0.5, Interpolation::Spline).unwrap()
        );
        assert!(track.sample(1.6).unwrap().translation.x > 20.0);
    }
    #[test]
    fn rejects_bad_times_and_keys() {
        assert!(Track::new(vec![]).is_err());
        assert!(Track::new(vec![key(0.0, 0.0, 0.0, Easing::Linear); 2]).is_err());
        let t = Track::new(vec![key(0.0, 0.0, 0.0, Easing::Linear)]).unwrap();
        assert!(t.sample(f64::NAN).is_err());
        assert!(t.sample(-1.0).is_err());
    }
    #[test]
    fn humanoid_is_seekable_and_preserves_hierarchy() {
        let idle = humanoid(None, 0.0);
        assert_eq!(idle[HEAD], Vec2::new(0.0, -90.0));
        let walk = humanoid(Some(1.0 / 6.0), 0.0);
        assert_ne!(walk, idle);
        assert_eq!(walk, humanoid(Some(1.0 / 6.0), 0.0));
        assert_eq!(humanoid(None, 1.0)[RIGHT_HAND].y, -85.0);
        assert_eq!(humanoid(None, f64::NAN), idle);
        assert!(humanoid(Some(f64::MAX), 0.0)
            .iter()
            .all(|p| p.x.is_finite() && p.y.is_finite()));
    }

    fn two_joint_rig() -> Rig {
        Rig::new(vec![
            Joint {
                name: "root".into(),
                parent: None,
                rest: Transform2::default(),
            },
            Joint {
                name: "hand".into(),
                parent: Some(0),
                rest: Transform2 {
                    translation: Vec2::new(3.0, 0.0),
                    ..Default::default()
                },
            },
        ])
        .unwrap()
    }

    #[test]
    fn joint_tracks_seek_and_propagate_parent_rotation() {
        let track = PoseTrack::new(
            two_joint_rig(),
            vec![
                Some(
                    Track::new(vec![
                        key(0.0, 0.0, 0.0, Easing::Linear),
                        key(2.0, 10.0, std::f64::consts::PI, Easing::Linear),
                    ])
                    .unwrap(),
                ),
                None,
            ],
        )
        .unwrap();
        let mid = track.sample(1.0).unwrap();
        let world = track.rig().evaluate(&mid).unwrap();
        assert!((world[1].tx - 5.0).abs() < 1e-10);
        assert!((world[1].ty + 3.0).abs() < 1e-10);
        track.sample(100.0).unwrap();
        assert_eq!(track.sample(1.0).unwrap(), mid);
        assert_eq!(mid[1], track.rig().rest_pose()[1]);
        assert!(track.sample(f64::INFINITY).is_err());
    }

    #[test]
    fn pose_blend_endpoints_shortest_rotation_and_input_integrity() {
        let rig = two_joint_rig();
        let mut a = rig.rest_pose();
        let mut b = a.clone();
        a[0].rotation = 170f64.to_radians();
        b[0].rotation = (-170f64).to_radians();
        b[1].translation.x = 9.0;
        let original = (a.clone(), b.clone());
        assert_eq!(blend_pose(&rig, &a, &b, 0.0).unwrap(), a);
        assert_eq!(blend_pose(&rig, &a, &b, 1.0).unwrap(), b);
        let mid = blend_pose(&rig, &a, &b, 0.5).unwrap();
        assert!((mid[0].rotation - std::f64::consts::PI).abs() < 1e-10);
        assert_eq!(mid[1].translation.x, 6.0);
        for weight in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            assert_eq!(blend_pose(&rig, &a, &b, weight), Err(Error::InvalidWeight));
        }
        assert_eq!(blend_pose(&rig, &a[..1], &b, 0.0), Err(Error::InvalidPose));
        assert_eq!((a, b), original);
    }

    #[test]
    fn pose_tracks_reject_shape_budget_and_composed_overflow() {
        assert!(PoseTrack::new(two_joint_rig(), vec![None]).is_err());
        let big = Track::new(vec![key(0.0, 1e6, 0.0, Easing::Hold)]).unwrap();
        let track = PoseTrack::new(two_joint_rig(), vec![Some(big), None]).unwrap();
        assert_eq!(track.sample(0.0), Err(Error::InvalidPose));
        let rig = Rig::new(
            (0..17)
                .map(|i| Joint {
                    name: format!("joint_{i}"),
                    parent: None,
                    rest: Transform2::default(),
                })
                .collect(),
        )
        .unwrap();
        let keys = (0..4096)
            .map(|i| key(i as f64, 0.0, 0.0, Easing::Hold))
            .collect();
        let long = Track::new(keys).unwrap();
        assert!(PoseTrack::new(rig, vec![Some(long); 17]).is_err());
    }
}

#[cfg(test)]
mod offset_blend_tests {
    use super::*;
    #[test]
    fn additive_components_signed_rotation_and_exact_zero() {
        let base = Transform2 {
            translation: Vec2::new(10.0, -20.0),
            rotation: 3.0,
            scale: Vec2::new(0.5, 2.0),
        };
        let delta = Transform2 {
            translation: Vec2::new(30.0, 40.0),
            rotation: -8.0,
            scale: Vec2::new(3.0, 0.5),
        };
        for (weight, translation, rotation, scale) in [
            (0.0, Vec2::new(10.0, -20.0), 3.0, Vec2::new(0.5, 2.0)),
            (0.5, Vec2::new(25.0, 0.0), -1.0, Vec2::new(1.0, 1.5)),
            (1.0, Vec2::new(40.0, 20.0), -5.0, Vec2::new(1.5, 1.0)),
        ] {
            let actual = add_humanoid_offset(base, delta, weight).unwrap();
            assert_eq!(actual.translation, translation);
            assert_eq!(actual.rotation, rotation);
            assert_eq!(actual.scale, scale);
            assert_eq!(
                add_humanoid_offset(base, Transform2::default(), weight).unwrap(),
                base
            );
        }
        let signed_zero = Transform2 {
            translation: Vec2::new(-0.0, -0.0),
            rotation: -0.0,
            ..Default::default()
        };
        let zero = add_humanoid_offset(signed_zero, delta, -0.0).unwrap();
        assert_eq!(zero.translation.x.to_bits(), (-0.0f64).to_bits());
        assert_eq!(zero.translation.y.to_bits(), (-0.0f64).to_bits());
        assert_eq!(zero.rotation.to_bits(), (-0.0f64).to_bits());
    }

    #[test]
    fn additive_rejects_invalid_inputs_even_at_zero_and_combined_bounds() {
        let identity = Transform2::default();
        for weight in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1, 1.1] {
            assert_eq!(
                add_humanoid_offset(identity, identity, weight),
                Err(Error::InvalidWeight)
            );
        }
        for invalid in [
            Transform2 {
                translation: Vec2::new(f64::NAN, 0.0),
                ..identity
            },
            Transform2 {
                translation: Vec2::new(0.0, 1001.0),
                ..identity
            },
            Transform2 {
                rotation: f64::INFINITY,
                ..identity
            },
            Transform2 {
                rotation: -1001.0,
                ..identity
            },
            Transform2 {
                scale: Vec2::new(0.09, 1.0),
                ..identity
            },
            Transform2 {
                scale: Vec2::new(1.0, 4.01),
                ..identity
            },
        ] {
            assert_eq!(
                add_humanoid_offset(invalid, identity, 0.0),
                Err(Error::InvalidPose)
            );
            assert_eq!(
                add_humanoid_offset(identity, invalid, 0.0),
                Err(Error::InvalidPose)
            );
        }
        for offset in [
            Transform2 {
                translation: Vec2::new(600.0, 0.0),
                ..identity
            },
            Transform2 {
                translation: Vec2::new(0.0, -600.0),
                ..identity
            },
            Transform2 {
                rotation: -600.0,
                ..identity
            },
            Transform2 {
                scale: Vec2::new(3.0, 1.0),
                ..identity
            },
            Transform2 {
                scale: Vec2::new(1.0, 0.2),
                ..identity
            },
        ] {
            assert!(valid_humanoid_offset(offset));
            assert_eq!(
                add_humanoid_offset(offset, offset, 1.0),
                Err(Error::InvalidPose)
            );
        }
    }

    #[test]
    fn offsets_blend_independent_components_endpoints_and_rotation() {
        let a = Transform2 {
            translation: Vec2::new(10.0, -20.0),
            rotation: 170f64.to_radians(),
            scale: Vec2::new(0.5, 2.0),
        };
        let b = Transform2 {
            translation: Vec2::new(30.0, 40.0),
            rotation: (-170f64).to_radians(),
            scale: Vec2::new(2.5, 4.0),
        };
        assert_eq!(blend_humanoid_offset(a, b, 0.0).unwrap(), a);
        assert_eq!(blend_humanoid_offset(a, b, 1.0).unwrap(), b);
        let mid = blend_humanoid_offset(a, b, 0.5).unwrap();
        assert_eq!(mid.translation, Vec2::new(20.0, 10.0));
        assert_eq!(mid.scale, Vec2::new(1.5, 3.0));
        assert!((mid.rotation - std::f64::consts::PI).abs() < 1e-12);
        let half = blend_humanoid_offset(
            Transform2::default(),
            Transform2 {
                rotation: std::f64::consts::PI,
                ..Default::default()
            },
            0.5,
        )
        .unwrap();
        assert!((half.rotation - 1.5 * std::f64::consts::PI).abs() < 1e-12);
    }
    #[test]
    fn offsets_validate_extreme_angles_and_weights_without_clamping() {
        let a = Transform2 {
            rotation: 1000.0,
            ..Default::default()
        };
        let b = Transform2 {
            rotation: -1000.0,
            ..Default::default()
        };
        let mid = blend_humanoid_offset(a, b, 0.5).unwrap();
        assert!(valid_humanoid_offset(mid));
        let delta = (b.rotation - a.rotation + std::f64::consts::PI)
            .rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        let expected = a.rotation + delta * 0.5;
        assert!((mid.rotation.sin() - expected.sin()).abs() < 1e-12);
        for weight in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            assert_eq!(
                blend_humanoid_offset(a, b, weight),
                Err(Error::InvalidWeight)
            );
        }
        let invalid = Transform2 {
            rotation: 1001.0,
            ..Default::default()
        };
        assert_eq!(
            blend_humanoid_offset(invalid, b, 0.0),
            Err(Error::InvalidPose)
        );
    }
}
