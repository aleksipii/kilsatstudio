//! Run: cargo run -p motion-animation --example blend_pose
use motion_animation::{blend_pose, Easing, Keyframe, PoseTrack, Track, Transform2, Vec2};
use motion_rig::{Joint, Rig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rig = Rig::new(vec![
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
    ])?;
    let root_track = Track::new(vec![
        Keyframe {
            time: 0.0,
            value: Transform2::default(),
            easing: Easing::Linear,
        },
        Keyframe {
            time: 2.0,
            value: Transform2 {
                translation: Vec2::new(10.0, 0.0),
                rotation: std::f64::consts::FRAC_PI_2,
                ..Default::default()
            },
            easing: Easing::Linear,
        },
    ])?;
    let animation = PoseTrack::new(rig, vec![Some(root_track), None])?;
    let rest = animation.rig().rest_pose();
    let sampled = animation.sample(1.0)?;
    for weight in [0.0, 0.5, 1.0] {
        let pose = blend_pose(animation.rig(), &rest, &sampled, weight)?;
        let world = animation.rig().evaluate(&pose)?;
        println!(
            "weight={weight:.1}: root=({:.3}, {:.3}), hand=({:.3}, {:.3})",
            world[0].tx, world[0].ty, world[1].tx, world[1].ty
        );
    }
    Ok(())
}
