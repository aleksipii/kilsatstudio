# Remaining studio work and active agents

Updated 2026-10-19. This is the execution list for the next increment after PR #54.
The authoritative requirements, completion marks and acceptance evidence remain
in [master-progress.md](master-progress.md), based on the
[master build prompt](master-build-prompt.md). Historical unchecked rows are not
automatically current missing features. Integration is owned by the parent agent.

`[x]` means integrated and checked; `[ ]` means still open. Work in the table is
in progress until the parent records the merge and actual CI evidence here and in
the master tracker. Code tests cannot grant physical-device or rights approval.

| Area / agent | Current concrete package | Further work left open |
| --- | --- | --- |
| Characters — `remaining_character` | Versioned bounded original-rig clip clipboard exchange between projects, explicit replacement and shared source history | Custom-rig clip libraries/retargeting/general mixing, textured parts, deformation, idle and physical motion review |
| Artist tools — `remaining_artist` | Scene-local animated image-mask rectangles, native key editing, shared preview/export and seek tests | Polygon and painted-mask animation, retouch/image processing and physical stylus acceptance |
| Speech — `remaining_speech` | Exact-audio sample-boundary mouth-shape editing, explicit manual provenance and Apply through existing history | Real forced-alignment integration, audited recognizer environment, annotated corpus and measured/perceptual quality |
| Screenplay — `remaining_screenplay` | Validated cinematic push/pull/pan camera presets using existing camera tracks, detached native Apply | Separate audio cues, reusable/conditional animation rules and general editable compiled tracks |
| GPU — `remaining_gpu` | Image alpha/luminance masks in the existing bounded GPU experiment, animation sampling, relocation and shader/command tests | Feathered polygon masks, fonts, measured full parity, production GPU preview/export and Metal performance |
| Performance — parent | Opt-in bounded application-update timing window, active/idle background-work summaries and copyable report | Input-to-display latency, process-wide memory budget, dependency-DAG scheduling and release/Apple Silicon benchmarks |
| Mac — `remaining_release_mac` | Build-bound per-scenario evidence completeness in the existing operator report, with truthful untested/fail states | Physical offline Kokoro, devices/sync, Finder, shortcuts, accessibility, pressure and end-to-end workflow |
| Release/licences — `remaining_release_mac` | Preserve exact artifact binding and unresolved review gates while improving evidence coverage | Final upstream component/asset rights review, NOTICE corpus, production packaging and Developer ID/notarization decision |

## Acceptance for this increment

- [ ] Integrate the concrete packages above without duplicating clocks, renderers
  or document histories; preserve old project defaults and unrelated work.
- [ ] Run relevant format/check/strict Clippy/Rust, Python, GPU shader/contracts
  and media CI; record real counts and distinguish ignored hardware tests.
- [ ] Review changed public types, enum consumers, duration/resource preflight,
  history/persistence, cancellation and source-context protection.
- [ ] Update this file and [master-progress.md](master-progress.md) after merge
  with exact source/merge identities, actual checks and still-open scope.
- [ ] Collect external Mac, real-speech, GPU-quality/performance and licensing
  evidence. These gates remain open until supplied and reviewed.

## Existing completed baseline

- [x] Custom rigid definitions and custom-rig joint tracks/editor (PRs #46/#50).
- [x] Original-rig override/additive/normalized clips, weights/fades, loops and
  joint masks (PRs #40–#46).
- [x] Groups, image masks, polygon feathering and vector mask painting/erasing
  (PRs #46/#48/#49/#53).
- [x] Soft mask strokes, actual-event pressure handling and experimental painted
  GPU mask commands (PR #54); physical Mac stylus and measured parity stay open.
- [x] Automatic estimated-cue reuse for explicitly enrolled supported local
  packages (PR #46); environment/quality approval stays open.

M1–M6 remain unaccepted as complete products. See the individual gates in
[master-progress.md](master-progress.md#whole-product-milestone-gates).
