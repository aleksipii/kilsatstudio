# Next speech and mouth-quality workstream

## PR #46 delivery and remaining scope

PR #46 adds an opt-in explicitly trusted self-contained local Phonetic package
profile and automatic worker lookup/miss/publication, with exact package/runtime
identity and controlled execution. See [current local enrollment](estimated-auto-reuse.md)
and [master progress](../master-progress.md). This local trust assertion does not
supply externally audited closure, actual package execution, forced alignment or
real-audio quality; those gates below remain open.

Owner: speech/lipsync workstream. Planning handoff, 2026-10-18.
Scope: inspected repository implementation and existing recorded evidence only.
No production code changed, recognizer/model executed, asset downloaded, timing
measured or quality approval granted for this handoff.

## Verified starting point

PR #43 is recorded in `docs/master-progress.md` as merged at
`ebd7287825c10c8dcfdbf7805ac920c03545eedc` after Linux/macOS CI. Its
`crates/motion-lipsync/src/engine_reuse.rs` supplies `RequestIdentity`, private
`VerifiedEnvironment` fields and immutable `EngineIndex::lookup/publish` storage.
Only module-local synthetic fixtures construct eligibility tokens. There is no
production environment inspector/constructor, supported resource profile or
production automatic cache hit. A request digest does not verify execution.

The request key covers exact decoded PCM/rate/channels/frame count, exact text,
recognizer, fixed adapter/converter contract and OS/architecture. The index is
bounded to 16 KiB, validates original executable/version and immutable archive
payload, distinguishes missing entries from corruption, and preserves conflicting
results through no-replace publication. These are storage guarantees, not proof
of a recognizer's actual resource loading or perceptual timing quality.

`estimated_cache.rs` archives complete estimated results under result-content
keys (including cues and provenance); those keys cannot be computed before a
result exists. `alignment_worker.rs::Action::Estimate` always calls
`rhubarb::align`; `Load { key }` explicitly loads and validates PCM/text without
executing the tool. `ResultOrigin` has only `Fresh` and `ExplicitArchiveLoad`.
The panel exposes the automatic-reuse bypass reason and requires separate Apply.
One bounded worker, capacity-one request/update channels, generation/line/path/text
stale checks, cancellation and 100 KB individual/combined editor limits already
exist. Preserve these contracts.

`rhubarb::align` stages WAV/text, executes version and recognition subprocesses,
checks executable before/after, and validates exact prepared PCM on return. It
currently inherits process environment/cwd. Binary hashing does not identify
external models, dictionaries, libraries or loader policy. Bounds are 128 MiB
binary, 32 MiB WAV/five minutes mono 24 kHz, 64 KiB transcript, 4 MiB JSON,
4 KiB/five-second version output and 120-second recognition. Existing file/hash
reads and 4 MiB archive I/O do not provide chunk-level cancellation; checks around
I/O do not guarantee deadlines during stalled filesystem calls.

## Ordered remaining delivery

1. Establish one real, pinned package's complete execution environment and rights;
   implement a fail-closed inspector and controlled execution snapshot. Until
   evidence exists, unknown/configured executables retain explicit Estimate and
   archive Load, with automatic eligibility unavailable.
2. Connect verified inspection, input-index lookup, miss execution and publication
   in the existing worker. A hit must launch zero processes, including version;
   explicit Estimate remains the authorization for a miss execution. Do not
   duplicate PR #43's completed input-index storage foundation.
3. Build an exact-audio mouth-timing editor with sample-position boundaries,
   waveform selection, validation, explicit Apply, transactional Undo/Redo,
   project save/reload and exportable manual-edit provenance. Screenplay event
   retiming is not a mouth-boundary editor. Keep edited estimated shapes separate
   from forced phone intervals; settle a versioned manual-shape envelope before
   implementation rather than retaining misleading engine-generated provenance.
4. Investigate and implement an opt-in forced-aligner provider separately: exact
   installed tool/model/dictionary revisions and licenses, offline Mac CPU
   compatibility, phone inventory and normalization, cancellation, bounded output
   conversion and exact PCM/text binding. No provider is currently selected or
   validated. Rhubarb shapes are estimates, not forced-aligned phonemes, and
   Kokoro tokens do not supply reliable timestamps.
5. Collect authorized real recordings, independent manual annotations and reviewed
   targets; measure the actual candidates and review audible/visible results.
   Complete physical Mac playback/seek/pause/offset/trim and encoded-video checks
   independently. None of implementation, CI or synthetic arithmetic closes these
   quality/device gates.

## Concrete next implementation package

Deliver **controlled Rhubarb preparation and fail-closed eligibility inspection**
first. It can be reviewed with synthetic files without installing or running a
real recognizer. Production eligibility must remain unavailable until the real
package evidence below supports a specific profile. Worker automatic-hit wiring
follows only after that profile is reviewed; it is not justified by a synthetic
manifest or user-entered digest.

| Files | Contract and acceptance |
| --- | --- |
| `crates/motion-lipsync/src/engine_reuse.rs` | Preserve `RequestIdentity` and index format. Add a narrow package inspector returning eligible private token or actionable ineligible reason. Canonical manifest binds profile revision, exact executable/version, resources, dependency/runtime policy, controlled environment and options. Only validated inspector constructs production tokens. |
| `crates/motion-lipsync/src/rhubarb.rs` | Split bounded preparation from execution so a single owned verified snapshot/options drives lookup and miss execution. Keep existing explicit `align` behavior compatible. Define fixed cwd/environment and loader policy; prevent mutable resource/path swaps, revalidate before publication and candidate delivery. A verified hit uses pinned version evidence without `--version`. |
| `crates/motion-lipsync/src/estimated_cache.rs` | Add bounded-chunk cancellation-aware internal operations while retaining existing public explicit load/store behavior. Preserve immutable publication and owned temporary cleanup. Do not claim filesystem-stall deadlines. |
| `apps/studio-desktop/src/alignment_worker.rs` | Subsequent wiring: inspect off UI thread, lookup only with verified token, execute exactly once on miss, publish only after PCM/text and both JSON budgets validate, then check cancellation/staleness. Corrupt index is recoverable visible error; no silent fresh execution or overwrite. Preserve explicit Load. |
| `apps/studio-desktop/src/alignment_panel.rs` | Subsequent wiring: add distinct automatic-reuse origin and original provenance; show ineligibility/errors; retain separate Apply and existing invalidation. No I/O/hashing during painting. |
| Existing module tests and `docs/workstreams/estimated-auto-reuse.md` | Update tests/documentation around actual new contracts, removing already-delivered index work from future scope. Keep synthetic profiles test-only. |

Inspector limits should be explicit and tested: proposed 4,096 regular files,
4 KiB relative paths, 512 MiB hashed bytes, 64 KiB read chunks and 30-second
cooperative budget. Reject traversal, duplicate/unknown manifest fields, symlinks,
special files, incomplete closure and loader injection. These are proposed bounds,
not current guarantees; review them against the actual selected package.

Required tests: unchanged request/profile hit with zero launches; first eligible
miss exactly one recognition run; changed same-length PCM/text/rate/recognizer/
options/resource/binary/library/environment/platform invalidation; mutation and
path-swap rejection; unknown package ineligibility; cancellation during hashing,
lookup, execution, archive/index publication and delivery; corruption and dangling
archive errors; immutable concurrent/conflicting publication; busy queue/stale
completion; unchanged explicit Load/Apply and scene budget behavior. Stub launch
counters prove control flow only. Run workspace fmt/check/strict Clippy/tests and
relevant Python acceptance tests for changed code; no checks were rerun for this
document-only handoff.

## Evidence blockers and honest acceptance

- **Recognizer package:** no inspected pinned real package profile, inventory of
  embedded/external resources for each recognizer, actual dependency closure,
  controlled loader/cwd/environment policy or mutation-safe execution mechanism
  is established. Capture package origin/revision, exact files/hashes/version,
  OS/architecture/runtime and resource-loading evidence before eligibility.
  PocketSphinx and Phonetic must not be assumed to have the same resources.
- **Assets and distribution:** approved Kokoro-82M does not approve every voice,
  language package, dependency or recognizer bundle. The artifact-specific
  dependency/asset license and redistribution audit remains open. Retain exact
  revisions, licenses/notices and distribution terms; a digest is not a rights
  determination. This plan requests no installation or download.
- **Mac evidence:** existing user report covers Kokoro English CPU speech on an
  M1/macOS 27.1 using preview `98d265b`, Homebrew Python 3.12, `af_heart` and
  `am_michael`. It does not establish network-disabled synthesis, remaining in-app
  acceptance, actual Rhubarb execution, physical output synchronization or real
  encoded scene playback. Record actual package/build/device and observations.
- **Corpus:** retain exact WAV/decoded PCM/text, model/voice/config/environment
  identities and actual candidate provenance for both voices, speeds, pauses,
  short/long text, numbers and difficult phones. Independently annotate the exact
  audio, record uncertainty and resolve disagreements. Agree thresholds before
  judging results; report missing cases and failures, not fabricated timings.
- **Metrics and review:** `services/kokoro-runtime/viseme_acceptance.py` measures
  native shape agreement, active agreement/confusion and transition error only
  when collapsed shape sequences match. `acceptance.py` compares matching ordered
  phoneme labels and reports mean/max/p95 boundaries. Do not invent a label
  correspondence. Both reports remain `production_acceptance: not_granted`.
  Unperformed listening/device/animation observations stay `not_tested`, even
  with perfect synthetic metrics.

References: [automatic reuse](estimated-auto-reuse.md), [alignment](alignment.md),
[native controls](../native-alignment.md), [shape evidence](../viseme-quality.md),
[speech evidence](../speech-acceptance.md), [master progress](../master-progress.md).
