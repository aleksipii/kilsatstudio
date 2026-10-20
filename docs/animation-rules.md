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
    on: emotion                # emotion | speak | gesture | move | time
    value: happy               # optional; emotion or gesture name to match
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
time (`neutral` if none). `speak.emotion` labels a line and does not change state.

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
- Rules fire on authored events only, never on rule output, so there is no
  recursion or cycle. Rules cannot call other rules.
- Bounds: 64 rules; 128 outputs per scene; 1024 per screenplay. The existing clip
  limits (128 instances per scene, 32 per actor) still apply to the expanded result.
- Outputs must fit the scene: an instance or camera span past the scene end is an
  error, not a clamp.
- The source keeps `rules`; `to_source` and the edit helpers round-trip them
  unexpanded. Authored data is never rewritten by expansion.

## Errors

`RuleError` is the typed result of `expand_rules` and converts to `motion_script::Error`:

| Class | Examples | Maps to |
| --- | --- | --- |
| `UnknownReference` | unknown clip, actor or scene ID | `Validation` |
| `Unsupported` | trigger, effect, camera preset, emotion or gesture value not supported | `Unsupported` |
| `Conflict` | overlapping rule camera spans, rule span containing an authored camera key, generated ID equal to an authored instance ID, effect rejected by the expanded screenplay (for example a clip on a custom-rig actor) | `Validation` |
| `Bound` | rule count or output count exceeded | `Validation` |
| `Invalid` | non-finite or non-positive durations, bad names, `at` on a non-time trigger | `Validation` |

Unknown YAML fields are syntax errors (strict schema).

## Validation performed and limits

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
