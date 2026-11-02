//! `principles:` blocks evaluated on an original stick-studio rig.
use motion_scene::{evaluate, ValidatedScript};
use motion_script::Script;
use serde_json::{json, Value};
use std::sync::Arc;

/// The stick-studio project with VEIKKO alone in a fresh scene running
/// `events`; `principles` is VEIKKO's block, if any.
fn script(principles: Option<Value>, events: Value) -> Script {
    try_script(principles, events).expect("valid script")
}

fn try_script(principles: Option<Value>, events: Value) -> Result<Script, String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/stick-studio/stick-studio.kilsat");
    let project: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut source: Value = serde_json::from_str(project["source"].as_str().unwrap()).unwrap();
    let mut veikko = source["characters"]["VEIKKO"].clone();
    if let Some(principles) = principles {
        veikko["principles"] = principles;
    }
    source["characters"] = json!({ "VEIKKO": veikko });
    source["scenes"] = json!([{ "id": "s", "duration": 6, "events": events }]);
    motion_script::parse(&source.to_string()).map_err(|e| e.to_string())
}

fn parts(script: &Script, time: f64) -> Vec<motion_rig::EvaluatedPart> {
    evaluate(script, 0, time)
        .unwrap()
        .actors
        .remove(0)
        .rigid_parts
}

fn wave(principles: Option<Value>) -> Value {
    let mut event = json!({ "type": "gesture", "actor": "VEIKKO", "at": 0.5,
                            "value": "wave", "duration": 2 });
    if let Some(principles) = principles {
        event["principles"] = principles;
    }
    json!([event])
}

#[test]
fn default_block_values_equal_no_block() {
    let plain = script(None, wave(None));
    let explicit = script(
        Some(
            json!({ "anticipation": 0.12, "overshoot": 1.3, "follow_through": 0.25,
                     "overlap": 0.08, "squash_stretch": 0, "moving_hold": "off" }),
        ),
        wave(None),
    );
    for i in 0..=144 {
        let t = i as f64 / 24.0;
        assert_eq!(parts(&plain, t), parts(&explicit, t), "{t}");
    }
}

#[test]
fn presets_and_gesture_blocks_change_the_motion() {
    let plain = script(None, wave(None));
    let cartoon = script(Some(json!("cartoon")), wave(None));
    let per_gesture = script(None, wave(Some(json!("cartoon"))));
    // During the attack the overshoot and squash and stretch differ.
    let t = 0.5 + 0.3 * 2.0;
    assert_ne!(parts(&plain, t), parts(&cartoon, t));
    // A gesture block alone gives the same gesture as the character preset,
    // except the character-level moving hold, which is inactive mid-gesture.
    assert_eq!(parts(&cartoon, t), parts(&per_gesture, t));
    // After the gesture both are at rest until the moving hold fades in.
    assert_eq!(parts(&plain, 2.6), parts(&per_gesture, 2.6));
}

#[test]
fn moving_hold_breathes_in_long_holds_and_can_be_switched_off() {
    let still = script(None, json!([]));
    let breathing = script(Some(json!("subtle")), json!([]));
    let off = script(
        Some(json!({ "preset": "subtle", "moving_hold": "off" })),
        json!([]),
    );
    // Short holds stay crisp; long holds move a little.
    assert_eq!(parts(&still, 0.3), parts(&breathing, 0.3));
    assert_ne!(parts(&still, 3.0), parts(&breathing, 3.0));
    for i in 0..=144 {
        let t = i as f64 / 24.0;
        assert_eq!(parts(&still, t), parts(&off, t));
    }
    // Deterministic: the same time gives the same pose, and a breath differs
    // over time.
    assert_eq!(parts(&breathing, 4.2), parts(&breathing, 4.2));
    assert_ne!(parts(&breathing, 3.0), parts(&breathing, 4.0));
    // No moving hold while a gesture plays.
    let gesture = script(Some(json!("subtle")), wave(None));
    let plain_gesture = script(None, wave(Some(json!("subtle"))));
    assert_eq!(parts(&gesture, 1.5), parts(&plain_gesture, 1.5));
}

#[test]
fn seeking_matches_frame_by_frame_playback_with_principles() {
    let script = script(
        Some(json!("cartoon")),
        json!([
            { "type": "gesture", "actor": "VEIKKO", "at": 0.2, "value": "jump", "duration": 1.2 },
            { "type": "gesture", "actor": "VEIKKO", "at": 3.5, "value": "wave", "duration": 1.5,
              "principles": { "preset": "snappy", "squash_stretch": 0.3 } }
        ]),
    );
    let validated = ValidatedScript::new(Arc::new(script)).unwrap();
    let played: Vec<_> = (0..=144)
        .map(|frame| validated.evaluate(0, frame as f64 / 24.0).unwrap())
        .collect();
    for frame in [144, 7, 90, 0, 33, 101] {
        assert_eq!(
            validated.evaluate(0, frame as f64 / 24.0).unwrap(),
            played[frame]
        );
    }
}

#[test]
fn invalid_or_unsupported_principles_are_rejected() {
    let message = |result: Result<Script, String>| result.unwrap_err();
    let check = try_script;
    let error = message(check(Some(json!({ "overshoot": 3.5 })), json!([])));
    assert!(
        error.contains("principles.overshoot") && error.contains("0..=3"),
        "{error}"
    );
    let error = message(check(Some(json!("bouncy")), json!([])));
    assert!(error.contains("subtle, cartoon, snappy"), "{error}");
    let error = message(check(None, wave(Some(json!({ "moving_hold": "on" })))));
    assert!(error.contains("moving_hold"), "{error}");
    let error = message(check(None, wave(Some(json!({ "overlap": 0.5 })))));
    assert!(error.contains("principles.overlap"), "{error}");
    // The original seven-joint humanoid has no rig layer to shape.
    let mut legacy = script(None, json!([]));
    let veikko = legacy.characters.get_mut("VEIKKO").unwrap();
    veikko.rig = None;
    veikko.holds = None;
    veikko.principles = motion_script::PrinciplesSpec::default().into();
    let error = legacy.validate().unwrap_err().to_string();
    assert!(error.contains("custom-rig"), "{error}");
}

fn actor(script: &Script, time: f64) -> motion_scene::ActorPose {
    evaluate(script, 0, time).unwrap().actors.remove(0)
}

#[test]
fn overlap_frames_equal_the_matching_fraction() {
    // 3 frames of a 2 s gesture at 24 fps is exactly 3/48 = 0.0625.
    let frames = script(Some(json!({ "overlap_frames": 3 })), wave(None));
    let fraction = script(Some(json!({ "overlap": 0.0625 })), wave(None));
    for i in 0..=144 {
        let t = i as f64 / 24.0;
        assert_eq!(parts(&frames, t), parts(&fraction, t), "{t}");
    }
}

#[test]
fn anticipation_frames_keep_their_length_across_gesture_lengths() {
    let gesture = |duration: f64, principles: Option<Value>| {
        script(
            principles,
            json!([{ "type": "gesture", "actor": "VEIKKO", "at": 0.5,
                     "value": "wave", "duration": duration }]),
        )
    };
    let block = json!({ "anticipation_frames": 5 });
    let rest = parts(&gesture(2.0, None), 0.0);
    for duration in [1.0, 2.0, 4.0] {
        let timed = gesture(duration, Some(block.clone()));
        // Mid-anticipation (frame 2.5 of 5) the pose already moves.
        assert_ne!(parts(&timed, 0.5 + 2.5 / 24.0), rest, "{duration}");
        // A different frame count changes the curve; the rest after the
        // gesture is the same.
        let longer = gesture(duration, Some(json!({ "anticipation_frames": 12 })));
        assert_ne!(
            parts(&timed, 0.5 + 4.0 / 24.0),
            parts(&longer, 0.5 + 4.0 / 24.0),
            "{duration}"
        );
        assert_eq!(parts(&timed, 0.6 + duration), rest, "{duration}");
    }
    let error = try_script(Some(json!({ "anticipation_frames": 0 })), json!([])).unwrap_err();
    assert!(error.contains("anticipation_frames"), "{error}");
}

#[test]
fn arc_moves_travel_on_a_curve_and_seek_like_playback() {
    let straight = script(
        None,
        json!([{ "type": "move", "actor": "VEIKKO", "at": 0.5, "x": 1400, "y": 756,
                 "duration": 2 }]),
    );
    let arced = script(
        None,
        json!([{ "type": "move", "actor": "VEIKKO", "at": 0.5, "x": 1400, "y": 756,
                 "duration": 2, "arc": 0.2 }]),
    );
    let start = actor(&straight, 0.0);
    let end = actor(&straight, 2.5);
    assert_eq!(
        (actor(&arced, 0.0).x, actor(&arced, 0.0).y),
        (start.x, start.y)
    );
    assert_eq!((actor(&arced, 2.5).x, actor(&arced, 2.5).y), (end.x, end.y));
    // Halfway the arc is 0.2 of the distance above the straight path.
    let (line, arc) = (actor(&straight, 1.5), actor(&arced, 1.5));
    let distance = (end.x - start.x).hypot(end.y - start.y);
    assert!(
        (line.y - arc.y - 0.2 * distance).abs() < 1e-6,
        "{} {}",
        line.y,
        arc.y
    );
    let validated = ValidatedScript::new(Arc::new(arced)).unwrap();
    let played: Vec<_> = (0..=72)
        .map(|frame| validated.evaluate(0, frame as f64 / 24.0).unwrap())
        .collect();
    for frame in [72, 13, 40, 0, 36, 59] {
        assert_eq!(
            validated.evaluate(0, frame as f64 / 24.0).unwrap(),
            played[frame]
        );
    }
}

#[test]
fn walks_on_an_arc_step_in_proportion_to_the_longer_route() {
    // A half circle is π/2 times the straight distance, so it steps like a
    // straight walk at pace π/2.
    let walk = |extra: Value| {
        let mut event = json!({ "type": "move", "actor": "VEIKKO", "at": 0.5, "x": 1300,
                                "y": 756, "duration": 3, "gait": "walk" });
        event
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        script(None, json!([event]))
    };
    let arced = walk(json!({ "arc": 0.5 }));
    let paced = walk(json!({ "pace": std::f64::consts::FRAC_PI_2 }));
    let plain = walk(json!({}));
    let mut differs = false;
    for i in 13..=84 {
        let t = i as f64 / 24.0;
        let (a, p) = (actor(&arced, t), actor(&paced, t));
        for (x, y) in a.skeleton.iter().zip(&p.skeleton) {
            assert!((x.x - y.x).abs() < 1e-6 && (x.y - y.y).abs() < 1e-6, "{t}");
        }
        differs |= a.skeleton != actor(&plain, t).skeleton;
    }
    assert!(differs, "the arc changes the step rate");
}
