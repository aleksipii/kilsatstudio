//! Gaits, eased moves, leaps and body poses evaluated on an original
//! stick-studio rig.
use motion_scene::{evaluate, ValidatedScript};
use motion_script::Script;
use serde_json::{json, Value};
use std::sync::Arc;

/// The stick-studio project with VEIKKO alone in a fresh scene running `events`.
fn script(events: Value) -> Script {
    motion_script::parse(&source(events)).expect("valid script")
}

fn source(events: Value) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/stick-studio/stick-studio.kilsat");
    let project: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut source: Value = serde_json::from_str(project["source"].as_str().unwrap()).unwrap();
    let veikko = source["characters"]["VEIKKO"].clone();
    source["characters"] = json!({ "VEIKKO": veikko });
    source["scenes"] = json!([{ "id": "s", "duration": 6, "events": events }]);
    source.to_string()
}

fn actor(script: &Script, time: f64) -> motion_scene::ActorPose {
    evaluate(script, 0, time).unwrap().actors.remove(0)
}

/// Lowest (largest y) point of a part in actor-local space.
fn lowest(pose: &motion_scene::ActorPose, part: &str) -> f64 {
    pose.rigid_parts
        .iter()
        .find(|p| p.id == part)
        .unwrap()
        .points
        .iter()
        .map(|p| p.y)
        .fold(f64::MIN, f64::max)
}

fn highest(pose: &motion_scene::ActorPose, part: &str) -> f64 {
    pose.rigid_parts
        .iter()
        .find(|p| p.id == part)
        .unwrap()
        .points
        .iter()
        .map(|p| p.y)
        .fold(f64::MAX, f64::min)
}

#[test]
fn eased_moves_start_slow_and_arrive_exactly() {
    let moving = |ease: Option<&str>| {
        let mut event = json!({ "type": "move", "actor": "VEIKKO", "at": 1, "x": 1500,
                                "y": 700, "duration": 2, "gait": "run" });
        if let Some(ease) = ease {
            event["ease"] = json!(ease);
        }
        script(json!([event]))
    };
    let (linear, eased) = (moving(None), moving(Some("in_out")));
    let start = actor(&linear, 1.0).x;
    assert_eq!(actor(&eased, 1.0).x, start);
    assert!(actor(&eased, 1.2).x - start < actor(&linear, 1.2).x - start);
    assert_eq!(actor(&eased, 3.0).x, 1500.0);
    assert_eq!(actor(&eased, 2.0).x, actor(&linear, 2.0).x);
    // Seeking matches stepping through every frame.
    let validated = ValidatedScript::new(Arc::new(eased)).unwrap();
    let stepped: Vec<_> = (0..=96)
        .map(|f| validated.evaluate(0, f as f64 / 24.0).unwrap())
        .collect();
    for f in [70, 3, 48, 25, 96] {
        assert_eq!(validated.evaluate(0, f as f64 / 24.0).unwrap(), stepped[f]);
    }
    // An unknown ease is reported as unsupported.
    let bad = source(
        json!([{ "type": "move", "actor": "VEIKKO", "at": 1, "x": 1500,
                               "y": 700, "duration": 2, "ease": "bounce" }]),
    );
    let error = motion_script::parse(&bad).unwrap_err();
    assert!(
        error.to_string().contains("linear, in, out, in_out"),
        "{error}"
    );
}

#[test]
fn every_gait_animates_the_rig_while_moving() {
    let rest = actor(&script(json!([])), 1.4);
    for gait in motion_scene::GAITS {
        let s = script(
            json!([{ "type": "move", "actor": "VEIKKO", "at": 1, "x": 1200,
                                 "y": 700, "duration": 1, "gait": gait }]),
        );
        let pose = actor(&s, 1.4);
        let changed = pose
            .rigid_parts
            .iter()
            .zip(&rest.rigid_parts)
            .any(|(a, b)| a.points != b.points);
        assert!(changed, "{gait}");
        // Back at rest after the move ends.
        assert_eq!(actor(&s, 2.5).rigid_parts, rest.rigid_parts, "{gait}");
    }
}

#[test]
fn a_leap_lifts_the_feet_off_the_floor() {
    let s = script(
        json!([{ "type": "move", "actor": "VEIKKO", "at": 1, "x": 1300,
                             "y": 700, "duration": 1, "gait": "leap" }]),
    );
    let floor = lowest(&actor(&s, 0.5), "foot_l");
    let top = actor(&s, 1.5);
    assert!(lowest(&top, "foot_l") < floor - 40.0);
    assert!(lowest(&top, "foot_r") < floor - 40.0);
}

#[test]
fn body_poses_lower_the_figure_and_lying_turns_it_flat() {
    let posed = |pose: &str| {
        script(json!([{ "type": "pose", "actor": "VEIKKO", "at": 0.5, "value": pose }]))
    };
    let standing = actor(&posed("stand"), 3.0);
    let head_top = highest(&standing, "face");
    let floor = lowest(&standing, "foot_l");
    for face in ["front", "right"] {
        for (pose, drop) in [("crouch", 15.0), ("kneel", 60.0), ("lie", 250.0)] {
            let s = script(json!([
                { "type": "face", "actor": "VEIKKO", "at": 0, "value": face },
                { "type": "pose", "actor": "VEIKKO", "at": 0.5, "value": pose }
            ]));
            let p = actor(&s, 3.0);
            assert!(
                highest(&p, "face") > head_top + drop,
                "{face} {pose} lowers the head"
            );
            // Feet stay on the floor line: none sinks through or floats far above it.
            for foot in ["foot_l", "foot_r"] {
                let y = lowest(&p, foot);
                assert!(
                    (y - floor).abs() < 6.0,
                    "{face} {pose} {foot}: {y} vs {floor}"
                );
            }
        }
    }
    // Lying: head and feet rest at about the same height, near the floor.
    let lying = actor(&posed("lie"), 3.0);
    let (head, foot) = (lowest(&lying, "face"), lowest(&lying, "foot_l"));
    assert!((head - foot).abs() < 40.0, "head {head}, foot {foot}");
    assert!((head - floor).abs() < 40.0, "head {head}, floor {floor}");
}
