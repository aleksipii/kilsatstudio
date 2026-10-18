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
