# Bounded export workers and measurement

## Native application update observations

The Inspector's **Performance observations** panel offers opt-in measurement of
elapsed application-update wall time. It retains only 256 samples and can copy a
versioned JSON report with mean, nearest-rank p95 and maximum duration. Separate
summaries cover the explicitly tracked speech/export/diagnostics/sequence job
flags; they do not describe every worker. Measurement does not request additional
frames or perform filesystem IO. Clear or disable it between workloads.

The same panel has a second opt-in, **Measure input to next frame** (128 samples,
report key `input_latency`, report `schema_version` 2). For each key, text, mouse
button or wheel event it records the time from the start of the update pass that
first sees the event to (a) the end of that pass and (b) the start of the next pass,
which eframe begins only after painting and swapping the previous frame. It requests
one extra repaint per measured input so the second timestamp exists; pointer motion
is not sampled and inputs with no following pass within 2 s are counted as abandoned.
This is application-side time only: OS/windowing delay before egui receives the event
and display scan-out are excluded, so it is not photon or end-to-end display latency.
Unit tests cover bounds, disabled state, pairing and abandonment with synthetic
timestamps; no real-input measurement on hardware has been taken.

The update-time observations are observations of the update callback, including blocking/descheduling
inside that callback. They are not CPU usage, input-to-display latency, frame
presentation time, audio latency or process memory measurements. Retain build,
hardware and exact workload alongside reports; release-profile Apple Silicon
benchmarks and actual responsiveness acceptance remain open in
[remaining work](remaining-work.md) and [master progress](master-progress.md).

## Process memory budget

`motion_jobs::MemoryBudget` is a lock-free byte accountant; `process_budget()` is the
single 1 GiB budget for the application process. `reserve(label, bytes)` either
returns an RAII `Reservation` (released on drop, shareable through `Arc`) or a
deterministic `BudgetExhausted` error naming the request, the bytes available and
the capacity. It accounts *estimates* of bulk allocations; it is not an allocator, does
not measure RSS and does not cover GPU textures, egui state or unreviewed allocators.
Wired in: decoded `motion_art::ImageResource` (via `reserve_in`; clones share one
reservation) as loaded by the desktop art resource worker, the sequence-export working
set estimate (held for the export), and the per-worker raster frame estimate of PNG/SVG
frame export. The older per-feature limits (128 MiB image set, 256 MiB export working
set, 320 MiB worker estimate, 300 s of cue source audio per scene) still apply in
addition. `reserve_pcm(label, frames, channels)` (`frames * channels * 4` bytes) and
`reserve_rgba(label, w, h)` (`w * h * 4`) compute saturating estimates, so overflow is
a refusal rather than a wrap. Also wired:

- Decoded audio-cue PCM: `motion_voice::cues::prepare_cues` reserves each cue's
  decoded frames (read from the WAV header, before decoding) and the reservation rides
  on the `CueClip` (shared by clones). A refusal names the scene, cue and byte counts.
- Retained sequence PCM: the desktop `Prepared` project holds reservations for every
  scene's prepared audio and the mixed sequence audio until it is dropped.
- Preview frames: `SequencePreviewFrame` (sequence and single-scene previews) carries
  a reservation for its RGBA buffer until the last clone drops. These are the only
  render-side buffers that are retained; the GPU scene cache and the compositor
  snapshot hold scripts/handles, not pixel buffers, and GPU memory is not accounted.

Not wired: PCM of synthesized or imported dialogue clips while a scene is being
prepared (only the cue, finished-scene and mixed buffers are), egui textures and GPU
memory. Tests: unit tests cover exhaustion, release on drop, overflow, saturating
estimates, release when a scope fails or panics, and concurrent reservation; an
integration test (`crates/motion-voice/tests/cues.rs`) covers cue exhaustion with
deterministic wording and no leaked bytes after the error; a preview test checks the
frame reservation equals its RGBA size. Export-path exhaustion has no dedicated test
because the budget there is process-global.

## Dependency scheduling

`motion_jobs::Graph` runs named jobs with declared dependencies on one to four
scoped workers (never a thread per job). `run` first rejects duplicate names, unknown
dependencies and cycles (Kahn; the error lists the jobs on or behind a cycle) before
any job executes. The calling thread dispatches ready jobs in registration order with
at most `workers` in flight (bounded queue; at most 4096 jobs). A job receives its
dependencies' outputs in declaration order. Outcomes are typed: `Done`, `Failed(E)`,
`Panicked`, `Cancelled` (cancellation seen before start, propagated to all
dependants) and `DependencyFailed(name)` (failure/panic upstream). Running jobs cannot
be interrupted.

Shipped use: the desktop project preparation in
`apps/studio-desktop/src/sequence_worker.rs` (`run_stages`) registers `scene:N`
(speech/alignment/cue preparation for scene N, no dependencies), `mix` (depends on every
scene) and `peaks` (depends on `mix`). It replaces the former hand-ordered loop followed
by mix and peaks. The first failing job in registration order supplies the error (so the
message equals the old path's), `Panicked` keeps the existing worker-panic message,
cancellation (checked before each dispatch and inside each scene) yields the same
"cancelled" error, and progress remains a cumulative dialogue count against the same
total. Product configuration uses **one** graph worker (`PREPARE_WORKERS`): each scene
may call the local speech sidecar and concurrent inference has not been reviewed, so
this change adds explicit ordering, typed outcomes and cancellation propagation but no
speed-up. Tests compare the graph path with a retained copy of the linear loop
(rendered scene audio, manifests, mixed audio, peaks, alignment statuses, progress
monotonicity, a failing scene, and pre-set cancellation) with 1 and 3 workers; they use
imported dialogue audio and cues, never real speech synthesis, so concurrent sidecar
behaviour is untested. The `mix` job clones the per-scene audio once (about the size of
the mixed sequence) because `mix_sequence` takes a contiguous slice. Frame export
itself still uses `ordered_map`.

## Export workers

SVG and PNG sequence exports use `motion-jobs::ordered_map`; MP4 inherits the
PNG path before invoking the explicitly configured local FFmpeg. Existing export
APIs and `floor(frame * 24000 / fps)` timing are unchanged.

The scheduler creates a fixed pool of one to four scoped workers for the entire
operation, with bounded input/result channels and batches of at most sixteen.
Export uses a batch equal to the worker count, so there is at most one outstanding
frame per worker. The coordinator writes files, checks the cumulative output byte
budget and reports progress in frame order. Workers neither write files nor
invoke UI callbacks. Mapper and consumer panics become errors; task errors and
cancellation close the channels and join workers before partial-output cleanup.
Cancellation is cooperative: an in-progress raster/PNG encoding must finish
before its worker can exit. Cancellation predicates must not panic.

For raster export, the worker estimate is 16 bytes per pixel plus 1 MiB per frame
(pixmap, PNG work buffers, and retained encoded output). A 320 MiB estimated frame
working-set budget and available CPU count cap concurrency at four. A 16 million
pixel frame uses one worker. This is an allocation estimate, not a measured RSS
limit: scene evaluation, SVG trees, audio and runtime allocations are additional.
The existing 16-million-pixel/frame, billion-pixel/sequence, frame/work and 256 MiB
output limits still apply. SVG reserves an estimated 4 MiB per worker.

## Reproduce a measurement

Run outside the UI thread, using a new output directory each time:

```sh
cargo run --release -p motion-export --example benchmark_export -- png /tmp/kilsat-bench-png
cargo run --release -p motion-export --example benchmark_export -- svg /tmp/kilsat-bench-svg
```

The example emits JSON containing elapsed wall time, frame count, dimensions,
pixel count, OS and architecture. It renders an actual 48-frame, 640×360 sequence
with synthetic silence; it neither loads a speech model nor downloads assets.
Keep compiler profile, machine, dimensions and background workload constant when
comparing measurements. The tests separately compare every PNG/SVG byte against
serial frame evaluation and verify ordered progress, 10,000-job stress,
cancellation, failure, panic handling and the worker cap.

Observed development-container smoke measurement (debug profile, Linux x86_64):
48 PNG frames / 11,059,200 pixels in 2.22692546 seconds. This is a single local
measurement, not a speedup claim or Apple Silicon/production performance result.
No GPU export implementation or GPU acceleration is claimed.

## Apple Silicon release benchmarks (one machine, by hand)

Harness: `crates/motion-export/tests/benchmark.rs`, three `#[ignore]` tests using
`std::time::Instant`, a fixed 1280x720/24 fps two-scene script (two vector characters,
moves/emotion/gesture events) and synthetic silent audio. No speech synthesis, no
downloaded assets, no GUI. Each benchmark does one discarded warm-up run, then 7
measured runs; `KILSAT_BENCH_RUNS` changes the count.

```sh
cargo test --release -p motion-export --test benchmark -- --ignored --nocapture --test-threads=1
```

Environment: Apple M1 (8 cores, 8 GiB), macOS 27.0.1 (build 26A434), aarch64, release
profile, run on 2026-10-20 after the build finished and other processes were idle (no
concurrent cargo/rustc; the 1-minute load average still showed the tail of the preceding
build, 4.7 at start). The run-to-run spread below is under 1.5 percent, so noise looks
small, but this is one session, not a stability study.

| Benchmark (per run) | Runs | Median | Min | Max |
| --- | --- | --- | --- | --- |
| Scene evaluation, 200 `evaluate` calls (2 scenes x 100 times) | 7 | 1.10 ms | 1.10 ms | 1.11 ms |
| SVG scene to PNG at 1280x720, 24 frames (evaluate, SVG, resvg raster, PNG encode, single thread) | 7 | 271.07 ms | 270.63 ms | 271.53 ms |
| Sequence export to PNG, 48 frames at 1280x720 plus `audio.wav` to the local temp directory (`export_png_sequence`, bounded workers) | 7 | 756.45 ms | 755.07 ms | 758.35 ms |

Per item that is about 5.5 microseconds per evaluation, about 11.3 ms per frame for the
single-thread render path and about 15.8 ms per frame for the export. The export does more
per frame than the render benchmark (project composition, file writes, audio WAV, progress,
worker hand-off), so the two rows are not comparable as a parallel speed-up. These
numbers describe this fixture on this machine only: they say nothing about other Macs,
heavier scenes, art-image scenes, GPU rendering, MP4 encoding, speech, or
input-to-display latency, which needs a GUI session and was not measured.
