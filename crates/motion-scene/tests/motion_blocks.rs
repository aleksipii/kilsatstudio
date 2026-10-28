//! Gait, facing and gesture events evaluated on an original stick-studio rig.
use motion_scene::{evaluate, FaceView, TURN_SECONDS};
use motion_script::Script;
use serde_json::{json, Value};

/// The stick-studio project with VEIKKO alone in a fresh scene running `events`.
fn script(events: Value) -> Script {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/stick-studio/stick-studio.kilsat");
    let project: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut source: Value = serde_json::from_str(project["source"].as_str().unwrap()).unwrap();
    let veikko = source["characters"]["VEIKKO"].clone();
    source["characters"] = json!({ "VEIKKO": veikko });
    source["scenes"] = json!([{ "id": "s", "duration": 4, "events": events }]);
    motion_script::parse(&source.to_string()).expect("valid script")
}

fn veikko(script: &Script, time: f64) -> motion_scene::ActorPose {
    evaluate(script, 0, time).unwrap().actors.remove(0)
}

fn part<'a>(pose: &'a motion_scene::ActorPose, id: &str) -> &'a motion_rig::EvaluatedPart {
    pose.rigid_parts.iter().find(|p| p.id == id).unwrap()
}

fn mean_x(part: &motion_rig::EvaluatedPart) -> f64 {
    part.points.iter().map(|p| p.x).sum::<f64>() / part.points.len() as f64
}

#[test]
fn moves_swing_the_legs_and_gaits_differ() {
    let moving = |gait: &str| {
        script(
            json!([{ "type": "move", "actor": "VEIKKO", "at": 0, "x": 1500, "y": 756,
                         "duration": 2, "gait": gait }]),
        )
    };
    let rest = veikko(&moving("walk"), 3.0);
    let swing = |gait: &str| {
        let pose = veikko(&moving(gait), 0.1);
        (mean_x(part(&pose, "shin_l")) - mean_x(part(&rest, "shin_l"))).abs()
    };
    assert!(swing("walk") > 1.0, "legs move during a walk");
    assert!(swing("run") > swing("walk"), "a run strides further");
    // Seeking is exact: the same instant gives the same pose in any order.
    let run = moving("run");
    assert_eq!(veikko(&run, 1.3), veikko(&run, 1.3));
    // After the move ends the figure stands at rest again.
    assert_eq!(part(&veikko(&run, 3.0), "thigh_l"), part(&rest, "thigh_l"));
}

#[test]
fn facing_mirrors_turns_and_hides_the_face_from_behind() {
    let s = script(json!([
        { "type": "face", "actor": "VEIKKO", "at": 1, "value": "left" },
        { "type": "face", "actor": "VEIKKO", "at": 2, "value": "back" },
    ]));
    let front = veikko(&s, 0.5);
    assert_eq!((front.view, front.scale.x), (FaceView::Front, 1.0));
    let left = veikko(&s, 1.0 + TURN_SECONDS);
    assert_eq!((left.view, left.scale.x), (FaceView::Side, -1.0));
    // Mid-turn the figure narrows but never collapses to zero width.
    let turning = veikko(&s, 1.0 + TURN_SECONDS * 0.5);
    assert!(turning.scale.x.abs() < 0.5 && turning.scale.x.abs() >= 0.2);
    // The side view moves facial features toward the facing side (+x locally).
    assert!(mean_x(part(&left, "eye_l")) > mean_x(part(&front, "eye_l")));
    let back = veikko(&s, 3.0);
    assert_eq!((back.view, back.scale.x), (FaceView::Back, 1.0));
    for id in ["eye_l", "pupil_r", "mouth", "brow_l"] {
        assert_eq!(part(&back, id).color[3], 0, "{id} hidden from behind");
    }
    assert_ne!(part(&back, "face").color[3], 0, "the head itself stays");
}

#[test]
fn gestures_drive_custom_rig_arms() {
    let s = script(json!([
        { "type": "gesture", "actor": "VEIKKO", "at": 0, "value": "wave", "duration": 2 },
    ]));
    let (rest, wave) = (veikko(&s, 3.0), veikko(&s, 1.0));
    let top = |pose: &motion_scene::ActorPose| {
        part(pose, "hand_r")
            .points
            .iter()
            .map(|p| p.y)
            .fold(f64::MAX, f64::min)
    };
    assert!(top(&wave) < top(&rest) - 100.0, "the right hand rises");
}

#[test]
fn running_left_mirrors_the_cycle_instead_of_running_backward() {
    let run = |x: f64| {
        script(
            json!([{ "type": "move", "actor": "VEIKKO", "at": 0, "x": x, "y": 756,
                         "duration": 2, "gait": "run" }]),
        )
    };
    let (right, left) = (veikko(&run(1800.0), 0.5), veikko(&run(100.0), 0.5));
    // The torso leans toward the direction of travel (parts are actor-local).
    let lean = |pose: &motion_scene::ActorPose| mean_x(part(pose, "neck"));
    assert!(lean(&right) > 0.0 && lean(&left) < 0.0);
}
