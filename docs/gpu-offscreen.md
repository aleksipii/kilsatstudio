# Experimental offscreen GPU fixture

This opt-in `motion-render/gpu` feature is a **fixture backend**, not the
production MotionScript renderer. CPU SVG/resvg preview/export remains the
application path. No Metal device or GPU rendering was exercised during this
implementation; compilation and adapter-free tests are separate evidence.

## Architecture decision

Use locked wgpu **24.0.5** behind a default-disabled feature. A caller must choose
Metal, Vulkan or GL explicitly. The probe additionally rejects CPU/unknown device
types and any adapter whose backend differs from the requested one. Adapter
failure is an error; it never returns a CPU-rendered frame as GPU success.

The original fixture performs actual GPU composition:

1. Clear an offscreen target to an opaque background.
2. Sample an original 1×1 orange texture onto a quad.
3. Rotate/translate a green actor triangle in the vertex shader.
4. Draw a translucent blue foreground rectangle with a smaller scissor clip.
5. Copy the target into a mapped GPU readback buffer and strip padded rows.

The target is `Rgba8Unorm`, linear color (no sRGB transfer), straight-alpha
fragment inputs with standard source-alpha blending, opaque final alpha.
Draw order defines plane order; the source texture/view/sampler live until the
submitted work completes. There are no external textures, fonts, character assets
or uploaded CPU-rasterized final frames.

Width/height are restricted to 64–2048. Readback rows are rounded to wgpu's
256-byte alignment and checked before allocation; the buffer ceiling is 32 MiB.
At maximum dimensions, target + padded readback + returned pixels require about
48 MiB, excluding driver/device overhead. Initialization futures/readback poll
loops share a 10-second deadline and explicit atomic cancellation. Synchronous
native driver calls cannot be forcibly interrupted; this is not a process-level
watchdog. Run the API off the UI thread. Drop releases owned resources; cancelled
submitted GPU commands may finish asynchronously. Validation scopes and bounded
uncaptured-error collection turn reported errors into failures.

## Reproducible checks

```sh
cargo check --locked -p motion-render --features gpu --all-targets
cargo test --locked -p motion-render --features gpu
```

Local Linux validation completed: GPU-feature/all-target compilation passed;
`cargo test -p motion-render --features gpu` passed **14 tests**, with the one
real-adapter test intentionally ignored.

Adapter-free tests check dimensions/padding, pre-adapter cancellation, invalid
readback length, and parse/validate the WGSL with the locked Naga implementation.
No default test opens a GPU adapter.

On an explicitly selected **real Mac**, run:

```sh
cargo run --locked -p motion-render --features gpu --example gpu_probe -- \
  --metal NEW_OUTPUT_DIRECTORY
KILSAT_GPU_BACKEND=metal cargo test --locked -p motion-render --features gpu \
  gpu::tests::real_fixture -- --ignored --exact
```

`--vulkan` and `--gl` are explicit alternate probes; they do not establish Metal
acceptance. The example refuses existing output and writes `fixture.rgba`
(top-left origin, tightly packed RGBA, 256×256) and `report.json` containing
adapter name/backend/device type, format and reference/validation results.
The opt-in ignored test renders twice to exercise repeated creation/readback.
Output files are written only after readback and interior comparisons succeed.
No adapter was run in the current development environment.

CPU reference probes cover background, sampled texture, transformed actor,
clipped foreground and an excluded foreground point. Each expected linear-unorm
channel allows error ≤2. This checks plane order and alpha/scissor interiors;
it is **not** full CPU raster equivalence or an antialiasing-edge comparison.

## Acceptance still open

- [ ] Run on actual Metal hardware; retain commit, macOS, native architecture,
  adapter identity, raw pixels/report and output hashes.
- [ ] Record initialization failure, real device loss, cancellation during an
  in-flight readback and validation-layer evidence on supported hardware.
- [ ] Add decoded PNG output, transparent-edge comparisons, full evaluated scene
  geometry/art textures, advanced masks/blending, rigging and transitions.
- [ ] Integrate shared preview/export backend selection only after scene parity.
- [ ] Measure cold/warm CPU versus GPU time, GPU time where available, peak memory
  and readback separately. This fixture establishes no speedup claim.

API structure was checked against the locally cached official wgpu24.0.5 source:
`Instance::request_adapter`, `Adapter::request_device`, pipeline descriptors,
`TexelCopyBufferLayout`, `Buffer::map_async`, `Device::poll` and error scopes.
The locked source checksum remains in Cargo.lock; no new graphics API version or
system dependency installation is required by the implementation process.
