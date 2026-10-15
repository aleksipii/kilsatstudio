# Staged development

Keep each increment independently reviewable. Approval to merge a specific PR
does not imply approval to merge later stages. Preserve both main and
kilsatstudio1.0 without force-pushing or discarding changes when synchronizing.

| Stage | Deliverable | Verification gate |
| --- | --- | --- |
| 1: audio foundation | Provider, CPU sidecar, cache, YAML dialogue, frame mix and manual mouth tracks | Merged PR #2; Linux/macOS CI passed |
| 2: dialogue workbench | Editor Open/Save, waveform/scrub preview, external alignment import, stronger cache/runtime checks | PR #3; Linux/macOS CI passed; awaiting merge approval |
| 3: playback and export | macOS output worker, frame transport, scene WAV + sample-viseme JSON | Local tests and platform CI; actual device validation remains required |
| 4: scene foundation | MotionScript movement/expressions/wave, deterministic evaluator, original Emma preview and complete Kokoro master prompt | Compiler/scene/dialogue tests and platform CI; actual GUI test remains pending |
| 5: full-scene dialogue | Prepare all lines off UI, mix leading silence/overlaps, source-clock scene playback and all actor mouths | Synthetic/imported pipeline tests and platform CI; real device validation remains pending |
| 6: real Mac acceptance | Run local Kokoro CPU smoke with approved preinstalled assets, exercise device/seek/pause and measure mouth boundaries | Record machine/runtime versions, WAV and timing evidence; never infer success from mocks |
| 7: rendered output | Connect character mouth rig and frame renderer to the same sample clock; encode synchronized video | Compare rendered frame boundaries and exported audio; test cancellation and errors |

Stage 3 is built on the Stage 2 branch while its review is pending. Merge stages
in order after approval. Recheck branch heads before advancing the release branch.

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
