# GPU production coverage follow-up

This work extends the explicit evaluated-scene GPU experiment. It does not make
GPU the default production backend or grant physical Metal/performance acceptance.

## Restored core implementation

- Closed polygon strokes use native analytic GPU commands with round joins.
  PR #89 over, behind and actor-wide silhouette order is preserved, including
  texture placement and outline widths in evaluated rig units.
- Animated props sample the same selected frame and half-open layer interval as
  SVG. Only active decoded image resources are uploaded, with bounded residency.
- Ambient tint, vignette, radial glows, animated/flickering lights and image-screen
  glows use native analytic light commands. Radial attachment, colour sampling and
  intensity are sampled by a shared CPU/SVG/GPU helper; this is not a rasterized
  whole-frame upload. The shader uses the SVG's seven piecewise falloff stops.
- Crossfades relocate image, textured/skinned polygon, closed-stroke and feathered
  polygon references into their combined bounded point/pixel storage.
- Existing scene dimensions, command, vertex, image, nesting and fragment-work
  budgets remain active. Dense or Full HD workloads can still fail these guards.

The root integrator owns Rust/Clippy/full-suite checks and hardware execution.
At this first remote checkpoint code has not yet been compiled in the restored
workspace. Shader validation and deterministic command tests will be run before
integration. Existing CPU/GPU tolerance guards are not weakened. New fixture
measurements require actual hardware and must not be reported as measured here.

## Explicit native captions

`scene_commands::append_captions(commands, scene, style, cues)` shapes the same
bundled Anton font with the locked rustybuzz version used by the SVG stack.
Wrapping, baselines, colours, outlines and backing-band placement use the shared
caption plan. Adaptive glyph contours are composed on GPU; holes use a compound
nonzero winding rule. This is CPU shaping/geometry plus GPU pixels, not a full
CPU-rendered caption or frame uploaded to the GPU.

Glyph count, curve subdivisions, contour storage, finite coordinates and
bounding-box-weighted fragment work are bounded. Per-glyph early-outs justify
the weighted contour budget; all previous shape guards remain unchanged. The
append operation is atomic. Caption commands relocate correctly in a crossfade,
so callers append captions to each scene before crossfading complete requests.
Actor labels retain the prior explicit omission policy.

Nine new independent SVG/CPU reference probes cover outlines, ambient tint,
vignette, radial glow, two image frames and animated screen glow. Adapter-free
checks validate reference pixels and command construction; the separate ignored
hardware measurement test records actual differences without claiming newly
accepted tolerance thresholds. Prior measured fixture guards are unchanged.
Additional caption probes cover holes, kerning/shaping and timing boundaries.
These features remain experimental until actual GPU measurements, typography,
scene parity, responsiveness and memory behaviour have been reviewed.

Useful checks, run by the parent integrator:

```sh
cargo test -p motion-render --features gpu --lib
cargo test -p motion-export --features gpu new_gpu_coverage
KILSAT_GPU_BACKEND=metal cargo test -p motion-export --features gpu gpu_parity -- --ignored --nocapture
```

No physical Metal, artistic or five-minute production acceptance is claimed by
this implementation record. The restored source still awaits parent compilation
and suite results at the time of the second remote checkpoint.
