# Bounded export workers and measurement

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
