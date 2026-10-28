# Reusable and conditional animation rules

Status and remaining production work are tracked in [master progress](master-progress.md).
Rules are declarative data in the screenplay. They are not a scripting language:
nothing is evaluated at runtime, and a rule can only emit an existing capability
(a live clip instance or two camera keys) with bounded parameters.

## Format

```yaml
animation_clips:
  wave_head: {duration: 2, bone_tracks: {head: [{at: 0, x: 2}, {at: 2, x: 4}]}}
rules:
  smile_wave:                  # name: 1..40 ASCII letters, digits, _ or -
    on: emotion                # emotion | speak | gesture | move | time | key
    value: happy               # optional; emotion, gesture or key name to match
    when:                      # optional, all conditions must hold
      scenes: [intro]          # scene IDs; empty/absent = every scene
      actor: EMMA              # the event's actor (time triggers: the subject)
      emotion: happy           # subject's emotion at the trigger time
      from: 1.0                # trigger time >= from (scene seconds)
      until: 8.0               # trigger time < until
    then:
      type: clip               # clip | camera
      clip: wave_head          # project animation clip
      actor: EMMA              # optional; default = trigger actor, then when.actor
      offset: 0.5              # seconds after the trigger
      duration: 1.0
      weight: 0.5              # 0..1, plus blend_mode/fade_in/fade_out
  establishing_push:
    on: time                   # fires once per scene at `at`
    at: 0
    when: {scenes: [intro]}
    then: {type: camera, preset: push_in, offset: 0, duration: 3}
```

Camera presets are `push_in`, `pull_out`, `pan_left` and `pan_right`, with the same
geometry as [camera presets](camera-presets.md), shifted to start at
`trigger + offset`.

Emotion state is the latest `emotion` event for the subject at or before the trigger
time (`neutral` if none); for equal times the later authored event wins.
`speak.emotion` labels a line and does not change state.

`key` rules fire on key presses recorded in an [event log](#eventlog-v1). Like
`time` rules they have no event actor: the subject is `when.actor`, else the clip
effect's `actor`, and a clip effect needs one of them. `value` optionally names the
key (1..32 ASCII letters, digits, `_` or `-`). Without an event log they never fire.

## Expansion semantics

`Script::expand_rules()` returns an ordinary screenplay with `rules` cleared and the
outputs written into each scene's composition. It is a pure function of the
screenplay, so preview, seek and export agree. `ValidatedScript::new` and
`motion_scene::evaluate` expand automatically; `parse` and `Script::validate` run the
full expansion and validation, so a screenplay with bad rules does not parse.

- Order: rules by ascending name; firings by trigger time, then authored event
  order. Generated clip instances are appended after authored ones with IDs
  `r_<rule>_<n>` (`n` counts from 1 per rule and scene). Name rules accordingly to
  control override layering.
- Rules fire on authored events, fixed times and logged key presses, never on rule
  output, so there is no recursion or cycle. Rules cannot call other rules.
- Bounds: 64 rules; 128 outputs per scene; 1024 per screenplay. The existing clip
  limits (128 instances per scene, 32 per actor) still apply to the expanded result.
- Outputs must fit the scene: an instance or camera span past the scene end is an
  error, not a clamp.
- The source keeps `rules`; `to_source` and the edit helpers round-trip them
  unexpanded. Authored data is never rewritten by expansion.

## Rule contract (A03)

This is the contract consumers (A04 timeline, A06 released limbs, A07 viseme rules,
A09 key and camera input) may rely on. It records the behavior that existed before
A03; A03 added the `key` trigger and the event log without changing it.

| Topic | Rule |
| --- | --- |
| Order | Rules by ascending name (`BTreeMap`). Within a rule: `time` → one firing at `at`; event triggers → authored events sorted by `at`, ties in authored order (stable sort); `key` → logged presses of the scene in log order, which validation guarantees is sample order. |
| Priority | The rule name is the priority. Outputs are appended in the order above; for overlapping clip instances the later one layers on top, so a later name wins an override. There is no separate priority field (decision A03-D1). |
| Overlap | Clip outputs may overlap each other and authored clips (layering as above, existing per-scene/per-actor clip limits apply). Camera outputs may not overlap each other or contain an authored camera key: `Conflict`. A generated ID equal to an authored ID: `Conflict`. |
| Chaining | None (decision A03-D2). Effects emit clips or camera keys, which are not facts any trigger matches; logged events are input, not outputs. Expanding an expanded script is a no-op (rules are cleared). |
| Bounds | 64 rules; 128 outputs per scene; 1024 per screenplay; event logs ≤ 4096 events. Exceeding any bound is a typed error, never truncation. |
| Purity | `expand_rules` and `expand_rules_with_events` are pure functions of (script, log). Same inputs → same firings, same IDs, same expanded script. No hidden history or clock reads. |

**A03-D1 — no explicit priority field.** Name order already gives a total,
documented and tested order, and generated IDs (`r_<rule>_<n>`) derive from it. A
second ordering key would need its own tie-break back to the name and would let two
fields disagree. Rename a rule (e.g. `10_base`, `20_accent`) to change priority.
Revisit only if a consumer needs priority independent of a stable name.

**A03-D2 — no chained rules.** No current consumer needs an effect to create a new
fact. If one is added later it needs a fixed chain depth and firing budget that fail
with `RuleError::Bound`, plus tests; that is a new, versioned rule feature.

## EventLog v1

`motion_script::EventLog` (`crates/motion-script/src/event_log.rs`, ORK-G1-03) is the
complete record of external input: nothing outside the log influences a replay.
Examples: [`examples/event-log-v1/`](../examples/event-log-v1/).

```json
{
  "version": 1,
  "sample_rate": 24000,
  "events": [
    {"scene": "one", "sample": 96000, "source": "keyboard", "value": {"type": "key", "key": "x"}},
    {"scene": "one", "sample": 132000, "source": "pointer",
     "value": {"type": "drag", "actor": "A", "joint": "head", "x": 1.5, "y": -2.0}},
    {"scene": "one", "sample": 144000, "source": "pointer",
     "value": {"type": "release", "actor": "A", "joint": "head"}},
    {"scene": "two", "sample": 48000, "source": "camera",
     "value": {"type": "camera_pose", "roll": 0.2, "pitch": -0.1}}
  ]
}
```

| Field | Meaning and validation |
| --- | --- |
| `version` | `1`; any other value is `UnsupportedVersion` (reported before field errors). |
| `sample_rate` | Samples per second of the clock `sample` counts. Must be 24 000 (`EVENT_LOG_SAMPLE_RATE`, the canonical dialogue clock `motion_audio::clock`, ORK-G1-04); any other value is `EventLogError::UnsupportedSampleRate`. The field is kept so a log from another clock is rejected rather than reinterpreted. Before KS-002f (2026-10-27) version 1 accepted `1..=192000`; the application does not write event logs yet, and the bundled examples already used 24 000. |
| `scene`, `sample` | Scene ID and integer scene-local sample. Within one scene samples must not decrease (`Order`); scenes may interleave. Against a script, `sample / sample_rate` must lie in `[0, duration)`. |
| `source` | `keyboard`, `pointer`, `camera` or `manual`. Allowed: key ← keyboard; drag ← pointer; camera_pose ← camera; release ← pointer or keyboard; anything ← manual. |
| `value` | `key {key}`; `drag {actor, joint, x, y}` (scene units, finite, \|v\| ≤ 100 000); `camera_pose {roll, yaw?, pitch?}` (radians, \|roll\| ≤ π/2, \|yaw\|, \|pitch\| ≤ π, same limits as the desktop head-pose panel); `release {actor, joint}`. Joints must exist on the actor: rig joints for custom rigs, humanoid bones otherwise. |

Unknown fields are syntax errors. `EventLog::from_json` limits input to 4 MiB and
validates; `to_json` validates and writes pretty JSON (stable for equal logs).
`validate` checks structure; `validate_for(&Script)` also checks references.

`Script::expand_rules_with_events(&log)` validates the log against the script and
returns `RuleExpansion { script, firings }`. `firings` lists every firing whose
conditions held, in expansion order, with scene, rule, trigger time, source
(`Authored(event index)`, `Time` or `Logged(log index)`) and generated clip ID.
Only `key` events trigger rules; drag, release and camera-pose events are input for
the timeline and physics consumers (A04/A06) and A09's recording. Embedding a log in
the project file and recording it from the UI are A09's G3 work.

## Errors

`RuleError` is the typed result of `expand_rules` and converts to `motion_script::Error`:

| Class | Examples | Maps to |
| --- | --- | --- |
| `UnknownReference` | unknown clip, actor or scene ID | `Validation` |
| `Unsupported` | trigger, effect, camera preset, emotion or gesture value not supported | `Unsupported` |
| `Conflict` | overlapping rule camera spans, rule span containing an authored camera key, generated ID equal to an authored instance ID, effect rejected by the expanded screenplay (for example a clip on a custom-rig actor) | `Validation` |
| `Bound` | rule count or output count exceeded | `Validation` |
| `Invalid` | non-finite or non-positive durations, bad names, `at` on a non-time trigger | `Validation` |
| `EventLog` | event log syntax, bounds, order, values or references (`EventLogError` with an `events[i].field` path) | `Validation`; `UnsupportedVersion` → `Unsupported` |

Unknown YAML fields are syntax errors (strict schema).

## Expansion cost (measured)

`cargo run --release -p motion-script --example rule_expansion_cost` expands one
scene and reports the median of repeated full calls (expansion plus validation of the
expanded script; `0 rules` is the validation-only baseline). Measured 27.10.2026 on
Apple M1 (8 cores, 8 GB), macOS 27.0.1 (26A434), rustc 1.99.1, release build, arm64.
Single machine, no load control; treat as orders of magnitude, not a guarantee.

| Case | Rules × events | Before A03 (µs) | After A03 (µs) |
| --- | --- | ---: | ---: |
| `scan` (trigger never matches) | 64 × 128 | 45 | 48 |
| `scan` | 64 × 1024 | 145 | 155 |
| `condition` (matches, emotion condition false) | 8 × 128 | 440 | 27 |
| `condition` | 64 × 128 | 3 476 | 156 |
| `condition` | 64 × 1024 | 213 185 | 1 453 |
| `logged` (key rules, no match) | 64 × 1024 presses | – | 255 |

Cost is linear in rules × events for trigger matching, plus validation of the
expanded script. Before A03 every firing rescanned the scene's events for the emotion
condition (rules × events², 0.2 s at the bound of 64 rules and 1024 events). A03
precomputes each actor's emotion history once per scene and binary-searches it, so the
condition costs O(log events) per firing with identical results (test
`emotion_condition_uses_latest_event_with_stable_ties` and the unchanged
`rules_tests`). No constant-time claim is made; validation runs on every parse and
edit, so very large scenes still cost milliseconds.

## Validation performed and limits

`crates/motion-script/tests/event_log.rs` covers EventLog v1 (JSON round trip,
typed version/syntax/bound/order/reference errors with paths, the bundled examples)
and rule expansion with a log (same script + same log → same firings and expanded
script, also after a JSON replay; no chaining; key-rule validation and bounds).
Unit tests cover deterministic expansion and ordering, condition true/false cases,
time triggers, camera merging and conflicts, every error class, bounds, ID
collisions, YAML round trips, interaction with authored clip instances, and
identical `evaluate`/`ValidatedScript` results when seeking backward and forward.
The desktop editor does not yet show or edit rules or their expanded output; rules
are edited in the YAML source. No GUI, perceptual or hardware validation was done.
Clip rules do not target custom-rig actors; joint-track copy effects and rules that
react to other rules' output are intentionally absent.

## Native editor

The inspector's "Animation rules" section edits all rules as one detached draft
(name, trigger, match value, scene/actor/emotion/time conditions, clip or camera
effect; text fields are length-limited). Apply calls `motion_script::apply_rules`,
which validates the full expansion (references, overlaps, bounds) and records one
source-history transaction, so Undo restores the previous source and errors are the
real `RuleError` messages. A changed source discards the draft; duplicate names are
rejected rather than merged. Tests are headless egui draws and Studio-level
Apply/Undo/stale-source checks; no GUI was exercised by hand.
