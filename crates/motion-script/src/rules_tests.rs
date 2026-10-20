use crate::{parse, to_source, Error, RuleError, Script};

const BASE: &str = "
animation_clips:
  wave_head:
    duration: 2
    bone_tracks: {head: [{at: 0, x: 2}, {at: 2, x: 4}]}
characters:
  A: {asset: a}
  B: {asset: b}
  C: {asset: c, rig: {version: 1, id: c, joints: [{id: root, parent: null}, {id: arm, parent: root, x: 5}], parts: [{id: a, joint: arm, points: [[0, 0], [2, 0], [0, 2]], color: [1, 2, 3, 255]}]}}
scenes:
  - id: one
    duration: 10
    events:
      - {type: emotion, at: 3, actor: B, value: sad}
      - {type: emotion, at: 1, actor: A, value: happy}
      - {type: emotion, at: 6, actor: A, value: sad}
      - {type: gesture, at: 2, actor: A, value: wave, duration: 1}
      - {type: gesture, at: 7, actor: A, value: nod, duration: 1}
  - id: two
    duration: 10
    events:
      - {type: emotion, at: 1, actor: A, value: happy}
";

fn with_rules(rules: &str) -> String {
    format!("{BASE}rules:\n{rules}")
}
fn expand(rules: &str) -> Result<Script, Error> {
    parse(&with_rules(rules))?
        .expand_rules()
        .map_err(Error::from)
}
const HAPPY: &str = "  smile:\n    on: emotion\n    value: happy\n    then: {type: clip, clip: wave_head, offset: 0.5, duration: 1, weight: 0.5}\n";

#[test]
fn expansion_is_deterministic_ordered_and_idempotent() {
    let rules = format!("{HAPPY}  aaa:\n    on: gesture\n    then: {{type: clip, clip: wave_head, duration: 1, blend_mode: additive}}\n");
    let a = expand(&rules).unwrap();
    let b = expand(&rules).unwrap();
    assert_eq!(to_source(&a).unwrap(), to_source(&b).unwrap());
    let ids: Vec<_> = a.scenes[0]
        .composition
        .clip_instances
        .iter()
        .map(|i| (i.id.as_str(), i.at))
        .collect();
    // Rule names ascend; firings ascend by trigger time regardless of event order.
    assert_eq!(
        ids,
        [("r_aaa_1", 2.0), ("r_aaa_2", 7.0), ("r_smile_1", 1.5)]
    );
    assert_eq!(a.scenes[1].composition.clip_instances.len(), 1);
    assert!(a.rules.is_empty());
    assert_eq!(
        a.expand_rules().unwrap().scenes[0].composition,
        a.scenes[0].composition
    );
}

#[test]
fn conditions_true_and_false() {
    let count = |extra: &str| {
        let rules = format!("  r:\n    on: gesture\n    when: {extra}\n    then: {{type: clip, clip: wave_head, duration: 1}}\n");
        expand(&rules)
            .unwrap()
            .scenes
            .iter()
            .map(|s| s.composition.clip_instances.len())
            .sum::<usize>()
    };
    assert_eq!(count("{}"), 2);
    assert_eq!(count("{emotion: happy}"), 1); // gesture at 2 only; at 7 A is sad
    assert_eq!(count("{emotion: sad}"), 1);
    assert_eq!(count("{emotion: angry}"), 0);
    assert_eq!(count("{scenes: [two]}"), 0);
    assert_eq!(count("{scenes: [one]}"), 2);
    assert_eq!(count("{actor: B}"), 0);
    assert_eq!(count("{actor: A, from: 2, until: 7}"), 1);
    assert_eq!(count("{from: 2, until: 2.5}"), 1);
}

#[test]
fn time_trigger_and_neutral_default() {
    let rules = "  t:\n    on: time\n    at: 0.5\n    when: {actor: B, emotion: neutral}\n    then: {type: clip, clip: wave_head, duration: 1}\n";
    let script = expand(rules).unwrap();
    assert_eq!(script.scenes[0].composition.clip_instances[0].actor, "B");
    assert_eq!(script.scenes[1].composition.clip_instances.len(), 1);
    // B turned sad at 3 in scene one only.
    let script = expand(&rules.replace("at: 0.5", "at: 4")).unwrap();
    assert!(script.scenes[0].composition.clip_instances.is_empty());
    assert_eq!(script.scenes[1].composition.clip_instances.len(), 1);
}

#[test]
fn camera_rules_merge_with_keys_and_conflicts_are_reported() {
    let rule = "  cam:\n    on: emotion\n    value: happy\n    when: {scenes: [one]}\n    then: {type: camera, preset: push_in, duration: 2}\n";
    let script = expand(rule).unwrap();
    let keys = &script.scenes[0].composition.camera;
    assert_eq!(
        (keys[0].at, keys[0].zoom, keys[1].at, keys[1].zoom),
        (1.0, 1.0, 3.0, 1.25)
    );
    // Two firings with overlapping spans conflict.
    let two = rule.replace("    value: happy\n", "");
    assert!(matches!(expand(&two), Err(Error::Validation(m)) if m.contains("overlap")));
    // Authored key inside the span conflicts.
    let authored = BASE.replace(
        "- id: one\n    duration: 10",
        "- id: one\n    duration: 10\n    composition: {camera: [{at: 2, x: 0, y: 0, zoom: 1}]}",
    );
    let source = format!("{authored}rules:\n{rule}");
    assert!(
        matches!(parse(&source), Err(Error::Validation(m)) if m.contains("authored camera key"))
    );
    // Beyond the scene end is rejected by composition validation.
    assert!(expand(&rule.replace("duration: 2", "duration: 20")).is_err());
}

#[test]
fn error_classes() {
    let clip = |body: &str| format!("  r:\n    on: emotion\n    {body}\n");
    let msg = |rules: String| match expand(&rules) {
        Err(Error::Validation(m)) => m,
        Err(Error::Unsupported(m)) => format!("UNSUPPORTED {m}"),
        other => panic!("{other:?}"),
    };
    let then = "then: {type: clip, clip: wave_head, duration: 1}";
    assert!(msg(clip("then: {type: clip, clip: nope, duration: 1}")).contains("unknown clip nope"));
    assert!(msg(clip(
        "then: {type: clip, clip: wave_head, actor: Z, duration: 1}"
    ))
    .contains("unknown actor Z"));
    assert!(
        msg(clip(&format!("when: {{scenes: [zzz]}}\n    {then}"))).contains("unknown scene zzz")
    );
    assert!(msg(clip(&format!("when: {{actor: Q}}\n    {then}"))).contains("unknown actor Q"));
    assert!(msg(clip("then: {type: clip, clip: wave_head, duration: 0}")).contains("duration"));
    assert!(msg(clip("then: {type: clip, clip: wave_head, duration: .nan}")).contains("finite"));
    assert!(msg(clip(
        "then: {type: clip, clip: wave_head, duration: 1, actor: C}"
    ))
    .contains("custom-rig"));
    assert!(msg(clip(&format!("at: 1\n    {then}"))).contains("only time"));
    assert!(msg(clip(&format!("when: {{from: 5, until: 5}}\n    {then}"))).contains("until"));
    assert!(msg(
        "  r:\n    on: time\n    then: {type: camera, preset: pan_left, duration: 1}\n".into()
    )
    .contains("at"));
    // Unsupported: explicit, not ignored.
    assert!(msg(clip("then: {type: shake, duration: 1}")).starts_with("UNSUPPORTED"));
    assert!(msg(
        "  r:\n    on: blink\n    then: {type: camera, preset: pan_left, duration: 1}\n".into()
    )
    .starts_with("UNSUPPORTED"));
    assert!(msg(
        "  r:\n    on: emotion\n    then: {type: camera, preset: spin, duration: 1}\n".into()
    )
    .starts_with("UNSUPPORTED"));
    assert!(msg(clip(&format!("value: furious\n    {then}"))).starts_with("UNSUPPORTED"));
    assert!(
        msg(clip(&format!("when: {{emotion: furious}}\n    {then}"))).starts_with("UNSUPPORTED")
    );
    // Strict schema: unknown fields are syntax errors.
    assert!(matches!(
        expand(&clip(&format!("script: rm\n    {then}"))),
        Err(Error::Syntax(_))
    ));
    // The typed error is available directly.
    let mut script = parse(&with_rules(&clip(then))).unwrap();
    script.rules.get_mut("r").unwrap().condition.actor = Some("X".into());
    assert!(matches!(
        script.expand_rules(),
        Err(RuleError::UnknownReference { .. })
    ));
}

#[test]
fn bounds_and_id_conflicts() {
    let events: String = (0..60)
        .map(|i| {
            format!(
                "{{type: emotion, at: {}, actor: A, value: happy}},",
                i as f64 * 0.1
            )
        })
        .collect();
    let rule =
        |n: &str| format!("  {n}: {{on: emotion, then: {{type: clip, clip: c, duration: 1}}}}\n");
    let source = |rules: &str| {
        format!("animation_clips: {{c: {{duration: 1, bone_tracks: {{head: [{{at: 0, x: 1}}]}}}}}}\ncharacters: {{A: {{asset: a}}}}\nscenes: [{{id: s, duration: 20, events: [{events}]}}]\nrules:\n{rules}")
    };
    // 180 outputs exceed the per-scene bound.
    let three = format!("{}{}{}", rule("a"), rule("b"), rule("c"));
    assert!(matches!(parse(&source(&three)), Err(Error::Validation(m)) if m.contains("exceed")));
    // 60 outputs fit the rule bound but hit the existing 32-per-actor clip instance limit.
    assert!(
        matches!(parse(&source(&rule("a"))), Err(Error::Validation(m)) if m.contains("32 clip instances"))
    );
    // Rule count bound.
    let many: String = (0..65)
        .map(|i| {
            format!("  r{i}: {{on: time, at: 1, when: {{actor: A}}, then: {{type: camera, preset: pan_left, duration: 0.1, offset: {i}}}}}\n")
        })
        .collect();
    let m = format!("characters: {{A: {{asset: a}}}}\nscenes: [{{id: s, duration: 80, events: []}}]\nrules:\n{many}");
    assert!(matches!(parse(&m), Err(Error::Validation(m)) if m.contains("more than")));
    // Name bound and authored ID collision.
    assert!(expand(&HAPPY.replace("smile", &"x".repeat(41))).is_err());
    let collide = BASE.replace(
        "- id: two",
        "- id: two\n    composition: {clip_instances: [{id: r_smile_1, clip: wave_head, actor: A, at: 0, duration: 1}]}",
    );
    let source = format!("{collide}rules:\n{HAPPY}");
    assert!(
        matches!(parse(&source), Err(Error::Validation(m)) if m.contains("duplicates an authored ID"))
    );
}

#[test]
fn round_trip_keeps_rules_and_existing_instances() {
    let authored = BASE.replace(
        "- id: two",
        "- id: two\n    composition: {clip_instances: [{id: hand, clip: wave_head, actor: B, at: 0, duration: 1}]}",
    );
    let script = parse(&format!("{authored}rules:\n{HAPPY}")).unwrap();
    let yaml = to_source(&script).unwrap();
    let again = parse(&yaml).unwrap();
    assert_eq!(again.rules, script.rules);
    assert_eq!(to_source(&again).unwrap(), yaml);
    let expanded = script.expand_rules().unwrap();
    let ids: Vec<_> = expanded.scenes[1]
        .composition
        .clip_instances
        .iter()
        .map(|i| i.id.as_str())
        .collect();
    assert_eq!(ids, ["hand", "r_smile_1"]);
    // Authored data in the unexpanded script is untouched.
    assert_eq!(script.scenes[1].composition.clip_instances.len(), 1);
    // Expansion output is a plain rule-free screenplay that round-trips.
    let flat = to_source(&expanded).unwrap();
    assert!(!flat.contains("rules"));
    assert_eq!(
        parse(&flat).unwrap().scenes[1].composition,
        expanded.scenes[1].composition
    );
}
