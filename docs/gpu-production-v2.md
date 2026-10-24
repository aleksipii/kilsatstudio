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

Captions remain a separate explicit extension until bounded glyph contour code
and bundled-font shaping tests are integrated. Actor labels retain their explicit
omission policy. No new artistic/typographic or full-image parity approval is
implied by adapter-free tests.
