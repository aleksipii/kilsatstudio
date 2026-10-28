//! Attachment contract for the built-in seven-joint humanoid (KS-003a): hands
//! hang from the chest (shoulders), feet from the hip (pelvis), and no clip
//! stretches a limb far beyond its rest reach.
use motion_animation::{
    evaluate_humanoid, evaluate_humanoid_with_offsets, Gesture, HumanoidState, Transform2, Vec2,
    CHEST, GESTURES, HIP, LEFT_FOOT, LEFT_HAND, RIGHT_FOOT, RIGHT_HAND, WALK_CYCLE_SECONDS,
};

const EMOTIONS: [&str; 5] = ["neutral", "happy", "sad", "surprised", "angry"];
/// Sampled walk phases and gesture progress values.
const SAMPLES: usize = 24;

fn distance(a: Vec2, b: Vec2) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn every_clip_pose() -> Vec<[Vec2; 7]> {
    let mut poses = Vec::new();
    for emotion in EMOTIONS {
        let changes = [(emotion, 1.0)];
        for i in 0..=SAMPLES {
            let u = i as f64 / SAMPLES as f64;
            let gestures = GESTURES
                .iter()
                .map(|name| Some((Gesture::parse(name).unwrap(), u, 1.0)))
                .chain([None]);
            for gesture in gestures {
                for walk_seconds in [None, Some(u * WALK_CYCLE_SECONDS)] {
                    poses.push(evaluate_humanoid(&HumanoidState {
                        walk_seconds,
                        emotion_changes: &changes,
                        gesture,
                        ..Default::default()
                    }));
                }
            }
        }
    }
    poses
}

#[test]
fn hands_follow_the_chest_and_feet_follow_the_hip() {
    let rest = evaluate_humanoid(&HumanoidState::default());
    let shift = |joint: usize| {
        evaluate_humanoid_with_offsets(
            &HumanoidState::default(),
            &[(
                joint,
                Transform2 {
                    translation: Vec2::new(5.0, -7.0),
                    ..Default::default()
                },
            )],
        )
        .unwrap()
    };
    let moved = |a: Vec2, b: Vec2| (b.x - a.x, b.y - a.y);
    let chest = shift(CHEST);
    for hand in [LEFT_HAND, RIGHT_HAND] {
        assert_eq!(moved(rest[hand], chest[hand]), (5.0, -7.0), "hand {hand}");
    }
    for foot in [LEFT_FOOT, RIGHT_FOOT] {
        assert_eq!(chest[foot], rest[foot], "foot {foot} ignores the chest");
    }
    let hip = shift(HIP);
    for joint in [CHEST, LEFT_HAND, RIGHT_HAND, LEFT_FOOT, RIGHT_FOOT] {
        assert_eq!(moved(rest[joint], hip[joint]), (5.0, -7.0), "joint {joint}");
    }
}

#[test]
fn no_clip_detaches_or_overstretches_a_limb() {
    let rest = evaluate_humanoid(&HumanoidState::default());
    let arm = distance(rest[CHEST], rest[RIGHT_HAND]);
    let leg = distance(rest[HIP], rest[RIGHT_FOOT]);
    let poses = every_clip_pose();
    assert!(poses.len() > 1000);
    for pose in poses {
        for (anchor, end, length) in [
            (CHEST, LEFT_HAND, arm),
            (CHEST, RIGHT_HAND, arm),
            (HIP, LEFT_FOOT, leg),
            (HIP, RIGHT_FOOT, leg),
        ] {
            let d = distance(pose[anchor], pose[end]);
            assert!(d.is_finite(), "{pose:?}");
            // The built-in humanoid draws each limb as one line from its anchor,
            // so it stays attached at any length; the bound catches runaway
            // clips. Pointing while surprised and walking reaches about 1.76x rest.
            assert!(
                d >= 0.25 * length && d <= 2.0 * length,
                "joint {end}: {d} vs rest {length}"
            );
        }
    }
}
