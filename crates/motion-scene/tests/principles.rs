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
