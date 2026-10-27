use motion_script::compiler::{compile, Diagnostic, DiagnosticCode};
use std::path::{Path, PathBuf};

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
        assert_eq!(error.code.as_str(), code, "{error}");
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
    assert_eq!(error.code, DiagnosticCode::Syntax);
    assert!(error.line.is_some() && error.column.is_some());
    assert_eq!(
        compile(&" ".repeat(1_048_577)).unwrap_err().code,
        DiagnosticCode::Limit
    );
    let prefix = "version: 1\nscript: {characters: {}, scenes: [{id: s, duration: 100, events: []}]}\nschedules:\n  s: ";
    let nested = format!(
        "{}{}{{op: wait, duration: 1}}{}",
        prefix,
        "{op: seq, children: [".repeat(34),
        "]}".repeat(34)
    );
    assert_eq!(compile(&nested).unwrap_err().code, DiagnosticCode::Limit);
    let many = format!(
        "{prefix}{{op: par, children: [{}]}}",
        vec!["{op: wait, duration: 1}"; 1025].join(",")
    );
    assert_eq!(compile(&many).unwrap_err().code, DiagnosticCode::Limit);
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
    assert_eq!(error.code, DiagnosticCode::Reference);
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
    assert_eq!(compile(tiny).unwrap_err().code, DiagnosticCode::Time);
}

/// One scene of duration 20 with actors A and B, and the given root node.
fn doc(root: &str) -> String {
    format!(
        "version: 1\nscript: {{characters: {{A: {{asset: a}}, B: {{asset: b}}}}, scenes: [{{id: s, duration: 20, events: []}}]}}\nschedules:\n  s: {root}\n"
    )
}

fn emote(actor: &str, span: f64) -> String {
    format!("{{op: event, span: {span}, event: {{type: emotion, actor: {actor}, at: 0, value: happy}}}}")
}

fn starts(source: &str) -> Vec<f64> {
    compile(source)
        .unwrap_or_else(|error| panic!("{error}\n{source}"))
        .scenes[0]
        .events
        .iter()
        .map(|event| event.actor_time().0)
        .collect()
}

fn diagnostic(source: &str) -> Diagnostic {
    compile(source).expect_err(source)
}

#[test]
fn seq_children_start_after_preceding_durations() {
    let root = format!(
        "{{op: seq, children: [{}, {{op: wait, duration: 2.5}}, {}, {}]}}",
        emote("A", 1.0),
        emote("A", 3.0),
        emote("B", 1.0)
    );
    // 0, then 1 + 2.5, then 3.5 + 3.
    assert_eq!(starts(&doc(&root)), vec![0.0, 3.5, 6.5]);
    // seq duration is the sum (1 + 2.5 + 3 + 1 = 7.5): a following sibling starts there.
    let outer = format!("{{op: seq, children: [{root}, {}]}}", emote("B", 1.0));
    assert_eq!(starts(&doc(&outer)).last(), Some(&7.5));
}

#[test]
fn par_children_share_start_and_last_until_longest() {
    let par = format!(
        "{{op: par, children: [{}, {}, {{op: wait, duration: 4}}]}}",
        emote("A", 1.0),
        emote("B", 2.0)
    );
    assert_eq!(starts(&doc(&par)), vec![0.0, 0.0]);
    let outer = format!("{{op: seq, children: [{par}, {}]}}", emote("A", 1.0));
    assert_eq!(starts(&doc(&outer)), vec![0.0, 0.0, 4.0]);
}

#[test]
fn stagger_offsets_each_child_by_index_times_delay() {
    let stagger = format!(
        "{{op: stagger, delay: 0.75, children: [{}, {}, {}]}}",
        emote("A", 5.0),
        emote("B", 1.0),
        emote("A", 1.0)
    );
    // Starts i * delay; duration max(i * delay + D_i) = max(5, 1.75, 2.5) = 5.
    let outer = format!("{{op: seq, children: [{stagger}, {}]}}", emote("B", 1.0));
    assert_eq!(starts(&doc(&outer)), vec![0.0, 0.75, 1.5, 5.0]);
}

#[test]
fn nested_groups_compose_start_offsets() {
    let inner = format!(
        "{{op: stagger, delay: 1, children: [{}, {}]}}",
        emote("A", 1.0),
        emote("B", 1.0)
    );
    let root = format!(
        "{{op: seq, children: [{{op: wait, duration: 2}}, {{op: par, children: [{inner}, {{op: seq, children: [{{op: wait, duration: 0.5}}, {}]}}]}}]}}",
        emote("A", 1.0)
    );
    assert_eq!(starts(&doc(&root)), vec![2.0, 2.5, 3.0]);
}

#[test]
fn intervals_are_half_open() {
    // Same-actor moves that touch at t = 2 do not overlap.
    let mv = |at: f64| {
        format!("{{op: event, span: 2, event: {{type: move, actor: A, at: {at}, duration: 2, x: 1, y: 1}}}}")
    };
    let touching = format!("{{op: seq, children: [{}, {}]}}", mv(0.0), mv(0.0));
    assert_eq!(starts(&doc(&touching)), vec![0.0, 2.0]);
    // A node may end exactly at the scene end (20) but not after it.
    let at_end = format!(
        "{{op: seq, children: [{{op: wait, duration: 19}}, {}]}}",
        emote("A", 1.0)
    );
    assert_eq!(starts(&doc(&at_end)), vec![19.0]);
    let past_end = at_end.replace("duration: 19", "duration: 19.5");
    assert_eq!(diagnostic(&doc(&past_end)).code, DiagnosticCode::Time);
    // event.at must be strictly inside [0, span).
    let at_span = "{op: event, span: 1, event: {type: emotion, actor: A, at: 1, value: sad}}";
    let error = diagnostic(&doc(at_span));
    assert_eq!(
        (error.code, error.path.as_str()),
        (DiagnosticCode::Time, "$.schedules[\"s\"].event.at")
    );
}

#[test]
fn every_diagnostic_code_is_reachable_and_located() {
    let ok = doc(&emote("A", 1.0));
    let overlapping = format!(
        "{{op: par, children: [{m}, {m}]}}",
        m = "{op: event, span: 2, event: {type: move, actor: A, at: 0, duration: 2, x: 1, y: 1}}"
    );
    let cases = [
        ("version: [".to_string(), DiagnosticCode::Syntax, "$"),
        (
            ok.replace("\n  s:", "\n  s: {op: wait, duration: 1}\n  s:"),
            DiagnosticCode::Syntax,
            "$",
        ),
        (ok.replace("version: 1\n", ""), DiagnosticCode::Schema, "$"),
        (
            ok.replace("schedules:", "extra: 1\nschedules:"),
            DiagnosticCode::Schema,
            "$",
        ),
        (
            ok.replace("op: event", "op: event, label: x"),
            DiagnosticCode::Schema,
            "$.schedules[\"s\"]",
        ),
        (
            ok.replace("version: 1", "version: 2"),
            DiagnosticCode::Version,
            "$.version",
        ),
        (
            ok.replace("\n  s:", "\n  t:"),
            DiagnosticCode::Reference,
            "$.schedules[\"t\"]",
        ),
        (
            ok.replace("actor: A,", "actor: Z,"),
            DiagnosticCode::Reference,
            "$.schedules[\"s\"].event.actor",
        ),
        (
            ok.replace("span: 1", "span: -1"),
            DiagnosticCode::Time,
            "$.schedules[\"s\"].span",
        ),
        (
            ok.replace("op: event", "op: repeat"),
            DiagnosticCode::Unsupported,
            "$.schedules[\"s\"].op",
        ),
        (
            ok.replace("type: emotion", "type: fade_in"),
            DiagnosticCode::Unsupported,
            "$.schedules[\"s\"].event.type",
        ),
        (
            doc(&overlapping),
            DiagnosticCode::Validation,
            "$.schedules[\"s\"].children[1].event",
        ),
        (" ".repeat(1_048_577), DiagnosticCode::Limit, "$"),
    ];
    for (source, code, path) in cases {
        let error = diagnostic(&source);
        assert_eq!((error.code, error.path.as_str()), (code, path), "{error}");
        assert!(error.to_string().starts_with(code.as_str()), "{error}");
    }
}

#[test]
fn same_source_compiles_to_identical_output() {
    let source = include_str!("../../../examples/motionscript-v1/nested.yaml");
    let first = motion_script::compiler::compile_with_origins(source).unwrap();
    for _ in 0..8 {
        let again = motion_script::compiler::compile_with_origins(source).unwrap();
        assert_eq!(
            serde_yaml::to_string(&again.script).unwrap(),
            serde_yaml::to_string(&first.script).unwrap()
        );
        assert_eq!(again.origins, first.origins);
    }
}

fn repo_examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn yaml_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "yaml"))
        .collect();
    files.sort();
    files
}

#[test]
fn legacy_examples_still_parse_and_wrap_unchanged_in_version_1() {
    let files = yaml_files(&repo_examples());
    assert!(files.len() >= 5, "{files:?}");
    for path in files {
        let source = std::fs::read_to_string(&path).unwrap();
        let legacy = motion_script::parse(&source)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let indented: String = source.lines().map(|line| format!("  {line}\n")).collect();
        let wrapped = format!("version: 1\nscript:\n{indented}schedules: {{}}\n");
        let compiled =
            compile(&wrapped).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(
            serde_yaml::to_string(&compiled).unwrap(),
            serde_yaml::to_string(&legacy).unwrap(),
            "{}",
            path.display()
        );
    }
}

#[test]
fn invalid_examples_report_their_expected_diagnostic() {
    let dir = repo_examples().join("motionscript-v1");
    let mut checked = 0;
    for path in yaml_files(&dir) {
        let source = std::fs::read_to_string(&path).unwrap();
        let Some(expect) = source
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("# expect: "))
        else {
            compile(&source).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            continue;
        };
        let (code, expected_path) = expect.split_once(' ').unwrap();
        let error = diagnostic(&source);
        assert_eq!(
            (error.code.as_str(), error.path.as_str()),
            (code, expected_path),
            "{}",
            path.display()
        );
        checked += 1;
    }
    assert!(checked >= 3);
}

#[test]
fn envelope_json_schema_matches_compiler() {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../schema/motionscript-schedule-v1.schema.json"
    ))
    .unwrap();
    assert_eq!(schema["properties"]["version"]["const"], 1);
    let mut ops: Vec<_> = schema["$defs"]["node"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| {
            node["properties"]["op"]["const"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    ops.sort();
    assert_eq!(ops, ["event", "par", "seq", "stagger", "wait"]);
}
