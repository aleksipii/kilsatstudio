# Kilsat Studio agent instructions

These instructions apply throughout this repository. Follow any more specific
AGENTS.md in the directory being changed. Follow the user's current task scope;
do not implement every workstream merely because it appears in this document.

## Product and independence

Kilsat Studio is an original, macOS-first animation workstation built primarily
in Rust. MotionScript is an English YAML screenplay format compiled into editable,
deterministic animation timelines. The application must remain useful offline.

- Author production code independently.
- Storytold may be consulted only for high-level architectural research. Never
  clone, fork, copy, vendor, import or integrate its source code; add dependencies
  on its repositories, crates, APIs or services; modify its repositories; or build
  Storytold adapters or compatibility layers.
- Ordinary open-source libraries are permitted subject to their licenses.
- Kokoro-82M is an approved third-party speech model. Its approval does not relax
  the Storytold restriction or asset licensing obligations.

## Inspect before changing

- Inspect the actual branch, working tree, existing code and project instructions.
  Do not assume proposed crates or features already exist.
- Preserve unrelated work and public interfaces used by other workstreams.
- Prefer the smallest working implementation. Create a crate only when it has a
  concrete responsibility; avoid empty architectural scaffolding.
- Keep dependencies acyclic and shared types owned by the narrowest suitable crate.
- When coordinating parallel agents, establish interfaces, ownership, dependencies
  and acceptance criteria first. Assign non-overlapping files where possible.

## Architecture and engineering

The shared pipeline is:
MotionScript → parsing → validation → compilation → timeline → scene evaluation
→ rendering → preview/export.

- Separate document state, evaluated scene state and desktop UI state.
- Use the same deterministic evaluation and timing rules for preview and export.
- Use stable character/scene identifiers and versioned serializable project data.
- Distinguish syntax errors, invalid references and unsupported commands. Report
  unsupported commands explicitly rather than ignoring them.
- Validate finite times, durations, numeric parameters and asset references at
  boundaries. Prefer typed errors with actionable messages over panics.
- Prefer safe, idiomatic Rust. Document any necessary unsafe code.
- Use bounded job queues and controlled resource allocation. Never synthesize
  speech, load large assets or encode media on the desktop event loop.
- Avoid one thread per character/frame, unnecessary global mutable state and
  premature abstractions. Provide progress and recoverable failures for long jobs.

## Workstream boundaries

Target responsibilities, introduced as needed:

| Area | Ownership |
| --- | --- |
| motion-core / motion-scene / motion-timeline | Shared types, evaluated scenes, deterministic timing |
| motion-script | YAML schema, parser, diagnostics, semantic validation, compiler |
| motion-rig / motion-animation | Character hierarchy, poses, interpolation, expressions |
| motion-render | Scene composition, preview and offscreen rendering |
| studio-desktop | Native egui/eframe editor, inspector, preview, timeline |
| motion-project | Versioned project persistence and asset references |
| motion-audio | Audio assets, waveform data, sample-based placement and mixing |
| motion-tts | Speech provider boundary, synthesis backends and audio cache |
| motion-lipsync | Alignment provenance, timed phonemes and viseme evaluation |
| motion-voice | Character voices and dialogue/timeline orchestration |
| services/kokoro-runtime | Replaceable local inference sidecar |
| motion-export | Frame sequences, media encoding, progress and cancellation |
| motion-jobs | Bounded scheduling, background work and resource budgets |

Coordinate interface changes across affected consumers. Do not create competing
implementations of the same parser, clock or scene evaluator.

## Kokoro, dialogue and lip synchronization

- Primary speech model: Hugging Face hexgrad/Kokoro-82M; English first.
- Preserve a clean Rust SpeechProvider interface with validated SpeechRequest,
  SpeechResult and SpeechError types. A Python sidecar is acceptable; keep native
  backend replacement possible without rewriting the UI or screenplay model.
- Start with local CPU inference on macOS and test Apple Silicon explicitly.
  Investigate ONNX Runtime/Core ML separately. Claim native Rust, Metal, MPS or
  other acceleration only after verifying the actual execution path.
- Download model, voice and language assets only with appropriate user consent.
  Never require a paid cloud API. Keep large model assets outside Git.
- Verify exact model/voice revisions, dependency licenses, redistribution terms
  and required notices before distributing them. Model approval is not a complete
  license audit of every dependency or asset.
- Cache by content: text, preset, model revision, synthesis parameters, language
  and other settings affecting output. Include backend/environment identity and
  invalidate stale or corrupt entries.
- Do not assume Kokoro phonemes have accurate timestamps. Use a distinct alignment
  stage and record whether timings are manual, estimated or forced-aligned.
- Structural timestamp validation is not perceptual quality validation. Measure
  alignment against real audio before describing mouth timing as validated.
- Use one audio sample clock for dialogue placement, mouth evaluation and export.
  Account consistently for offsets, trimming, seeking, pause and scene boundaries.
- Missing inference must produce clear errors and permit explicit imported audio
  or another configured offline fallback. Never fabricate speech or silently use
  a cloud service.
- Do not present emotion/style metadata as acoustic conditioning unless supported
  and tested. Distinguish waveform data from a completed preview UI.

## Validation

For Rust changes, run the applicable checks from the workspace root:

```sh
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets
cargo test --workspace
```

- Add meaningful tests for parsers, semantic validation, deterministic evaluation,
  sample placement, mixing, cache invalidation, timeouts and alignment boundaries.
- Run relevant service tests when changing a sidecar. For the Kokoro runtime, when
  present: `python3 -m unittest discover -s services/kokoro-runtime/tests -v`.
- Keep Cargo.lock committed for the application workspace. Verify dependency
  compatibility rather than copying illustrative version numbers.
- Use macOS CI where practical. Linux tests do not establish macOS graphics,
  audio-device, packaging or Apple Silicon compatibility.
- If tools, assets or a platform are unavailable, report exact blocked checks.
  Never claim tests passed, synthesis ran, alignment quality was measured or a
  graphical application was exercised unless it actually happened.
- Never label an entire milestone complete based only on interfaces or mocks.

## Git and delivery

- Work on a task-specific development branch and submit a reviewable pull request
  against the repository's current default branch (normally main).
- Use `feat/milestone-1-desktop-foundation` for that milestone when available;
  use separate task branches for independent follow-up work.
- Commit logically grouped changes. Do not force-push, overwrite unrelated work,
  delete existing work or merge into main without explicit user approval.
- Check actual write operations; repository metadata alone does not prove the
  integration can create branches or commits.
- If GitHub denies access, retain the source and report the exact blocker. Do not
  claim a remote branch, push, commit or pull request exists without confirmation.
- Report implemented behavior, validation actually executed, remaining limitations
  and the pull request URL when one was successfully created.
