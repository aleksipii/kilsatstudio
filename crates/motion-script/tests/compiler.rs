use motion_script::compiler::compile;

const SOURCE: &str = include_str!("../../../examples/motionscript-v1/nested.yaml");

#[test]
fn nested_timing_and_stable_ties() {
    let script = compile(SOURCE).unwrap();
    let times: Vec<_> = script.scenes[0]
        .events
        .iter()
        .map(|event| event.actor_time())
        .collect();
    assert_eq!(
        times,
        vec![(1.0, "EMMA"), (1.0, "ALEX"), (1.5, "EMMA"), (3.0, "ALEX")]
    );
    assert_eq!(
        serde_yaml::to_string(&script).unwrap(),
        serde_yaml::to_string(&compile(SOURCE).unwrap()).unwrap()
    );
    // Compiled output is accepted by the existing editor/renderer parser.
    motion_script::parse(&serde_yaml::to_string(&script).unwrap()).unwrap();
}

#[test]
fn precise_semantic_paths_and_error_categories() {
    for (old, new, code, path) in [
        ("version: 1", "version: 2", "version", "$.version"),
        ("actor: EMMA", "actor: MISSING", "reference", ".event.actor"),
        ("delay: 0.5", "delay: .nan", "time", ".delay"),
        ("delay: 0.5", "delay: -1", "time", ".delay"),
        ("op: wait", "op: unknown", "unsupported", ".op"),
        ("span: 2", "span: 0", "time", ".span"),
        ("type: move", "type: teleport", "unsupported", ".event.type"),
        (
            "at: 0, duration: 2",
            "at: -1, duration: 2",
            "time",
            ".event.at",
        ),
        (
            "duration: 2, x",
            "duration: 3, x",
            "time",
            ".event.duration",
        ),
    ] {
        let error = compile(&SOURCE.replacen(old, new, 1)).unwrap_err();
        assert_eq!(error.code, code, "{error}");
        assert!(error.path.ends_with(path), "{error}");
    }
}

#[test]
fn rejects_unknown_fields_scenes_overflows_and_conflicting_moves() {
    for source in [
        SOURCE.replace("opening:\n", "missing:\n"),
        SOURCE.replace("duration: 8", "duration: 2"),
        SOURCE.replace("delay: 0.5", "delay: 1e308"),
        SOURCE.replace("op: wait, duration: 1", "op: wait, duration: 1, typo: true"),
        SOURCE.replace("value: happy", "value: unrecognized"),
        SOURCE.replace(
            "type: emotion, actor: ALEX, at: 0, value: happy",
            "type: move, actor: EMMA, at: 0, duration: 1, x: 200, y: 200",
        ),
    ] {
        assert!(compile(&source).is_err(), "{source}");
    }
}

#[test]
fn syntax_location_and_resource_limits() {
    let error = compile("version: [\n").unwrap_err();
    assert_eq!(error.code, "syntax");
    assert!(error.line.is_some() && error.column.is_some());
    assert_eq!(compile(&" ".repeat(1_048_577)).unwrap_err().code, "limit");
    let prefix = "version: 1\nscript: {characters: {}, scenes: [{id: s, duration: 100, events: []}]}\nschedules:\n  s: ";
    let nested = format!(
        "{}{}{{op: wait, duration: 1}}{}",
        prefix,
        "{op: seq, children: [".repeat(34),
        "]}".repeat(34)
    );
    assert_eq!(compile(&nested).unwrap_err().code, "limit");
    let many = format!(
        "{prefix}{{op: par, children: [{}]}}",
        vec!["{op: wait, duration: 1}"; 1025].join(",")
    );
    assert_eq!(compile(&many).unwrap_err().code, "limit");
}

#[test]
fn group_durations_zero_delay_and_explicit_speech_span() {
    let source = "version: 1\nscript: {characters: {A: {asset: a}}, scenes: [{id: s, duration: 10, events: []}]}\nschedules:\n  s:\n    op: seq\n    children:\n      - op: stagger\n        delay: 0\n        children:\n          - {op: wait, duration: 3}\n          - {op: wait, duration: 1}\n      - {op: event, span: 2, event: {type: speak, actor: A, at: 0, text: Hi, audio: hi.wav}}\n      - {op: event, span: 1, event: {type: emotion, actor: A, at: 0, value: happy}}\n";
    let script = compile(source).unwrap();
    assert_eq!(script.scenes[0].events[0].actor_time().0, 3.0);
    assert_eq!(script.scenes[0].events[1].actor_time().0, 5.0);
}

#[test]
fn origins_follow_sort_and_preserve_existing_events() {
    let source = SOURCE.replace(
        "events: []",
        "events: [{type: emotion, at: 7, actor: ALEX, value: sad}]",
    );
    let result = motion_script::compiler::compile_with_origins(&source).unwrap();
    assert_eq!(
        result.origins[0].len(),
        result.script.scenes[0].events.len()
    );
    assert_eq!(result.origins[0][4], "$.script.scenes[0].events[0]");
    assert_eq!(
        result.origins[0][0],
        "$.schedules[\"opening\"].children[1].children[0].event"
    );
}

#[test]
fn invalid_example_and_duplicate_schedule_are_rejected() {
    let error = compile(include_str!(
        "../../../examples/motionscript-v1/invalid-target.yaml"
    ))
    .unwrap_err();
    assert_eq!(error.code, "reference");
    assert_eq!(error.path, "$.schedules[\"opening\"].event.actor");
    assert!(compile(&format!("{SOURCE}\n  opening: {{op: wait, duration: 1}}\n")).is_err());
}

#[test]
fn leaf_offsets_exact_scene_end_and_precision_collapse() {
    let source = "version: 1\nscript: {characters: {A: {asset: a}}, scenes: [{id: s, duration: 3, events: []}]}\nschedules:\n  s: {op: event, span: 3, event: {type: move, at: 1, actor: A, duration: 2, x: 10, y: 10}}\n";
    assert_eq!(
        compile(source).unwrap().scenes[0].events[0].actor_time().0,
        1.0
    );
    for bad in [
        source.replace("at: 1", "at: 3"),
        source.replace("span: 3", "span: .inf"),
        source.replace("duration: 2, x", "duration: .nan, x"),
        source.replace("x: 10", "x: .nan"),
    ] {
        assert!(compile(&bad).is_err());
    }
    let empty = "version: 1\nscript: {characters: {}, scenes: [{id: s, duration: 3, events: []}]}\nschedules: {s: {op: par, children: []}}";
    assert!(compile(empty).unwrap_err().path.ends_with(".children"));
    let tiny = "version: 1\nscript: {characters: {}, scenes: [{id: s, duration: 3, events: []}]}\nschedules: {s: {op: seq, children: [{op: wait, duration: 1}, {op: wait, duration: 1e-30}]}}";
    assert_eq!(compile(tiny).unwrap_err().code, "time");
}
