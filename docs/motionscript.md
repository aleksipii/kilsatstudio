# MotionScript supported subset

See `examples/hello-emma.yaml` for the combined movement and Kokoro dialogue demo.
The full target specification is `docs/master-build-prompt.md`; this page describes
what this implementation actually accepts.

- `project` is optional. Its default is title `Untitled`, 24 fps, 1920×1080.
  Explicit projects may override defaults; title must be nonempty, fps 1–240 and dimensions 1–16384.
- `characters` maps stable IDs to a nonempty `asset`, optional `default_pose: idle`,
  and optional `voice` (`provider: kokoro`, English language, preset, speed 0.5–2).
  Asset strings identify future artwork; the desktop currently draws its own
  procedural placeholder actors and does not load a rig from those paths.
- `scenes` contain unique IDs, positive finite durations, optional background names,
  and events. Background names are metadata; `composition.background_color` and
  timed `composition.background_keys` control the rendered solid backdrop. See
  [scene composition](composition.md). Non-speech sounds use
  `composition.audio_cues` ([audio cues](audio-cues.md)). Pictures and
  recordings on a rig's phone or tablet use `composition.screens`, with optional
  per-cue `brightness` (0–4, default 1), `glow` (0–1, default 0) and
  `glow_radius` (slot lengths, (0, 4], default 0.75); see
  [screen slots](screen-slots.md).
- Each event starts at a finite nonnegative `at` less than scene duration and names
  an existing `actor`. Unknown fields and commands fail explicitly.

| type | Required additional fields | Behavior |
| --- | --- | --- |
| move | x, y, duration | Linear movement to project-space coordinates, from the last completed pose; positive duration, ending inside the scene |
| emotion | value | neutral, happy, sad, surprised or angry; also eases the body posture over 0.25 s |
| gesture | value, duration | wave, point, nod or shrug; the clip plays over the duration with a smooth in/out envelope. Other names fail as unsupported |
| speak | text | Dialogue job; requires character voice or event audio; optional emotion and audio |

Move intervals for one actor cannot overlap. At time zero actors begin at the
project center horizontally and 70% of height vertically. Stable source order
resolves simultaneous expression changes. Seeking calls the same stateless
`motion_scene::evaluate` as offline pose export; it does not replay stateful steps.
Speech emotion changes the visual expression but remains metadata for Kokoro
synthesis: no acoustic emotion conditioning is implemented.

Imported `audio` is a project-relative nonempty 24 kHz mono PCM16 WAV. Speech text,
preset, language, speed, model/runtime identity and local asset hashes determine
the synthesis cache. Voices need preinstalled, approved local model assets; scene
movement preview works without those assets. Kokoro tokens have no certified
phoneme timestamps; supply manual or separately aligned timing for mouth shapes.

The desktop supports silent scene playback, selected-line previews, and prepared
full-scene dialogue mixes. Source playback position drives movement and timed
mouth tracks when audio is loaded; a silent transport advances quantized sample
time otherwise. Full-scene preparation preserves leading silence and rejects
same-actor overlapping dialogue. Device latency and actual macOS GUI execution have not been validated here.

Offline pose sampling:

```sh
cargo run -p motion-scene --example evaluate_scene -- examples/hello-emma.yaml 0 /absolute/new-poses.json
```

The output is sampled actor poses at project fps, not rendered images or video.
The command refuses to replace an existing output and caps frame/actor counts.

## Captions

`project.captions` (optional) turns speak events into speaker-coloured captions; see [captions](captions.md).

## Versions

A document without a `version` key is a **legacy script** (implicit version 0) and is
read by `motion_script::parse` exactly as described above. Every existing example
remains a legacy script; nothing about that format changed.

A document with `version: 1` is a **schedule document** read by
`motion_script::compiler::compile`. It wraps an unchanged legacy script and adds
structural timing. Other versions are rejected. The envelope schema is
[`crates/motion-script/schema/motionscript-schedule-v1.schema.json`](../crates/motion-script/schema/motionscript-schedule-v1.schema.json);
the inner `script` is defined by the Rust types in `motion-script` and checked by
`Script::validate`. A breaking change requires a new version and a migration.

```yaml
version: 1
script: {characters: {EMMA: {asset: emma}}, scenes: [{id: opening, duration: 8, events: []}]}
schedules:
  opening:                     # scene id → one root node
    op: seq
    children:
      - {op: wait, duration: 1}
      - op: stagger
        delay: 0.5
        children:
          - {op: event, span: 1, event: {type: emotion, actor: EMMA, at: 0, value: happy}}
          - {op: event, span: 1, event: {type: emotion, actor: EMMA, at: 0, value: sad}}
```

### Schedule semantics

`seq`, `par` and `stagger` are compiler sugar: they are lowered to absolute event
times in the existing `Script` before `Script::validate` runs. No new event type,
clock or timeline is introduced. Times are finite seconds from scene start and
intervals are half-open `[start, end)`. With node start `S` and child duration `Dᵢ`:

| Node | Start of child `i` | Node duration |
| --- | --- | --- |
| `seq` | `S + Σ_{j<i} Dⱼ` | `Σ Dᵢ` |
| `par` | `S` | `max Dᵢ` |
| `stagger`, `delay` | `S + i·delay` | `max(i·delay + Dᵢ)` |
| `wait`, `duration` | – (no event) | `duration` |
| `event`, `span`, `event` | event at `S + event.at` | `span` |

- `children` is nonempty; `delay ≥ 0`; `duration` and `span` are positive and finite.
- `event.at` is a local offset in `[0, span)`; a move/gesture `duration` must fit the span.
- Every node must end at or before the scene duration; the scene is never extended.
  Overflow and precision collapse (a positive interval rounding to zero) are rejected.
- `speak` needs an author-supplied `span`. The compiler does not measure audio or
  guess speech length; the span reserves structural time only.
- Output events are stably sorted by start time. Ties keep the `script` section's own
  events first, then schedule events in depth-first source order. This order is part
  of version 1.

### Diagnostics

Compilation is atomic: it returns a complete `Script` or one `Diagnostic` with a typed
`DiagnosticCode`, a YAML path and, for YAML syntax errors, a 1-based line and column.

| Code | Meaning |
| --- | --- |
| `syntax` | Malformed YAML, including duplicate keys |
| `schema` | Missing/unknown field or wrong value shape |
| `version` | `version` is not 1 |
| `reference` | Unknown scene id or actor |
| `time` | Non-finite, negative or zero time; interval outside span or scene; precision loss |
| `unsupported` | Unknown `op` or event `type` (reported, never skipped) |
| `validation` | Rejected by the existing `Script::validate` (e.g. overlapping moves) |
| `limit` | Source > 1 MiB, > 1024 schedule nodes or depth > 32 |

Paths use zero-based indexes, e.g. `$.schedules["opening"].children[1].event.actor`.
Errors raised by the legacy validator inside `script` are reported at `$.script`.

Examples: [`nested.yaml`](../examples/motionscript-v1/nested.yaml) (valid) and the
`invalid-*.yaml` files beside it, each of which states its expected diagnostic in an
`# expect:` header that the test suite checks.

```sh
cargo run -p motion-script --example compile_script < examples/motionscript-v1/nested.yaml
```

### SceneSpec handed to A04

The compiler output is `motion_script::compiler::Compilation`:

- `script: Script` — the same `Script`/`Scene`/`Event` types and scene-relative
  seconds every other consumer already uses, already accepted by `Script::validate`
  and by legacy `parse` when re-serialized.
- `origins: Vec<Vec<String>>` — `origins[scene][event]` is the source path of each
  output event after sorting. It is tied to that source revision, not a persistent ID.

The compiler does not call `expand_rules` (A03 runs rules afterwards and owns any
reordering provenance), does not quantize to frames or samples, and does not resolve
tracks (A04, `motion-scene`).
