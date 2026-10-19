# Experimental evaluated-scene GPU renderer

The `gpu` feature now includes `gpu_scene::GpuSceneRenderer` and the
`gpu_scene_probe` example. This is an original GPU analytical-shape renderer that
receives an already evaluated scene, uploads bounded shape commands, composes
pixels in WGSL and reads back the GPU target. It never uploads a CPU-rendered
frame. Production preview and export still select the existing CPU renderer.

Supported geometry is the original procedural skeleton, head, eyes, viseme
mouths and expression strokes, in actor order, with the shared evaluated root
pose, camera, centered canvas clipping/letterboxing and complete-scene opacity
over black. The renderer reuses its device and pipeline and permits one render
at a time. No scene evaluator or second animation clock is introduced.

Font labels are **unsupported by default**. `Labels::Require` rejects any actor
before adapter initialization. An experimental caller must explicitly choose
`Labels::OmitAcknowledged`; the probe additionally requires the command-line
acknowledgement below. Omitted labels and analytical antialiasing/stroke joins
mean this is not CPU/SVG/native parity. The basic `build`/`render_scene` API is
geometry-only. PR #46 adds bounded artwork/images, masks, isolated groups,
backdrop blending and crossfade commands through `build_with_art` and the project/
sequence probe documented below. The probe accepts `.kilsat` files; its legacy
numeric scene selector still rejects transitions, while `sequence` uses the shared
sequence clock. Production GPU backend selection and measured parity remain open.

An explicit hardware experiment, off the desktop event loop:

```sh
KILSAT_IMPLEMENTATION_COMMIT=$(git rev-parse HEAD) cargo run -p motion-render --features gpu --example gpu_scene_probe -- --metal --omit-labels-acknowledged examples/hello-emma.yaml 0 0.5 640 360 new-gpu-scene-output
```

Use an existing valid local MotionScript path in place of `examples/hello-emma.yaml`.
No model, artwork, font or remote asset is fetched. The newly created directory
contains tightly packed top-left `scene.rgba`, the identical evaluated scene as
`reference.svg` (which includes CPU font labels), the original input and a JSON
report identifying backend/device, dimensions, scene/time and limitations. This
increment writes raw pixels rather than a PNG or claimed raster comparison.
Malformed/unsupported input, cancellation and adapter failures create no output
directory. A partially failed file write removes only the directory this probe
just created.

`gpu_scene::compare_decoded_rgba` compares complete equally sized decoded RGBA
buffers, returning per-channel maximum/RMS error, changed-pixel fraction and
separate edge/interior RMS measurements. Reference pixels differing from a direct
neighbour by more than 8 in any channel are classified as edges. These byte-space
diagnostics establish no pass/fail tolerance or perceptual equivalence; retain
color-space, alpha and CPU rasterizer settings with any actual measurement.

Limits are 64–2048 pixels per dimension, at most 4096 shape commands, 256 mouth
entries and 64 Mi command-fragment visits per render. Work is rejected before
geometry expansion or adapter creation. The target, padded readback and returned
pixels need at most about 48 MiB at 2048 square; host/device command buffers add
less than 1 MiB, below the 128 MiB owned-resource target (driver overhead excluded).
Large actor scenes can be rejected at high resolutions even when their memory
fits. No unbounded target cache or per-frame threads are created.

Cancellation and the 10-second deadline cover cooperative initialization/readback
polling. Native driver calls cannot be forcibly interrupted; retain the existing
child-process watchdog for packaged physical-device checks. Adapter-free Naga
shader validation, geometry/transform/viseme/budget tests and decoded-image metric
tests do not prove Metal execution. Actual Metal runs, complete-image edge
acceptance, performance and full scene parity remain open in
[the GPU release gates](workstreams/gpu-release-gates.md).

PR #43 integrated the experiment after Linux/macOS optional-feature compilation,
strict Clippy, adapter-free shader/command/metric tests and default regression
checks passed. Exact evidence is in [master progress](master-progress.md).
No adapter was requested by those tests; real GPU/parity acceptance stays open.

## Artwork, custom parts and complete-frame transitions (next PR)

The optional renderer now has a separate `scene_commands::build_with_art` entry
point. It validates the entire `motion_art::Document` before constructing commands;
missing decoded image resources fail before adapter initialization. It consumes
`Document::composition`, so background/foreground order, active half-open time
ranges, actor-translation attachments, hidden group subtrees and nested isolated
group opacity/blends use the same authored structure as SVG. No image loading or
asset decode occurs in the renderer.

Actual WGSL scene composition now includes:

- Rectangle and ellipse fills and round-cap/round-join polyline coverage. A stroke
  is evaluated as a union before its alpha is applied, avoiding darker segment
  intersections for translucent strokes.
- Original decoded RGBA images, uploaded as bounded source pixels rather than a
  CPU-rendered frame. Bilinear interpolation uses premultiplied source samples to
  avoid transparent-edge color fringes. Source colors and the opaque output use
  byte-space sRGB conventions; no silent conversion to linear-light blending.
- Hard rectangular, elliptical and nonzero-winding polygon clips; the existing
  animated soft-ellipse mask sampler supplies scene-local rectangle and feather.
- Isolated layer and group normal/multiply/screen source-over composition. Mask,
  layer opacity and group opacity each apply once to the completed subtree.
- Custom rigid-part polygon geometry in authored order, transformed by the same
  actor/camera inverse transform as humanoid geometry. Polygon filling uses the
  SVG default nonzero winding rule, including concave geometry.

`scene_commands::crossfade(first, second, second_weight)` accepts already validated
complete scene requests and the contribution weight supplied by the existing
sequence timeline. It relocates resource indices and composites both scenes on
GPU, including each scene's fade and letterboxing. Each scene is quantized to
8-bit channels before weighted composition, matching the existing CPU sequence's
byte-space rounding contract. Equal dimensions and a finite weight in `[0,1]`
are mandatory; nested complete-frame transitions are rejected. This API does not
create a second sequence clock or infer transitions.

Resource limits remain deliberately conservative: 4096 commands, 64 Mi fragment
work units, 65,536 geometry vertices, 4 Mi source image pixels and 16 composition
stack entries (including the root). Polygon/stroke vertices and four-tap image
sampling count toward fragment work. Crossfades include both scenes in every
resource/work bound, even at weights zero or one. A reusable `GpuSceneRenderer`
keeps at most one source pixel set on host/device, identified by exact content;
replacement evicts the previous set before upload. Frame targets/readback are
released per call. These bounds target less than 128 MiB of renderer-owned
resources at 2048 square, excluding caller-owned decoded assets, retained command
requests, driver overhead and the independent CPU reference renderer.

These are implementation capabilities, **not full CPU/GPU parity or production
GPU selection**. Actor labels still require explicit omission acknowledgement:
there is no selected redistribution-reviewed deterministic font/layout resource.
Analytical edge coverage and SVG rasterization differ, and no pixel-error tolerance
or Metal performance acceptance is inferred from adapter-free tests. Production
preview/export keep their existing CPU backend. Physical GPU images, decoded
reference comparisons, full Unicode text resources and shared production backend
acceptance remain open.

### Exercise artwork and crossfades on a real Mac

The existing helper accepts a saved `.kilsat` project as well as a MotionScript
file. The argument count and explicit label omission remain unchanged. For a
project without crossfades, render scene index 0 at scene-local 0.5 seconds:

```sh
cargo run -p motion-render --features gpu --example gpu_scene_probe -- \
  --metal --omit-labels-acknowledged /absolute/path/project.kilsat \
  0 0.5 640 360 /absolute/path/new-evidence-directory
```

Use `sequence` instead of the scene index to sample the existing 24 kHz project
sequence clock, including authored cuts and complete-scene crossfades:

```sh
cargo run -p motion-render --features gpu --example gpu_scene_probe -- \
  --metal --omit-labels-acknowledged /absolute/path/project.kilsat \
  sequence 1.5 640 360 /absolute/path/new-crossfade-evidence-directory
```

The helper reads regular local files only, resolves active artwork image references
through `motion_project::resolve_asset`, and decodes the exact bounded bytes it
retains. It bounds combined retained encoded assets to 64 MiB and decoded source
images to 4 Mi pixels before adapter initialization. All GPU command and fragment
budgets still apply. Requests exceeding those limits fail explicitly.

The new output directory contains raw `scene.rgba`, exact original project/script
input plus SHA-256, screenplay source, exact active image bytes plus hashes, and
`replay.kilsat` with retained image paths. Replay covers the sampled frame only:
unused images, audio and speech/model assets are not copied. A single-scene run
retains `reference.svg`; an overlap retains separate `reference-0.svg` and
`reference-1.svg` with the original sequence weights in `report.json`. Each SVG
contains the same evaluated scene and artwork **with CPU font labels**, so these
files are not a claimed label-matched pixel comparison. Rasterize and compose
references using the documented byte-space contract before measuring differences.
The helper never synthesizes speech or downloads assets; mouths remain at rest.
No physical acceptance or performance result is claimed merely by building it.
