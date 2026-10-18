# Next performance and architecture workstream

Status: ready for implementation handoff; specification only. Inspected the
`docs/remaining-work-owners` branch, `AGENTS.md`, `docs/master-progress.md` section
8, `docs/jobs-performance.md`, the actual scheduler, export example and native
worker sources. No production changes or measurements were made for this plan.

## Implemented boundary and remaining gaps

| Actual source | Implemented behavior | Remaining boundary |
| --- | --- | --- |
| `crates/motion-jobs/src/lib.rs` | `ordered_map` uses 1–4 scoped workers, input/result sync channels, batches of 1–16, ordered coordinator consumption, panic conversion and joining on failure/cancellation. Existing tests exercise 10,000 jobs and ordered cancellation/error handling. | Fixed frame batches, not a dependency graph or process-wide resource manager. Cancellation is cooperative; a running mapper can delay joining. |
| `crates/motion-export/src/lib.rs` (`frame_jobs`, export coordinator) | CPU availability caps worker count at four. PNG estimate is pixels × 16 + 1 MiB per outstanding frame; SVG estimate is 4 MiB. Estimated raster allowance is 320 MiB; batch equals workers. Coordinator enforces output caps and ordered progress. | Estimates exclude other concurrent lanes and shared scene/audio/runtime allocations. The minimum of one worker is not a hard RSS guarantee. |
| `crates/motion-export/src/sequence.rs` and `preview_scene.rs` | Separate preflight includes audio/art/raster working-set and work limits; sequence has a 256 MiB estimate and output caps. | These local limits do not constitute one global memory budget; preserve their different purposes. |
| `apps/studio-desktop/src/export_worker.rs` | Persistent worker; busy through completion consumption, coalesced progress, audio clone performed off UI thread. | No shared reservation with other workers or measured UI latency. |
| `apps/studio-desktop/src/sequence_worker.rs` | One bounded preparation/frame/export lane, active token retained until completion is polled, generation-tagged results, coalesced progress. | Independent from scene/export/speech lanes; no dependency-DAG executor. |
| `apps/studio-desktop/src/scene_worker.rs`, `asset_worker.rs`, `alignment_worker.rs`, `gpu_check_worker.rs`; `crates/motion-voice/src/lib.rs`; speech preparation in desktop `main.rs` | Persistent background lanes with bounded requests/results and lane-specific validation/cancellation. Scene progress uses bounded best-effort messages. | Capacity-one channels alone do not bound aggregate retained bytes. Audit each lane rather than assuming identical admission/completion semantics. |
| `crates/motion-export/examples/benchmark_export.rs` | Runnable JSON wall-time measurement for a 48-frame 640×360 fixture, SVG/PNG, synthetic silent PCM; no models/assets required. | No configurable worker baseline, repetitions, peak RSS, UI measurement or production Apple Silicon series. Existing documented Linux debug smoke is historical evidence only. |

Open acceptance remains production release benchmarks, real-workload UI
responsiveness, process-wide measured resource budgeting and explicit task
DAG scheduling. Background scene-image replacement and texture-rig loading remain
open in master progress and need their own consumer audit. Do not mark any of
these complete from this document or scheduler unit tests.

## First implementation package: deterministic estimates and evidence harness

Deliver a small scheduler/export package before migrating every desktop lane.
Keep existing public export entry points working. Do not add Rayon/Tokio, a new
crate, graph scheduler, automatic downloads or runtime-dependent heuristics.

1. Add `crates/motion-jobs/src/budget.rs`, exported by the existing `lib.rs`.
   Define `JobBudget { max_workers: usize, max_in_flight: usize,
   estimated_bytes: u64 }`, `JobEstimate { retained_bytes: u64,
   bytes_per_job: u64 }`, `BudgetPlan { workers: usize, batch_size: usize,
   reserved_bytes: u64 }` and typed `BudgetError` variants for invalid bounds,
   overflow and insufficient capacity. Implement pure
   `plan_jobs(budget, estimate, available_workers) -> Result<BudgetPlan, BudgetError>`.
   Reject zero workers/in-flight/available capacity, limits above the current
   scheduler caps (4/16), zero per-job estimates and checked-arithmetic overflow.
   Subtract retained bytes before dividing remaining bytes by per-job bytes;
   workers and batch equal the minimum of configured limits, available workers
   and byte capacity. Reject capacity below one; do not silently overcommit.
   Validate a plan before any expensive allocation or destination creation.
   This API is deterministic for explicit inputs and describes estimated
   admission, not observed resident memory.
2. Refactor `crates/motion-export/src/lib.rs::frame_jobs` to use this planner
   with explicitly supplied CPU availability and unchanged normal-case defaults.
   Keep raster dimensions/preflight checks and output limits. Add an options
   entry point for benchmark worker overrides (1–4), with existing APIs delegating
   to defaults. Reject invalid overrides before creating output. Do not infer
   benchmark settings from hidden environment variables. Audit scene and sequence
   call paths separately: `sequence.rs` currently has its own preflight and must
   retain it; do not claim all export is parallelized by this refactor.
3. Extend `crates/motion-export/examples/benchmark_export.rs` with an explicit
   worker override and schema-versioned JSON containing requested/effective
   workers, batch, estimate inputs, fixture dimensions/fps/frame count, elapsed
   time and completion/cancellation outcome. Retain current two-positional-argument
   invocation. Add a larger procedurally generated fixture only within existing
   frame/pixel/byte limits. Use synthetic audio; declare rendering/preparation
   timing boundaries. Include externally supplied commit/profile/run labels
   rather than guessing build provenance. Keep JSON machine readable on stdout.
4. Add `scripts/benchmark-export.py` using only Python standard library to run
   an already-built release example, unique output directories, one warmup and
   five measured runs for each format/fixture/worker setting; store raw JSON,
   command, exit status and aggregate median/p95 with the stated quantile rule.
   Record failed runs; never discard them to improve a summary. Hash output
   files outside the timed region to compare worker settings. Preserve evidence
   on failure and allow explicit cleanup of only harness-created directories.
   Do not compile/install/download inside the timed harness. Keep RSS as a
   separate OS measurement field with units/source, nullable if unavailable.
5. Extend `docs/jobs-performance.md` with usage and limitations. Tests belong in
   `crates/motion-jobs/src/budget.rs`, existing export tests in `lib.rs`, and
   `scripts/tests/test_benchmark_export.py` for JSON aggregation/failed subprocess
   handling using a fake local executable. No test depends on wall-time speedup.

Acceptance: exact-limit admission succeeds; one byte below a single-job capacity
fails; retained bytes reduce concurrency; zero/overflow/oversized configuration
fails; explicit identical inputs produce identical plans. Existing ordered,
cancellation, panic, serial/parallel byte and large-frame worker tests remain
valid. New export tests compare workers 1/2/4 byte-for-byte and ordered progress;
invalid options leave no output directory. Harness tests cover warmup exclusion,
failed runs, missing RSS and reproducible percentile calculations. Run applicable
Rust formatting/check/clippy/tests from `AGENTS.md` and the Python harness tests
when implementing; none were run or claimed by this specification-only change.

## Subsequent resource and architecture packages

After the first package, add one application-owned shared admission controller in
`motion-jobs`, explicitly injected from desktop `main.rs`. Use nonblocking
`try_reserve` on UI submission, typed Busy/TooLarge errors and RAII reservations;
never wait for memory on the event loop. Include queued input snapshots, running
scratch, retained results and shared immutable prepared data with documented
single ownership accounting. Reservations follow ownership through completion,
poll/discard, cancellation, errors and panics; do not release while large results
remain queued. Test mixed fake lanes and rollback on failed channel submission.
Migrate export/sequence first, then asset/scene/alignment/speech and texture lanes;
list uncovered caches, GPU allocations, sidecar and encoder child processes.
Estimated reservations require measured RSS calibration and headroom and do not
create an OS-enforced process cap. Child process memory needs separate evidence.

Instrument UI in `apps/studio-desktop/src/main.rs::update` with bounded aggregated
samples of update duration, update gaps and input-to-visible-response latency;
worker submission/completion and cancellation-to-joined/completion latency need
separate timestamps. No per-frame unbounded telemetry or synchronous file writes
on the UI thread. Add an opt-in export of accumulated evidence. Headless fake
worker/clock tests verify sample bounds and quantiles; physical interaction is
required for responsiveness acceptance.

Only introduce a DAG when concrete dependencies require it: synthesis/import →
alignment → prepared scene/audio → frame batches → ordered write → encode.
Initially represent this as an explicit orchestration/state-machine design using
existing lanes. A future DAG API must validate acyclicity and missing dependencies,
bound ready/running/completed payloads, define deterministic tie ordering, propagate
failure/cancellation to descendants and reserve resources before dispatch. Test
with fake tasks; preserve shared sample-clock/evaluator ownership and public
provider interfaces. Do not replace working frame batches solely to add a graph.

## Reproducible release and Mac measurement protocol

The existing example can be measured now on a host with an existing Rust toolchain
and dependencies. Build once outside measurement:

```sh
cargo build --release -p motion-export --example benchmark_export
```

Run `target/release/examples/benchmark_export png NEW_DIRECTORY` and the SVG
equivalent, with a fresh directory for every run. Record one warmup and five raw
runs separately. Do not reuse a populated destination. Until the proposed options
exist, the current example measures only its built-in automatic worker policy;
do not claim a serial baseline or larger fixture. With the new harness, compare
1/2/4 requested workers on the same revision, fixture and host, recording effective
workers; available CPU and estimated memory may lower the effective value.

Record commit and dirty state, exact build command, `rustc -Vv`, OS version,
architecture, CPU/chip, logical cores, RAM, power source/mode, storage/output path,
background activity and thermal observations. Use a quiet machine and consistent
power/display settings; report run order and output hashes. Keep elapsed export
time separate from compilation, hashing and RSS tooling overhead. On macOS use
`/usr/bin/time -l` around the release executable for maximum resident set size
(bytes on macOS), preserving raw stderr; Linux `time` units differ and must be
normalized explicitly. RSS is high-water process evidence, not per-stage live
memory and not proof of an enforced budget.

On a physical Apple Silicon Mac, run the same synthetic export series and record
native architecture (avoid an unlabeled Rosetta run). Then exercise the release
desktop with procedural/imported local fixtures: playback and scrubbing during
PNG export, asset import, sequence preparation and alignment, plus cancellation
under load. Use fixed scripted interaction times and record update p50/p95/p99,
maximum gap, input response p95, cancellation latency and process/child RSS.
Record baseline and each overlapping workload separately. Propose acceptance
budgets before results: at 60 Hz target p95 update ≤16.7 ms, p99 ≤33.4 ms and
input response p95 ≤100 ms; report violations and sample counts rather than
retrofitting limits. These are proposed product gates, not achieved measurements.
Collect real licensed speech/model and long-run encoder workloads as separately
consented acceptance; synthetic export does not establish those results.

## Exact blockers and work that does not need hardware

No Mac, GPU, audio device, speech assets, network access or new toolchain install
is needed to implement the pure planner, fake mixed-lane reservation tests,
benchmark argument/schema handling, harness aggregation tests, procedural
fixtures, instrumentation buffering or DAG validation/state-machine tests.
Running Rust checks still requires an already available toolchain/dependencies;
if missing, record the exact command error without installing them in this task.

Production Apple Silicon release measurements and native event-loop/interaction,
Metal, device-audio and thermal acceptance require a physical Mac and cannot be
substituted by Linux tests. Real Kokoro latency/memory requires explicitly
configured approved local model/voice assets; real MP4 measurements require an
explicit local FFmpeg executable. Neither is required for the first package.
No hardware or model blocker prevents its implementation and deterministic tests.
No new benchmark result, speedup, RSS limit or UI responsiveness claim is made.
