# Alignment workstream

Status: repository inspection and next implementation plan, 2026-10-16.
This report completes no automatic alignment or perceptual quality acceptance.
No model, voice or language asset was downloaded and no inference was run.

## Implemented foundations

- `motion-lipsync` validates explicit manual/forced phone intervals and evaluates
  visemes. Forced provenance requires tool/version; tokens are not timestamps.
- `import_alignment_json` checks schema, exact caller-supplied audio identity,
  rate/frame count, phone inventory and intervals. English ARPAbet conversion is
  explicit; unsupported labels fail rather than being guessed.
- `AlignmentCache` keys decoded f32 PCM bits, rate, channels, frame count,
  transcript, provider/version, configuration and manual-edit identity. It bounds
  entry sizes, checks integrity/provenance/duration and publishes atomic writes.
- The headless `alignment_cache` example binds imported timings to decoded PCM;
  it does not execute an aligner. Manual corrections require manual provenance
  and a changed edit identity. Existing cache tests cover identity invalidation,
  corruption, concurrent writes, bounds and forced-provenance mismatch.
- `prepare_scene_with_cancellation` imports/synthesizes dialogue, accepts an
  explicit dialogue-ID-to-alignment map, rejects unrelated IDs and same-actor
  overlaps, then mixes clips. No partial prepared scene is returned on failure.
- The desktop scene worker parses that map off the UI thread and supports
  cancellation/progress. Single-line preparation also accepts pasted timing JSON.
- `DialogueClip::prepare` checks mono 24 kHz audio and alignment duration; mouth
  evaluation shares the audio sample clock, start offset and trim behavior.
- `acceptance.py` binds WAV, smoke evidence, candidate, manual reference and human
  review by hashes, recomputes signal measurements, and reports boundary errors.
  It always leaves production acceptance ungranted; report generation is no
  evidence of listening, physical-device synchronization or measured quality.

## Missing integration and limits

- Desktop/scene preparation does not load/store `AlignmentCache` or invoke an
  automatic aligner. Pasted plain `Alignment` objects carry duration/provenance
  but no exact-audio identity; equal duration does not prove correct pairing.
- There is no provider execution contract, managed aligner environment, pinned
  acoustic/dictionary assets, cancellation adapter or validated output converter.
- There is no waveform phone-boundary editor with transactional Undo/Redo and
  exportable provenance. Existing event retiming edits screenplay events only.
- Real-audio annotation, reviewed thresholds, corpus measurements and physical
  Mac playback/animation acceptance remain open.

## Next smallest implementation: exact-audio import and cache in preparation

1. Add an explicit alignment-input envelope carrying transcript, exact PCM
   identity, provider/version, configuration and original import/edit identity.
   Preserve the existing no-alignment path and label legacy paste as unbound.
2. After decoding the actual dialogue WAV, construct `CacheKey` from that audio
   and current transcript. Use the same path for selected-line and scene jobs;
   dialogue index alone must never identify audio or cached timing.
3. Validate a supplied external document through `import_alignment_json`; store
   and reuse it only under the complete matching key. Without explicit input,
   load only when the complete expected identity is known. A miss leaves the
   mouth at rest; a corrupt entry produces a recoverable, visible error.
4. Run filesystem/hash/import work in existing workers. Check cancellation at
   stage boundaries and prevent cancelled/stale requests publishing UI state.
5. Expose imported/manual, bound/unbound and cache-hit status. Human correction
   changes source to manual and edit hash; it must not retain forced provenance.

This slice introduces useful safe reuse without pretending to generate timings.
An automatic provider follows behind a separate opt-in execution interface.

## Provider investigation, not selection

Montreal Forced Aligner is an unverified offline candidate already mentioned in
`docs/audio.md`; no compatibility, license, package or model claim is established
here. Compare potential providers only after checking official current sources,
Mac CPU support, phone inventory, transcript normalization, model/dictionary
revision pinning, asset rights, output conversion and cancellation behavior.
Require explicit consent before any installation, download or real inference.

## Verification and acceptance plan

- Integration fixtures: exact repeat hit; same-length different PCM rejection;
  transcript/provider/config/model/edit change misses; wrong phone inventory,
  duration or provenance rejection; imported and synthesized audio parity.
- Worker fixtures: cancellation, stale completion, corrupt cache, multi-actor
  IDs, save/reload, and identical preview/export sample-clock mouths. Stub
  providers prove control flow only and must remain labeled synthetic/stub.
- Corpus: obtain authorized recordings across both managed English voices,
  short/long lines, pauses, punctuation, numbers, speeds and difficult phones.
  Preserve exact text, WAV/model/voice/config hashes and environment revisions.
- Have reviewers annotate exact recordings independently, resolve disagreements
  and retain uncertainty. Compare identical ordered phone labels; do not silently
  drop unmatched labels or compare timings from different audio.
- Report start/end mean, maximum and p95 absolute boundary error per recording
  and aggregate. Agree targets before judging results; report alignment failures
  and missing cases. Small or uncertain references limit conclusions.
- On a real Mac, listen and inspect mouths during play, pause, seek and restart,
  recording output device and observations separately from numerical metrics.
  Keep automatic alignment, corpus quality and hardware approval unchecked until
  their corresponding evidence is reviewed.
