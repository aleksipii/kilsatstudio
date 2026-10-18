//! Deterministic keyframes and an original rule-driven seven-joint humanoid.
mod clips;
pub use clips::{
    evaluate_humanoid, evaluate_humanoid_with_offsets, evaluate_humanoid_with_weights_and_offsets,
    gesture_envelope, valid_humanoid_offset, ClipWeights, Gesture, HumanoidState, GESTURES,
    HUMANOID_BONES, POSTURE_BLEND_SECONDS, WALK_CYCLE_SECONDS,
};
#[cfg(test)]
use motion_rig::Joint;
use motion_rig::Rig;
pub use motion_rig::{Transform2, Vec2};
pub const HIP: usize = 0;
pub const CHEST: usize = 1;
pub const HEAD: usize = 2;
pub const LEFT_HAND: usize = 3;
pub const RIGHT_HAND: usize = 4;
pub const LEFT_FOOT: usize = 5;
pub const RIGHT_FOOT: usize = 6;
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    Linear,
    SmoothStep,
    Hold,
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
            || keys
                .iter()
                .any(|k| !k.time.is_finite() || k.time < 0.0 || !k.value.is_valid())
            || keys.windows(2).any(|k| k[0].time >= k[1].time)
        {
            return Err(Error::InvalidTrack);
        }
        Ok(Self { keys })
    }
    pub fn sample(&self, time: f64) -> Result<Transform2, Error> {
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
        let t = match a.easing {
            Easing::Linear => t,
            Easing::SmoothStep => t * t * (3.0 - 2.0 * t),
            Easing::Hold => 0.0,
        };
        let lerp = |a: f64, b: f64| a + (b - a) * t;
        let rotation_delta = (b.value.rotation - a.value.rotation + std::f64::consts::PI)
            .rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        Ok(Transform2 {
            translation: Vec2::new(
                lerp(a.value.translation.x, b.value.translation.x),
                lerp(a.value.translation.y, b.value.translation.y),
            ),
            rotation: (a.value.rotation + rotation_delta * t).rem_euclid(std::f64::consts::TAU),
            scale: Vec2::new(
                lerp(a.value.scale.x, b.value.scale.x),
                lerp(a.value.scale.y, b.value.scale.y),
            ),
        })
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
