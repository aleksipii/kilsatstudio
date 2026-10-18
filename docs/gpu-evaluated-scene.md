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
mean this is not CPU/SVG/native parity. Artwork, image resources, masks, blend
modes and sequence transitions are unsupported. Do not feed a scene stripped of
these features into the primitive-only API and describe that as full rendering.
The script-only probe rejects non-cut transitions and loads no project artwork.

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
