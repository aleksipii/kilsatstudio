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

mod timed_props_and_sitting {
    use motion_scene::evaluate;
    use motion_script::Script;

    fn veikko_unchecked(events: &str) -> Script {
        let project: serde_json::Value = serde_json::from_str(include_str!(
            "../../../examples/stick-studio/characters/veikko.kilsat"
        ))
        .unwrap();
        let mut script = motion_script::parse(project["source"].as_str().unwrap()).unwrap();
        script.scenes =
            serde_yaml::from_str(&format!("- id: s\n  duration: 4\n  events:\n{events}")).unwrap();
        script
    }
    fn veikko(events: &str) -> Script {
        let script = veikko_unchecked(events);
        script.validate().unwrap();
        script
    }
    fn part_ids(script: &Script, t: f64) -> Vec<String> {
        let scene = evaluate(script, 0, t).unwrap();
        scene.actors[0]
            .rigid_parts
            .iter()
            .map(|p| p.id.clone())
            .collect()
    }

    #[test]
    fn a_hold_event_picks_up_and_puts_down_a_prop_at_its_time() {
        let script = veikko(
            "      - {type: hold, actor: VEIKKO, at: 1, hand: left, item: kahvikuppi}\n      - {type: hold, actor: VEIKKO, at: 3, hand: left}\n",
        );
        let before = part_ids(&script, 0.5);
        let during = part_ids(&script, 2.0);
        let after = part_ids(&script, 3.5);
        assert!(during.len() > before.len(), "the cup appears");
        assert_eq!(before, after, "and is put down again");
        // Seeking is exact: the same time gives the same parts.
        assert_eq!(during, part_ids(&script, 2.0));
    }

    #[test]
    fn sitting_lowers_the_hips_and_standing_restores_them() {
        let script = veikko(
            "      - {type: pose, actor: VEIKKO, at: 1, value: sit}\n      - {type: pose, actor: VEIKKO, at: 3, value: stand}\n",
        );
        let lowest = |t: f64| {
            let scene = evaluate(&script, 0, t).unwrap();
            scene.actors[0]
                .rigid_parts
                .iter()
                .flat_map(|p| p.points.iter().map(|v| v.y))
                .fold(f64::NEG_INFINITY, f64::max)
        };
        let top = |t: f64| {
            let scene = evaluate(&script, 0, t).unwrap();
            scene.actors[0]
                .rigid_parts
                .iter()
                .flat_map(|p| p.points.iter().map(|v| v.y))
                .fold(f64::INFINITY, f64::min)
        };
        assert!(top(2.0) > top(0.5) + 10.0, "the head drops when seated");
        assert!(
            (top(3.9) - top(0.5)).abs() < 1e-6,
            "standing restores the pose"
        );
        assert!(lowest(2.0).is_finite());
    }

    #[test]
    fn timed_props_and_poses_reject_unsupported_input() {
        let base = motion_script::parse(include_str!("../../../examples/hello-emma.yaml")).unwrap();
        let mut legacy = base.clone();
        legacy.scenes[0].events.push(motion_script::Event::Pose {
            at: 0.0,
            actor: "EMMA".into(),
            value: "sit".into(),
        });
        assert!(legacy.validate().is_err(), "no rig, no sitting");
        let mut hold = base;
        hold.scenes[0].events.push(motion_script::Event::Hold {
            at: 0.0,
            actor: "EMMA".into(),
            hand: motion_rig::HandSide::Left,
            item: Some("kahvikuppi".into()),
        });
        assert!(hold.validate().is_err(), "no rig, no props");
        let banana = veikko_unchecked(
            "      - {type: hold, actor: VEIKKO, at: 1, hand: left, item: banana}\n",
        );
        assert!(banana.validate().is_err(), "unknown prop");
    }
}
