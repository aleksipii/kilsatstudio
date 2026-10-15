# Open-feature review and acceptance path

Reviewed 2026-10-16 against the attached master tracker and the current source.
This is a scoped implementation review, not a declaration that the master prompt
or any whole-product milestone is complete. The integrator records executed
checks and exact commit identity in `master-progress.md`.

## Current increment: what the checks must establish

| Work | Concrete acceptance | Scope still open |
| --- | --- | --- |
| Scene composition | Invalid/nonfinite camera keys rejected atomically; seek reproduces camera position; hidden actors excluded; stable z-order; native preview and SVG/PNG use the same camera and canvas opacity | Scene-edge fades do not implement cross-scene transitions, timed background changes or a multi-scene video |
| Asset inventory | Import copies bounded local files without replacing files; failures preserve inventory; relink stays inside the project root; removal preserves physical files; saved `.kilsat` round trip retains inventory | Inventory import is not image decoding, sprite rendering, rig-part editing or an asset thumbnail browser |
| Asset background worker | Busy state prevents conflicting document operations; a completed worker cannot replace a different/newly opened document; save and dirty state include inventory changes | Full asset-history Undo/Redo and physical Mac responsiveness measurement |
| Parallel frame jobs | Bounded workers/results; deterministic index-ordered output; cancellation and map/consumer failures join workers and clean partial export; PNG/SVG equivalence checked | A frame scheduler is not a dependency-DAG scheduler, a process-wide memory allocator or GPU rendering |
| Alignment cache | Keys include exact decoded PCM, rate/channels, transcript, provider/version, configuration and manual edits; corrupt/oversized/identity-mismatched entries rejected; timing stays within audio duration | A library cache alone does not establish automatic alignment, preparation-path integration or perceptual quality |

Run integrated Rust formatting, workspace checking, strict Clippy and all-target
tests after the agents' edits settle. Run service/package tests if their code
changes. A passing PR #18 CI run establishes that exact PR head's checks only;
new changes need their own CI evidence. Linux/macOS hosted compilation does not
establish an interactive Mac or physical output-device test.

## Forced alignment: researched option, no automatic backend implemented

The official Montreal Forced Aligner (MFA) 3.3.4 command reference accepts local
corpus, dictionary and acoustic-model paths and writes phone/word alignment
output. It is a candidate for a separately pinned offline adapter. No MFA
installation, asset download, subprocess run or quality measurement was performed
for this review. Tool, dictionary and acoustic-model licenses must each be
reviewed before distribution; do not infer them from the Kokoro approval.

For an already installed and separately approved local environment, the next
external experiment is:

```sh
mfa align /local/corpus /local/english.dict /local/acoustic-model.zip \
  /local/new-alignment-output --num_jobs 1 --output_format json
```

Use matching WAV/transcript basenames in the corpus and a fresh output directory.
Supply local asset paths rather than names that could resolve through a model
registry. Record executable/package versions, architecture, exact command,
dictionary/model hashes, transcript, WAV hash, output and errors. Repeat with
network access disabled to demonstrate the actual offline path. This command is
an external acceptance experiment, not a supported Studio command or a download
instruction.

An adapter still needs bounded/timeout/cancellable execution, strict output-size
and phone-interval validation, explicit silence and phone-set mapping, provenance,
cache integration and no silent fallback. Existing acceptance reports require
matching phone labels against a manual reference, so independently map the
aligner's phone set and document that mapping before using those metrics.
Keep original tool output as evidence. Compare real recordings across short and
long text, pauses, numbers, voices and speeds; agree on quality targets before
claiming acceptance. A small mean boundary error cannot establish mouth quality
without listening and visual review.

TorchAudio's current official forced-alignment tutorial says its former APIs
were deprecated in 2.8 and removed in 2.9. Do not base a new adapter on an
unversioned copy of that tutorial. Selecting MFA above is a research direction,
not a claim that it has been validated on Apple Silicon.

Official references consulted 2026-10-16:

- [MFA alignment command and formats](https://montreal-forced-aligner.readthedocs.io/en/v3.3.4/user_guide/workflows/alignment.html)
- [TorchAudio forced-alignment API status](https://docs.pytorch.org/audio/master/tutorials/ctc_forced_alignment_api_tutorial.html)

## Mac and real-speech acceptance still requiring observation

1. On an actual Apple Silicon Mac, open the preview app, inspect icon/Finder file
   association, test document open/save and all edit/Undo/Redo/transport workflows.
   Record OS, chip, build commit and concrete failures; an app launch alone is
   insufficient.
2. Provision only approved, hash-pinned Kokoro/config/voice/language assets. Run
   `services/kokoro-runtime/smoke_macos.py` with its explicit local-inference opt-in,
   then repeat offline. Follow [speech acceptance](speech-acceptance.md) to bind
   reports to the resulting real WAV files. Synthetic fixtures cannot close this
   gate.
3. Listen on the physical output device. Exercise pause/restart/seek, offsets,
   trims, simultaneous actors and scene boundaries while watching mouths. Record
   each unperformed check as `not_tested`.
4. Export real Kokoro and imported-dialogue scenes to MP4. Check decoded video
   duration/frame count, start/end synchronization and drift over a long scene;
   listen and inspect mouths independently of structural timestamp checks.
5. Before production release, complete dependency/asset/encoder notices and
   redistribution review, evaluate signing/notarization, and test the distributed
   bundle on a clean Mac. GPU/Metal execution, production performance and UI
   responsiveness need measurements from their actual paths.

Custom sprite rigs, persistent artist keyframe tracks/easing UI, blending,
drawing/masking, reusable rules, audio-cue commands, editable compiled timelines,
GPU rendering and complete artist-facing documentation remain distinct open
work. Mark them complete only when behavior, integration and relevant evidence
exist.
