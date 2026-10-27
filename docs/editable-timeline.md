# Editable scene timing document

`motion_scene::editable_timeline` provides a versioned authored scene-order and
duration document. Explicit compilation returns both the edited MotionScript and
its existing `SequenceTimeline`; callers must keep these together because scene
contribution indices refer to the edited scene order. The source screenplay
continues to contain the actual events, characters, animation and composition.

```json
{
  "version": 1,
  "scenes": [
    {"scene_id": "intro", "duration": 2.0},
    {"scene_id": "reply", "duration": 3.0}
  ]
}
```

The source must define exactly these scene IDs. An edit document must include
every source scene exactly once, with 1–1024 entries. Unknown versions, unknown
fields, duplicate or missing IDs, nonfinite durations and invalid durations are
rejected. Deserialization alone does not approve a document: `compile(source)`
is the validation boundary.

## Commands and timing

`EditableTimeline::from_script(source)` starts from a valid screenplay.
`apply(source, TimelineEdit::SetDuration { scene_id, duration })` changes one
scene duration. `TimelineEdit::MoveScene { scene_id, before }` moves a scene before
another stable ID; `before: None` moves it to the end. Both commands compile a
candidate before committing the document, leaving the original document and
caller-owned screenplay unchanged on failure.

Compilation validates the original and edited screenplay and calls the existing
`SequenceTimeline` compiler. It uses the same 24 kHz sample rounding, sequence
bounds, transition validation and deterministic random-seek weights as other
sequence consumers. It does not create an additional playback clock.

Changing a duration preserves scene-local events, camera keys, animation keys,
clip windows and transition settings. Their local times are not scaled or
trimmed. For example, reducing a two-second scene with a camera key at 1.5 seconds
to one second fails validation. Moving a scene whose outgoing crossfade would
become a final-scene crossfade also fails. These failures require an explicit
change to the offending authored content rather than silently altering it.

## Desktop use

Select a scene and open **Add / duplicate / delete**. **Move scene earlier** and
**Move scene later** use the validated document API and preserve the selected
scene by stable ID. Existing source history supplies Undo/Redo; the resulting
screenplay is saved through normal project persistence. The usual preparation,
diagnostics and export guards apply. Source serialization normalizes formatting
and comments, as with the other structural editors.

The scene-duration command is currently a library API; the new desktop buttons
expose scene reordering. There is no independent timeline-document import/export
UI. The JSON example above documents the serializable API format.

## Validation and remaining scope

New tests cover the JSON round trip, stable IDs after reordering, exact sample
boundaries, atomic invalid edits, unknown versions and references, crossfade
rules, deterministic overlap seeking and preservation of authored camera tracks.
Workspace and desktop integration checks passed Linux/macOS CI in PR #43.
Exact evidence is recorded in [master progress](master-progress.md).

This is a bounded scene-order/duration editing step. General editing of compiled
event tracks, event retiming, a versioned compiled-track interchange format,
separate audio commands and conditional animation rules remain open.

## TrackSpec, overlap order and time conversion (A04)

Contract owner: `motion-scene` (ORK-G1-02). The authored form stays in
`motion-script` (A02); resolved tracks are evaluated state and are not serialized.

### `scene_at_time`

`ValidatedScript::evaluate(scene_index, t)` is `scene_at_time`. It is a pure
function of the validated script, the scene index and `t`: there is no playback
history, cache of earlier frames or hidden clock. `evaluate_sample(scene_index,
local_sample)` is the same function addressed on the 24 kHz clock. The one-shot
`motion_scene::evaluate` prepares the same data and returns identical results.

### Targets: Abs, Rel, Revert

`motion_scene::tracks` resolves authored spans for one `(entity, property)` pair
into concrete `{from, to}` intervals once, when the script is prepared:

| Target | `to` |
| --- | --- |
| `Abs(v)` | `v` |
| `Rel(d)` | `from + d`, where `from` is the track value at the span start |
| `Revert` | the track's base value (not the value before the previous span) |

`from` is always the track value at the span start, so `Rel` gives the cumulative
`base + Σ prior deltas` sum without per-frame accumulation. Playback only finds the
active span (binary search) and interpolates linearly. A finished span holds
exactly `to` until the next span starts.

### Mapping of current authored data

| Authored data | Property | Target | Overlap rule |
| --- | --- | --- | --- |
| `Event::Move` | actor root position | `Abs`, starts where the previous move ended; holds | overlapping moves for one actor are a validation error; touching moves hand over the exact endpoint |
| `actor_tracks` | root translation offset | `Rel` on top of the move position (keyframed) | one track per actor |
| `actor_tracks` | rotation, scale | `Abs` (keyframed) | one track per actor |
| `Event::Emotion` | emotion | `Abs` step; holds | latest start ≤ t wins; equal times: later written wins |
| `background_keys` | background colour | `Abs` step; holds | strictly increasing times (validated) |
| `camera` keys | camera pose | `Abs` keyframes; holds after the last key | strictly increasing times (validated) |
| `Event::Gesture` | gesture weight | envelope that reverts to 0 at its end | strongest envelope wins; ties keep the earlier event |
| clip instances | joint offsets | `Override` ≈ `Abs`, `Additive` ≈ `Rel`; revert outside their window | composed in row order; normalized groups by stable ID |
| bone / joint tracks | joint offsets | `Abs` (keyframed) | one track per joint |

Only `Event::Move` currently goes through `ResolvedTrack`. MotionScript has no
authored `Rel` or `Revert` move yet; adding one is a schema request to A02. The
other rows keep their existing evaluators; the table records their semantics so
consumers can rely on them.

### General overlap order

For resolved tracks: spans are ordered by start time (`f64::total_cmp`), then by
written order. The span with the latest start at or before `t` is active, so a
later start interrupts an earlier span and takes its value at that instant as
`from`. On equal starts the later-written span wins and the earlier one has no
effect. Input order of the span slice does not change the result.

### Time conversion

`motion_scene::time` names the rules already used by `SequenceTimeline` and
`motion-export`, on the single `SEQUENCE_SAMPLE_RATE` = 24 000 Hz clock:

| Conversion | Rule |
| --- | --- |
| seconds → sample | nearest sample, halves away from zero; negative or non-finite is `InvalidTime` |
| sample → seconds | `sample / 24000` |
| frame → sample | `floor(frame × 24000 / fps)` in integer arithmetic (export's rule) |
| frame → seconds | `frame_to_sample / 24000`; bit-identical to `frame / fps` when fps divides 24000 |
| sample → frame | the last frame whose first sample is not after it |
| scene offsets | scene start = sum of whole-sample durations minus crossfade overlaps; local sample = global − start |

Scene durations and crossfades round to whole samples, so sequence seeks never
accumulate floating-point drift. A scene's local time range is `[0, duration]`;
the endpoint is valid for evaluation, but `frame_count` exports only frames
strictly before it, and in a sequence the boundary sample belongs to the next
scene at local sample 0. Negative, non-finite or past-the-end times are
`Error::InvalidTime`; `-0.0` evaluates bit-identically to `0.0`.

ORK-G1-04 places the canonical clock in `motion-audio` (A07). When it lands,
`motion_scene::time` delegates to it rather than keeping a second definition.

### Determinism levels (ADR-A01-04)

1. **State** — tested here: the same script and time give a bit-identical
   `EvaluatedScene` (compared via `Debug`, which distinguishes every relevant f64,
   including `-0.0`). Direct seeks, backwards scrubbing, fresh evaluators and
   frame-by-frame playback agree for every frame of a two-scene sequence.
2. **Pixels** — not covered by A04; needs renderer, font and platform evidence (A08/A10).
3. **Compressed video** — not covered; also requires codec and settings control (A10).

Tests: `crates/motion-scene/tests/timeline_determinism.rs`, unit tests in
`src/time.rs` and `src/tracks.rs`.
