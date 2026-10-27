//! EventLog v1 and rule expansion with external input (A03).
use motion_script::event_log::{MAX_EVENT_LOG_SAMPLE_RATE, MAX_LOGGED_EVENTS};
use motion_script::{
    parse, to_source, Error, EventLog, EventLogError, EventSource, ExternalEvent, FiringSource,
    LoggedEvent, RuleError, Script,
};

const RATE: u32 = 24_000;

const SCRIPT: &str = "
animation_clips:
  wave_head:
    duration: 1
    bone_tracks: {head: [{at: 0, x: 2}, {at: 1, x: 4}]}
characters:
  A: {asset: a}
  B: {asset: b}
  C: {asset: c, rig: {version: 1, id: c, joints: [{id: root, parent: null}, {id: arm, parent: root, x: 5}], parts: [{id: a, joint: arm, points: [[0, 0], [2, 0], [0, 2]], color: [1, 2, 3, 255]}]}}
scenes:
  - id: one
    duration: 10
    events:
      - {type: gesture, at: 2, actor: A, value: wave, duration: 1}
      - {type: emotion, at: 1, actor: A, value: happy}
  - id: two
    duration: 10
    events: []
rules:
  jump:
    on: key
    value: space
    when: {actor: A}
    then: {type: clip, clip: wave_head, duration: 1}
  any_key:
    on: key
    then: {type: clip, clip: wave_head, actor: B, offset: 0.5, duration: 1, blend_mode: additive}
  waved:
    on: gesture
    then: {type: clip, clip: wave_head, duration: 1}
";

fn script() -> Script {
    parse(SCRIPT).unwrap()
}

fn event(scene: &str, seconds: f64, source: EventSource, value: ExternalEvent) -> LoggedEvent {
    LoggedEvent {
        scene: scene.into(),
        sample: (seconds * f64::from(RATE)) as u64,
        source,
        value,
    }
}

fn key(scene: &str, seconds: f64, name: &str) -> LoggedEvent {
    event(
        scene,
        seconds,
        EventSource::Keyboard,
        ExternalEvent::Key { key: name.into() },
    )
}

fn drag(actor: &str, joint: &str) -> ExternalEvent {
    ExternalEvent::Drag {
        actor: actor.into(),
        joint: joint.into(),
        x: 1.5,
        y: -2.0,
    }
}

fn log(events: Vec<LoggedEvent>) -> EventLog {
    EventLog {
        events,
        ..EventLog::new(RATE)
    }
}

fn sample_log() -> EventLog {
    log(vec![
        key("one", 5.0, "space"),
        key("two", 1.0, "space"),
        event("one", 5.5, EventSource::Pointer, drag("A", "head")),
        key("one", 3.0 + 1.0, "x"),
        event(
            "one",
            6.0,
            EventSource::Pointer,
            ExternalEvent::Release {
                actor: "A".into(),
                joint: "head".into(),
            },
        ),
        event(
            "two",
            2.0,
            EventSource::Camera,
            ExternalEvent::CameraPose {
                roll: 0.2,
                yaw: None,
                pitch: Some(-0.1),
            },
        ),
        event("two", 3.0, EventSource::Manual, drag("C", "arm")),
    ])
}

fn ok_sorted() -> EventLog {
    // Same as `sample_log` but scene `one` is in sample order.
    let mut log = sample_log();
    log.events.swap(2, 3);
    log.events.swap(0, 2);
    log
}

#[test]
fn json_round_trip_is_stable_and_versioned() {
    let log = ok_sorted();
    let json = log.to_json().unwrap();
    assert!(json.contains("\"version\": 1"));
    assert!(json.contains("\"sample_rate\": 24000"));
    assert!(json.contains("\"type\": \"camera_pose\""));
    let back = EventLog::from_json(&json).unwrap();
    assert_eq!(back, log);
    assert_eq!(back.to_json().unwrap(), json);
    back.validate_for(&script()).unwrap();
}

#[test]
fn version_and_syntax_errors_are_typed() {
    let v2 = r#"{"version": 2, "sample_rate": 24000, "events": [], "future": true}"#;
    assert_eq!(
        EventLog::from_json(v2),
        Err(EventLogError::UnsupportedVersion(2))
    );
    for bad in [
        r#"{"version": 1, "sample_rate": 24000, "extra": 1}"#,
        r#"{"version": 1, "sample_rate": 24000, "events": [{"scene": "one", "sample": 0, "source": "keyboard", "value": {"type": "swipe"}}]}"#,
        r#"{"version": 1, "sample_rate": 24000, "events": [{"scene": "one", "sample": -1, "source": "keyboard", "value": {"type": "key", "key": "a"}}]}"#,
        r#"{"version": 1, "sample_rate": 24000, "events": [{"scene": "one", "sample": 0.5, "source": "keyboard", "value": {"type": "key", "key": "a"}}]}"#,
        "not json",
    ] {
        assert!(
            matches!(EventLog::from_json(bad), Err(EventLogError::Syntax(_))),
            "{bad}"
        );
    }
}

fn invalid_path(log: &EventLog) -> String {
    match log.validate() {
        Err(EventLogError::Invalid { path, .. }) => path,
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn structural_validation_rejects_non_finite_unbounded_and_mismatched_events() {
    let mut nan = log(vec![event(
        "one",
        1.0,
        EventSource::Pointer,
        drag("A", "head"),
    )]);
    if let ExternalEvent::Drag { x, .. } = &mut nan.events[0].value {
        *x = f64::NAN;
    }
    assert_eq!(invalid_path(&nan), "events[0].value.x");

    let roll = log(vec![event(
        "one",
        1.0,
        EventSource::Camera,
        ExternalEvent::CameraPose {
            roll: 2.0,
            yaw: None,
            pitch: None,
        },
    )]);
    assert_eq!(invalid_path(&roll), "events[0].value.roll");

    let yaw = log(vec![event(
        "one",
        1.0,
        EventSource::Camera,
        ExternalEvent::CameraPose {
            roll: 0.0,
            yaw: Some(f64::INFINITY),
            pitch: None,
        },
    )]);
    assert_eq!(invalid_path(&yaw), "events[0].value.yaw");

    let source = log(vec![event(
        "one",
        1.0,
        EventSource::Camera,
        ExternalEvent::Key { key: "a".into() },
    )]);
    assert_eq!(invalid_path(&source), "events[0].source");

    assert_eq!(
        invalid_path(&log(vec![key("one", 1.0, "no spaces")])),
        "events[0].value.key"
    );
    assert_eq!(
        invalid_path(&log(vec![key("", 1.0, "a")])),
        "events[0].scene"
    );

    for rate in [0, MAX_EVENT_LOG_SAMPLE_RATE + 1] {
        assert_eq!(invalid_path(&EventLog::new(rate)), "sample_rate");
    }

    let many = log(vec![key("one", 0.0, "a"); MAX_LOGGED_EVENTS + 1]);
    assert!(matches!(many.validate(), Err(EventLogError::Bound(_))));
    assert!(log(vec![key("one", 0.0, "a"); MAX_LOGGED_EVENTS])
        .validate()
        .is_ok());
}

#[test]
fn samples_must_not_decrease_within_a_scene() {
    // Interleaved scenes are fine; only per-scene order matters.
    log(vec![
        key("one", 1.0, "a"),
        key("two", 0.5, "a"),
        key("one", 1.0, "b"),
    ])
    .validate()
    .unwrap();
    let bad = log(vec![
        key("one", 2.0, "a"),
        key("two", 9.0, "a"),
        key("one", 1.0, "a"),
    ]);
    assert_eq!(
        bad.validate(),
        Err(EventLogError::Order {
            path: "events[2].sample".into(),
            scene: "one".into(),
            sample: 24_000,
            previous: 48_000,
        })
    );
    assert!(matches!(
        sample_log().validate(),
        Err(EventLogError::Order { .. })
    ));
}

#[test]
fn references_are_checked_against_the_script() {
    let script = script();
    let reference = |log: EventLog| match log.validate_for(&script) {
        Err(EventLogError::UnknownReference { path, kind, name }) => (path, kind, name),
        other => panic!("expected UnknownReference, got {other:?}"),
    };
    assert_eq!(
        reference(log(vec![key("three", 0.0, "a")])),
        ("events[0].scene".into(), "scene", "three".into())
    );
    assert_eq!(
        reference(log(vec![event(
            "one",
            0.0,
            EventSource::Pointer,
            drag("Z", "head")
        )])),
        ("events[0].value.actor".into(), "actor", "Z".into())
    );
    // Humanoid actors use the humanoid bones; custom rigs use their own joints.
    assert_eq!(
        reference(log(vec![event(
            "one",
            0.0,
            EventSource::Pointer,
            drag("A", "arm")
        )])),
        ("events[0].value.joint".into(), "joint", "arm".into())
    );
    assert_eq!(
        reference(log(vec![event(
            "one",
            0.0,
            EventSource::Pointer,
            drag("C", "head")
        )])),
        ("events[0].value.joint".into(), "joint", "head".into())
    );
    // Scene time is half-open: the last valid sample is just before the end.
    let end = LoggedEvent {
        sample: 10 * u64::from(RATE),
        ..key("one", 0.0, "a")
    };
    assert!(matches!(
        log(vec![end.clone()]).validate_for(&script),
        Err(EventLogError::Invalid { ref path, .. }) if path == "events[0].sample"
    ));
    let last = LoggedEvent {
        sample: end.sample - 1,
        ..end
    };
    log(vec![last]).validate_for(&script).unwrap();
}

#[test]
fn same_script_and_log_give_same_firings_and_expansion() {
    let script = script();
    let log = ok_sorted();
    let a = script.expand_rules_with_events(&log).unwrap();
    let b = script.expand_rules_with_events(&log).unwrap();
    assert_eq!(a.firings, b.firings);
    assert_eq!(to_source(&a.script).unwrap(), to_source(&b.script).unwrap());
    // A replay from the serialized log is identical too.
    let replayed = EventLog::from_json(&log.to_json().unwrap()).unwrap();
    let c = script.expand_rules_with_events(&replayed).unwrap();
    assert_eq!(c.firings, a.firings);
    assert_eq!(to_source(&c.script).unwrap(), to_source(&a.script).unwrap());

    let order: Vec<_> = a
        .firings
        .iter()
        .map(|f| {
            (
                f.scene.as_str(),
                f.rule.as_str(),
                f.time,
                f.source,
                f.clip_instance.as_deref(),
            )
        })
        .collect();
    // Rules by ascending name, then firings by sample; `n` counts per rule and scene.
    assert_eq!(
        order,
        [
            (
                "one",
                "any_key",
                4.0,
                FiringSource::Logged(0),
                Some("r_any_key_1")
            ),
            (
                "one",
                "any_key",
                5.0,
                FiringSource::Logged(2),
                Some("r_any_key_2")
            ),
            (
                "one",
                "jump",
                5.0,
                FiringSource::Logged(2),
                Some("r_jump_1")
            ),
            (
                "one",
                "waved",
                2.0,
                FiringSource::Authored(0),
                Some("r_waved_1")
            ),
            (
                "two",
                "any_key",
                1.0,
                FiringSource::Logged(1),
                Some("r_any_key_1")
            ),
            (
                "two",
                "jump",
                1.0,
                FiringSource::Logged(1),
                Some("r_jump_1")
            ),
        ]
    );
    let one = &a.script.scenes[0].composition.clip_instances;
    let placed: Vec<_> = one
        .iter()
        .map(|i| (i.id.as_str(), i.actor.as_str(), i.at))
        .collect();
    assert_eq!(
        placed,
        [
            ("r_any_key_1", "B", 4.5),
            ("r_any_key_2", "B", 5.5),
            ("r_jump_1", "A", 5.0),
            ("r_waved_1", "A", 2.0),
        ]
    );
    assert!(a.script.rules.is_empty());
}

#[test]
fn without_key_presses_expansion_equals_expand_rules() {
    let script = script();
    let plain = script.expand_rules().unwrap();
    // `expand_rules` never fires key rules.
    assert_eq!(plain.scenes[0].composition.clip_instances.len(), 1);
    let pointer_only = log(vec![event(
        "one",
        1.0,
        EventSource::Pointer,
        drag("A", "head"),
    )]);
    let with = script.expand_rules_with_events(&pointer_only).unwrap();
    assert_eq!(to_source(&with.script).unwrap(), to_source(&plain).unwrap());
    let empty = script
        .expand_rules_with_events(&EventLog::new(RATE))
        .unwrap();
    assert_eq!(
        to_source(&empty.script).unwrap(),
        to_source(&plain).unwrap()
    );
    assert_eq!(empty.firings.len(), 1);
}

#[test]
fn logged_events_and_rule_outputs_do_not_chain() {
    // `waved` matches authored gestures only: neither the key-generated clips
    // nor the logged events add gestures, so it fires exactly once.
    let script = script();
    let expansion = script.expand_rules_with_events(&ok_sorted()).unwrap();
    assert_eq!(
        expansion
            .firings
            .iter()
            .filter(|f| f.rule == "waved")
            .count(),
        1
    );
    // Expanding the expanded script again changes nothing (rules are cleared).
    let again = expansion
        .script
        .expand_rules_with_events(&ok_sorted())
        .unwrap();
    assert!(again.firings.is_empty());
    assert_eq!(
        to_source(&again.script).unwrap(),
        to_source(&expansion.script).unwrap()
    );
}

#[test]
fn key_rule_validation_and_bounds() {
    let rule = |body: &str| {
        let source = SCRIPT.split("rules:").next().unwrap().to_string() + "rules:\n" + body;
        parse(&source)
    };
    let invalid = |body: &str| match rule(body) {
        Err(Error::Validation(message)) => message,
        other => panic!("expected validation error, got {other:?}"),
    };
    assert!(
        invalid("  k:\n    on: key\n    then: {type: clip, clip: wave_head, duration: 1}\n")
            .contains("key trigger with a clip needs an actor")
    );
    assert!(invalid("  k:\n    on: key\n    at: 1\n    when: {actor: A}\n    then: {type: clip, clip: wave_head, duration: 1}\n")
        .contains("only time triggers take at"));
    assert!(invalid("  k:\n    on: key\n    value: 'two words'\n    when: {actor: A}\n    then: {type: clip, clip: wave_head, duration: 1}\n")
        .contains("key value"));
    assert!(invalid("  k:\n    on: key\n    when: {emotion: happy}\n    then: {type: camera, preset: push_in, duration: 1}\n")
        .contains("emotion condition on a key trigger needs an actor"));
    // Camera key rules need no actor; emotion conditions read the subject's state.
    let camera = rule("  k:\n    on: key\n    when: {actor: A, emotion: happy}\n    then: {type: camera, preset: push_in, duration: 1}\n").unwrap();
    let presses = log(vec![key("one", 0.5, "a"), key("one", 3.0, "a")]);
    let firings = camera.expand_rules_with_events(&presses).unwrap().firings;
    assert_eq!(firings.len(), 1, "neutral at 0.5 s, happy at 3 s");
    assert_eq!(firings[0].clip_instance, None);

    // Output bounds apply to logged firings as to authored ones.
    let flood = log((0..129)
        .map(|i| key("one", f64::from(i) * 0.01, "a"))
        .collect());
    let small = rule("  k:\n    on: key\n    when: {actor: B}\n    then: {type: clip, clip: wave_head, duration: 0.01}\n").unwrap();
    assert!(matches!(
        small.expand_rules_with_events(&flood),
        Err(RuleError::Bound(_))
    ));
}

#[test]
fn log_errors_surface_through_rule_expansion() {
    let script = script();
    let unknown = log(vec![key("nowhere", 0.0, "a")]);
    let error = script.expand_rules_with_events(&unknown).unwrap_err();
    assert!(matches!(
        error,
        RuleError::EventLog(EventLogError::UnknownReference { .. })
    ));
    assert!(matches!(Error::from(error), Error::Validation(_)));
    let future = EventLog {
        version: 2,
        ..EventLog::new(RATE)
    };
    let error = script.expand_rules_with_events(&future).unwrap_err();
    assert!(matches!(Error::from(error), Error::Unsupported(_)));
}

#[test]
fn emotion_condition_uses_latest_event_with_stable_ties() {
    // Emotion state is the latest event at or before the trigger time; for equal
    // times the later authored event wins, regardless of authored position.
    let source = |at: f64, wanted: &str| {
        format!(
            "animation_clips:\n  c: {{duration: 1, bone_tracks: {{head: [{{at: 0, x: 0}}, {{at: 1, x: 1}}]}}}}\n\
             characters:\n  A: {{asset: a}}\nscenes:\n  - id: s\n    duration: 10\n    events:\n\
             \x20     - {{type: emotion, at: 3, actor: A, value: sad}}\n\
             \x20     - {{type: emotion, at: 1, actor: A, value: happy}}\n\
             \x20     - {{type: emotion, at: 3, actor: A, value: angry}}\n\
             rules:\n  r: {{on: time, at: {at}, when: {{actor: A, emotion: {wanted}}}, then: {{type: clip, clip: c, duration: 1}}}}\n"
        )
    };
    let fires = |at: f64, wanted: &str| {
        let script = parse(&source(at, wanted)).unwrap();
        script.expand_rules().unwrap().scenes[0]
            .composition
            .clip_instances
            .len()
            == 1
    };
    assert!(fires(0.5, "neutral"));
    assert!(fires(1.0, "happy"));
    assert!(fires(2.9, "happy"));
    assert!(fires(3.0, "angry"));
    assert!(!fires(3.0, "sad"));
    assert!(fires(8.0, "angry"));
}

#[test]
fn bundled_examples_match_their_documented_result() {
    let valid =
        EventLog::from_json(include_str!("../../../examples/event-log-v1/valid.json")).unwrap();
    assert_eq!(valid, ok_sorted());
    valid.validate_for(&script()).unwrap();
    assert!(matches!(
        EventLog::from_json(include_str!("../../../examples/event-log-v1/invalid-order.json")),
        Err(EventLogError::Order { ref path, .. }) if path == "events[1].sample"
    ));
}
