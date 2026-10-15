# Staged development

Keep each increment independently reviewable. Approval to merge a specific PR
does not imply approval to merge later stages. Preserve both main and
kilsatstudio1.0 without force-pushing or discarding changes when synchronizing.

| Stage | Deliverable | Verification gate |
| --- | --- | --- |
| 1: audio foundation | Provider, CPU sidecar, cache, YAML dialogue, frame mix and manual mouth tracks | Merged PR #2; Linux/macOS CI passed |
| 2: dialogue workbench | Editor Open/Save, waveform/scrub preview, external alignment import, stronger cache/runtime checks | Merged PR #3; Linux/macOS CI passed |
| 3: playback and export | macOS output worker, frame transport, scene WAV + sample-viseme JSON | Merged PR #4; Linux/macOS CI passed; actual device validation remains required |
| 4: scene foundation | MotionScript movement/expressions/wave, deterministic evaluator, original Emma preview and complete Kokoro master prompt | Merged PR #5; Linux/macOS CI passed; actual GUI test remains pending |
| 5: full-scene dialogue | Prepare all lines off UI, mix leading silence/overlaps, source-clock scene playback and all actor mouths | Merged PR #6; Linux/macOS CI passed; real device validation remains pending |
| 6: real Mac acceptance | Offline prerequisite diagnostics, opt-in CPU smoke evidence, device/seek/pause and measured mouth boundaries | Preview launch user-confirmed; real synthesis, audio and alignment still need machine/WAV/listening evidence |
| 7a: SVG frame output | Deterministic procedural frames; movement-only and sample-clock audio/mouth bundle paths | Frame/sample clock, bounded work, cancellation and output safety; record checks for the exact commit |
| 7b: encoded video | CPU PNG frames and explicit local FFmpeg H.264/AAC with the same scene WAV | Implemented; actual encoder checks cover integer/fractional duration. Real-Mac and actual-speech perceptual acceptance remain pending |

Stages 1–5 were reviewed as sequential PRs and merged in dependency order after
approval. Recheck branch heads before advancing the release branch. Later stages
remain subject to their own implementation, validation and merge approval.

## Stage 3 contracts

`motion_audio::transport::AudioTransport` provides deterministic play/pause/seek
and bounded sample advancement without wall-clock time. The macOS adapter obtains
its live clock from source consumption in the output pipeline. That clock does
not certify audio hardware latency or perceptual lip synchronization.

`motion_voice::export::render_scene` generates PCM and a versioned scene manifest.
Dialogue and viseme intervals are expressed in 24 kHz sample frames. Trimming,
offsets and boundaries follow preview evaluation. It rejects scene overrun;
unsupported formats and mismatched alignment durations are errors. The headless
example writes scene-N.wav and scene-N.json. This is not a video encoder.

Local tests include frame pause/seek/end behavior, preview/export consistency
after trim, invalid export intervals, and the Linux unavailable-device path.
macOS CI can compile the device adapter; only a real Mac with output hardware can
validate that the expected sound is heard and remains perceptually synchronized.

## Scene foundation stage

The original product prompt is preserved and expanded in
`docs/master-build-prompt.md`, with Kokoro as the primary offline speech engine
throughout responsibilities, schema, architecture and speech acceptance stages.
`docs/motionscript.md` separates the implemented subset from that target spec.
The movement preview uses original procedural shapes, not imported artwork or
a verified Metal renderer. Kokoro configuration is present in the combined demo;
real model synthesis remains a separately recorded acceptance gate.

## Full-scene dialogue stage

This increment uses one scene-preparation API for both desktop and headless WAV
export. Individual provider requests remain replaceable and cached. The scene
worker is bounded and persistent, and completion/progress/error messages leave
the UI thread free. Scene duration is checked before synthesis; unknown alignment
IDs and overlapping lines for a single actor are errors. Different actors may
overlap in the shared sample-clock mix. Missing timed alignment closes the mouth.
No partial scene is published after an error. Actual model quality, hardware
latency and perceptual synchronization are still separate acceptance gates.

## Current acceptance preparation

Preview packaging and cancellable generation are integrated stages. A user has
opened the preview on a Mac using the normal per-app macOS approval; the machine
identity and audio result have not been recorded. This is launch evidence only.

The integrated diagnostic increment provides an offline setup inspector and background desktop check,
plus optional JSON smoke evidence with asset identity and WAV signal statistics.
Diagnostics never download assets or perform inference. Opt-in real CPU synthesis
still requires a separately installed, approved local environment. Follow
[practical testing](practical-testing.md) to record both English voices, listening,
cache reuse, cancellation and retry. No inference, hardware synchronization or
perceptual alignment acceptance is established by metadata, hashes or stub tests.

## SVG frame-output stage

The next rendered-output increment is a bounded headless SVG sequence, documented
in [frame export](frame-export.md). It uses the existing scene evaluator and
project frame rate with procedural character artwork. The movement-only example
keeps mouths neutral. A separate `motion-export` path joins prepared scene audio
and explicit mouth intervals at `floor(frame × 24000 / fps)` samples, with bounded
work, progress and cancellation. Rendering neither performs Kokoro inference nor
produces phoneme timestamps.

Individual SVG files and their JSON index permit inspection of movement and
frame timing. They do not constitute encoded video, audio muxing, a production
texture rig, verified visual parity with the desktop renderer, or perceptual
lip-sync acceptance. Real Kokoro, physical audio playback and listening evidence
remain Stage 6 gates while encoding and synchronized media acceptance remain
Stage 7b work. No test result is implied by this roadmap.

## Studio workspace increment

The [workspace guide](ui-workspace.md) describes the screenplay/asset area, scene
preview, inspector/settings and actor timeline. Panel reorganization does not
establish hardware playback, texture loading, a complete project asset manager or
a production animation suite. Scene bundles preserve the existing alignment
provenance; external WAV/JSON checks establish structure, not recording identity
or perceptual mouth accuracy. Video encoding remains Stage 7b.

## Inspector editing increment

Following the integrated workspace/frame-output stages, the Inspector edits
detached event drafts and applies only fully validated replacements. Bounded
whole-screenplay undo/redo restores exact source text. Invalid updates preserve
the original document; audio and alignment invalidation prevent stale prepared
results after accepted edits. This is event-property editing, not timeline
retiming, keyframe tooling or a complete project/asset history. See the
[workspace guide](ui-workspace.md) for formatting and history boundaries.


## Timeline retiming increment

The next focused editor stage adds horizontal event bar/marker dragging on the
project frame grid. A provisional ghost leaves the document unchanged until
release; validation then commits one screenplay history action. Escape cancels,
click selects, and blank ruler/track space retains seeking. Active background
work disables dragging. Move/gesture duration stays fixed and constrains the
latest allowed start. This increment does not add duration trimming, actor
reassignment by dragging, event creation/deletion or encoded video.

Retiming uses the existing edit invalidation path, so prepared scene audio must
be regenerated. Speech duration is not inferred during dragging: preparation
obtains the actual length and can reject scene overrun or same-actor overlap.
The [workspace guide](ui-workspace.md) describes the interaction. Validation
results must be recorded for the actual commit; no GUI or Mac acceptance is
implied by this development-stage description.

## Event lifecycle increment

The Inspector creates detached Move/Expression/Gesture/Dialogue drafts, duplicates
selected events at the playhead, and removes selected events after an explicit
in-app confirmation. Whole-document validation precedes every change and history
records a single undoable transaction. Speech insertion/removal clears stale
sequential-ID alignment mappings; generation cache files are preserved. This does
not add new characters/scenes, automatically fit generated speech, change event
durations by dragging, or provide a complete project asset editor.

## Character voice Inspector increment

Character-level Kokoro configuration is editable through detached Inspector
drafts, atomic validation and source history. Changed synthesized voices invalidate
only their actor's pasted timing IDs, including later scenes; imported audio and
other actor mappings are preserved. Voice assets remain separately installed and
unbundled. This increment supplies configuration editing, not measured synthesis,
a new inference backend, automatic voice installation or acoustic emotion control.


### Project and scene settings

In the Project tab, expand **Project / scene settings** to edit the project title,
frame rate and dimensions, or the selected scene ID and duration. **Apply settings**
validates the whole screenplay and records one Undo action; **Reset settings draft**
discards pending settings. Event times remain in seconds and coordinates are not
rescaled. Dimensions apply to every scene. Shortening rejects events or known
prepared dialogue extending past the new end; speech not yet generated is checked
on preparation. Accepted changes invalidate prepared audio and pending editor
drafts while preserving pasted timing maps. Undo restores exact source text
and clears timing maps under the existing history policy. YAML is normalized and
comments removed on Apply. Script dimensions permit 16384 per side, while SVG
export supports 8192; audio export supports one hour.


## Master plan production-tools increment

See [the complete checked/unchecked tracker](master-progress.md) for every
workstream. This increment adds shared procedural rig evaluation, reusable
keyframe interpolation, versioned project source/asset metadata, scene/character
creation and deletion, PNG/MP4 controls, local speech evidence auditing and
macOS icon/document declarations. Implementation is distinct from full product
acceptance: texture rigs, layers/drawing, cameras/transitions, forced alignment,
GPU rendering and real Mac speech/device evidence remain open.


## Composition, asset inventory and bounded jobs

A subsequent increment adds scene camera tracks, actor z-order/visibility, solid
backgrounds and edge fades, editor composition drafts, local inventory import/
relink/remove, parallel ordered frame workers and an exact-audio alignment cache.
See the master tracker for implementation and acceptance boundaries. Scene-edge
fades are not transitions between scenes; inventory is not a texture renderer;
cache storage is not automatic forced alignment.
