# Next screenplay and timeline package

Planning owner: `screenplay_timeline_owner`; integration owner: parent agent.
Inspected against PR #43's merged implementation on 2026-10-18. This document
specifies future work; it closes no implementation or acceptance item.

## Current boundary

PR #43 completed a bounded, versioned scene-order/duration document:
`motion-scene/src/editable_timeline.rs` owns `EditableTimeline` version 1,
`SceneTiming`, `MoveScene` and `SetDuration`. Compilation validates the original
and candidate screenplay through `SequenceTimeline::compile`, returning the
reordered `Script` together with its timeline. Contribution indices belong to
that returned script. The document requires every stable scene ID exactly once,
limits scenes to 1024, preserves local content and rejects invalid trims or
transitions atomically. It does not retime events. Desktop lifecycle controls
expose earlier/later ordering; duration editing remains a library API. See
[editable timing](../editable-timeline.md) and [progress](../master-progress.md).

Actual remaining screenplay scope is separate audio cue commands, reusable and
conditional rules, cinematic camera commands/presets beyond existing camera
keys, and general versioned editable compiled event/animation tracks. None is
implemented by the scene-timing snapshot or by existing live bone clip layers.

`motion-script/src/lib.rs` currently defines only actor-bound `Move`, `Emotion`,
`Gesture` and `Speak` events. `Event::actor_time` returns a mandatory actor;
validation, scene evaluation, desktop event editing/factory/dragging and timeline
rows depend on that contract. `Speak.audio` imports dialogue and still produces
a dialogue job; it is not an actor-free sound effect command. `dialogue_jobs`
assigns deterministic global IDs in scene/event order, not persistent event IDs.

`motion-voice/src/scene_audio.rs` prepares dialogue off the UI thread;
`export::render_scene` creates PCM plus dialogue/viseme manifest on the 24 kHz mono
clock. It rejects dialogue past the scene endpoint. `motion-audio::MixClip`
already represents sample placement, trim, optional duration and gain;
`mix_timeline` has no implicit resampling and truncates to available/output
frames. Therefore cue preparation must validate exact bounds before invoking it.
`motion_voice::sequence::mix_sequence` applies the shared scene-span transition
weights and currently accepts manifest schema 1 only. Desktop scene and sequence
workers consume these prepared results; preparation must remain on bounded
workers, with cancellation and generation checks.

## Priority: actor-free imported WAV cues

Implement one reviewable vertical package: named, scene-local, one-shot audio
cues with explicit trim, duration and constant gain, audibly identical in scene
preview, sequence preview, WAV and MP4 export. No speech synthesis, lip alignment,
looping, ducking, streaming, implicit rate conversion or global music track is
part of this package. Imported files are user-provided; no asset download.

Proposed additive schema, to agree before production edits:

```yaml
scenes:
  - id: intro
    duration: 4
    events: []
    audio_cues:
      - type: audio
        id: door_close
        audio: audio/door.wav
        at: 0.5
        trim_start: 0.0
        duration: 0.75
        gain: 0.8
```

Add a default-empty `Scene.audio_cues: Vec<AudioCue>` with strict unknown-field
rejection and stable IDs unique within the scene. Use a tagged `type: audio`
command struct; keep it distinct from the actor-event collection. This avoids
inventing a dummy actor or changing the existing public `actor_time` contract.
It is an explicit additive design proposal, not an existing parser capability.
Atomic upsert/remove helpers take stable scene/cue IDs and serialize through
`motion_script::to_source`, after validating the complete candidate. Duplicating
a scene may retain its local cue IDs; deleting a character must preserve cues.
Scene deletion removes that scene's cues only. Missing cue IDs are errors;
replacing an existing cue is explicit. General event identity remains future work.

Schema checks: nonempty bounded IDs/paths, unique cue IDs, finite nonnegative
`at`/`trim_start`, finite positive duration, and finite gain in 0..4. Start with
256 cues per scene and 4096 per project; document these as proposed resource
limits, validate before decoding and tune only with evidence. Check authored
`at + duration <= scene.duration` and overflow. The preparation boundary must
also check rounded sample windows, actual decoded length and format; merely
passing YAML validation cannot approve a missing or short WAV.

Reuse the current project-root-relative audio path convention, but explicitly
validate cue references against absolute paths, parent traversal and symlink
escapes before decoding. Keep that change scoped to cues unless the parent
assigns a coordinated imported-dialogue migration. Store references in source;
use existing versioned `.kilsat` source persistence. Confirm project asset-kind
and root-resolution contracts before deciding whether inventory registration is
required; do not silently reinterpret an inventory ID as a filesystem path.

## Shared preparation and interchange contract

1. `motion-script` owns schema, numeric/reference diagnostics and atomic source
   edits. `Script::validate` must include cue duration checks, so existing
   `EditableTimeline::compile` rejects duration trims that cut cues.
2. `motion-audio` owns PCM and `MixClip`; keep it independent of screenplay,
   speech, desktop and project crates. Add bounded WAV header/size preflight and
   cancellable decoding/mixing here if existing APIs cannot satisfy budgets.
   Current `read_wav` decodes the whole file, so a post-decode limit is inadequate.
3. `motion-voice` extends its existing scene orchestration with prepared cue
   records containing scene/cue identity, resolved source identity and `MixClip`.
   Require 24 kHz mono PCM16 for the first package, matching imported dialogue;
   reject mismatches with an actionable message. No fake `SpeechResult`,
   `DialogueJob`, actor, alignment or viseme is created for a cue.
4. Convert authored seconds once using the established nearest-sample 24 kHz
   rule, including valid zero start/trim and positive rounded duration. Share
   conversion with existing placement where appropriate; do not add a clock.
   Require checked `trim + duration <= source_frames` and
   `start + duration <= scene_frames`; use half-open sample intervals. Mix cue
   and dialogue clips together through the existing mixer; preserve finite
   headroom, clip only at existing WAV encoding, and retain same-actor dialogue
   overlap checks while allowing cue overlaps.
5. Extend the rendered scene manifest explicitly. Preferred compatibility
   contract: retain schema 1 and add a default-empty `audio_cues` metadata field;
   old manifests deserialize as empty. Update every in-repo manifest validator
   and constructor together, including `motion-export` and sequence mixing.
   Parent must approve this additive contract before implementation; if strict
   external consumers require a version bump, coordinate all readers instead.
   Cue metadata records stable ID, start/duration/trim samples and gain, separately
   from dialogue provenance. Do not claim decoder/path provenance proves rights.
6. Both scene and sequence workers consume the same prepared scene PCM.
   Sequence placement and crossfade use `SequenceTimeline` and existing
   `mix_sequence`; cues inherit scene transition weights. No global-time cue
   path or independent sequence mixer. Preserve current total sequence/source
   sample budgets; add decoded cue frame/byte and aggregate-work budgets before
   allocation, checked arithmetic and chunk cancellation. Failed preparation
   publishes no partial result; export preflight fails before output creation.

## Native editing and source history

Add a detached cue inspector and actor-free cue lane, preferably new desktop
modules rather than extending every actor-only event branch. Expose add,
select, duplicate with a new local ID, edit, delete and explicit Apply. Keep
invalid drafts visible with diagnostics. A cue row can show its bounded prepared
waveform; decoding or waveform preparation never runs inside the UI callback.

Parent owns routing in `main.rs`, shared selection and source application.
Apply checks current source generation, scene/cue identity and worker busy
state; accepted edits call the existing source-history path once and invalidate
prepared scene/sequence snapshots. Rejected/noop/stale edits create no history
entry. Source Undo/Redo and `.kilsat` reopen restore cues; history must preserve
unrelated asset inventory and artwork. Maintain scene selection by stable ID
across PR #43 reorders. Dialogue selection/IDs must remain unchanged when only
cues are edited. Document existing structural serialization normalization.

## Ownership, order and verification

| Owner | Files/contracts | Dependency and acceptance |
| --- | --- | --- |
| Screenplay owner | `motion-script/src/lib.rs`, focused schema/edit tests and fixture | Agree schema, cue IDs and budgets first; old source unchanged semantically |
| Audio owner | `motion-audio`, `motion-voice` scene preparation/export/sequence | Depends on cue contract; bounded preparation and exact sample mix |
| Export owner | `motion-export` manifest validation and output fixtures | Depends on manifest contract; same prepared PCM for all consumers |
| Desktop owner | New cue editor/lane module; scene/sequence worker integration | Depends on preparation API; parent reserves `main.rs` and shared history routing |
| Parent integrator | Shared-file integration, API review, docs and progress | Assign nonoverlapping edits before implementation; completion only after checks |

First freeze schema, sample rules, manifest compatibility and resource budgets.
Then land schema plus preparation together with headless regressions; integrate
the native editor only against that accepted API. Do not mark the audio command
feature complete before all production consumers and history paths work.

Required tests include strict unknown fields/types, malformed IDs/paths,
nonfinite values, count/byte/frame budgets, duplicate IDs, unknown references,
atomic edit failure and no-op edits. Use tiny generated WAV fixtures, no model
or asset downloads. Test missing/corrupt/short/wrong-format WAV, trim at EOF,
subsample rounding to zero, end exactly at scene boundary and one sample past,
overflow and cancellation before allocation/during decode/mix. Verify bounded
read behavior with an oversized header rather than allocating huge fixtures.

Use independently computed impulse/constant PCM expectations for cue + dialogue
overlap/gain/trim, preserved headroom and final WAV clipping; cue-only projects
must not call a speech provider. Confirm cue addition leaves mouth manifests
and dialogue IDs unchanged. Test exact sequence cut/crossfade samples, scene
reorder, rejected duration trim, repeated/backwards seeks, pause/resume/seek,
and identical prepared PCM across preview/WAV/MP4. Exercise source history,
`.kilsat` round trip, invalid draft preservation, stale/busy edits and stable
selection after scene reorder. Actual encoder tests use the existing available
encoder harness; do not substitute mocks for encoded media evidence.

Run repository-required fmt/check/Clippy/workspace tests and relevant native and
media fixtures. If Rust, encoders or physical macOS audio devices are unavailable,
report exact blocked checks. Linux numeric tests cannot close physical Mac
playback/synchronization acceptance. This planning change itself runs no Rust or
hardware checks and requires only Markdown/link/whitespace review.

## Following packages remain separate

Camera presets are the next low-risk screenplay package after cues: deterministic
one-shot expansion into existing `Composition.camera: Vec<CameraKey>` through
`apply_composition`, explicit replacement confirmation, finite ordered local
keys and current zoom limits (0.05..20). Save expanded keys so preview/export use
the current evaluator; no second camera evaluator or hidden runtime preset state.
Start with bounded pan/zoom templates before reusable project preset libraries.

Reusable/conditional animation rules need a separate typed rule schema,
reference/cycle/expansion budgets and a deterministic compile-time condition
model. Existing procedural clips and authored clip instances are useful targets,
but are not a general conditional rule language. Define supported inputs and
ordering before implementing; do not permit wall-clock, IO or nondeterministic
conditions, or introduce a second YAML parser/evaluator.

General editable compiled tracks require persistent event/key identity,
version/migration rules, source mapping, explicit retiming semantics, conflict
handling and atomic regeneration. PR #43's scene timing document remains bounded
version 1; do not repurpose it as an unversioned bag of track data. All eventual
commands must return a validated source/snapshot pair compiled through the same
scene evaluator and sample clock. These packages do not depend on GPU parity,
real recognizer availability or license-audit completion; those acceptance gates
remain separately owned.
