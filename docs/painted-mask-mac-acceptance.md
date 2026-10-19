# Painted mask input acceptance on a physical Mac

Status: **not executed**. This procedure and the original, asset-free
[fixture](../examples/painted-mask-input.kilsat) prepare a reproducible check.
The authoring host is Linux; it cannot exercise Finder, native macOS pointer or
pen input, the graphical editor, or a physical Metal adapter. Automated rendering
and package CI do not supply those observations.

Use a trusted preview whose BUILD-INFO identifies the tested source. Preserve
BUILD-INFO, the exact preview ZIP and its SHA256, macOS/chip/display scale, input
device/driver, reviewer and UTC date. Copy the fixture to a new evidence directory
outside the checkout. No images, speech models, network access or extra assets
are needed. Save experiments to new `.kilsat` files, retaining the original.

## Native input, history and rendering

Open the fixture through the application's Open control, validate, and seek to
1.000 s. Select **Reveal: pressure ramp and soft dab** in Artwork / layers. Its
orange card starts hidden; the authored upper stroke narrows at both ends, and
the lower dab has a soft edge. The green card starts visible, contains a soft
erased hole, then a small restored solid center. The blue strip is attached to
GUIDE in actor-local coordinates. Authored pressure values exercise document
rendering only: their appearance is not evidence of native pen input.

| ID | Action | Expected observation |
| --- | --- | --- |
| M1 | Choose **Mask brush**, disable **Use device pressure for mask radius**, set radius 12 and **Mask hardness (1 = hard edge)** to 1. Click once on the orange card, then draw a separate curved stroke. | One round dab and one continuous round-ended stroke; no polygon seam or unexplained taper. Mouse fallback has full radius. |
| M2 | Undo once, Undo again, then Redo twice using Command-Z / Shift-Command-Z with canvas focus. | Each complete gesture is one chronological transaction. First Undo removes the drag, second removes the dab; authored fixture strokes survive. |
| M3 | Set hardness to 0.25 and radius 30. Paint a dab and a crossing drag. Switch to **Mask eraser**, erase across them, then restore a smaller area with the brush. | Soft edges are visible against the background; crossing a stroke does not create an unintended opaque knot. Later erasure hides and later paint reveals, with a soft partial edge rather than a hard cut. |
| M4 | Start a drag, press Escape, and release. Repeat while changing selection or scene context. | Cancelled drafts add no stroke and no Undo entry. Existing document content remains intact. |
| M5 | Save the modified `.kilsat`, close and reopen it; compare at 1.000 s. | Stroke order, pressure arrays, radius, hardness and coverage survive; saved document has no mask PNG asset. |
| M6 | Select **Actor-local: soft reveal**, add a short brush gesture, then seek 1.000 → 3.000 → 1.000 s. | Mask remains attached to the strip through actor translation and camera zoom/pan. Returning to the same time restores the same appearance; cursor radius tracks scene zoom. |
| M7 | Export SVG and PNG at the same documented scene time, dimensions and frame rate as preview; inspect them in named viewers. Export MP4 only if an explicit local FFmpeg is available. | The mask and ordering agree across CPU preview and export at equal samples. Review antialiased edges separately from interior coverage, and record player/viewer scaling. |

Do not change a previously authored mask type merely to enter painting: that
explicit operation replaces its mask strokes. Use the existing Painted selection
in this fixture. Record any failure and retain both pre-edit and post-edit
projects; a retry on a new build is a new observation.

## Pressure device boundary

The **Use device pressure for mask radius** checkbox opts into pressure captured
from an egui touch event for the initiating contact. Missing force and ordinary
mouse input use pressure 1. The current winit mouse event does not expose tablet
force. A tablet driver may emit only mouse events on macOS; connecting a pen
therefore does not establish pressure capture.

1. Enable the checkbox and draw with an ordinary mouse. Save and inspect the
   new stroke: absent pressure samples or samples of 1 mean full-radius fallback.
2. With a named pressure-capable input device, draw light → firm → light at a
   fixed radius. Retain the project and a screen recording plus device/driver
   identity. Inspect the new stroke's `pressures`: require actual varying samples
   bound to its points before claiming captured pressure. A saved authored ramp
   or a manually edited array cannot pass this test.
3. Repeat with the checkbox disabled: the radius must stay fixed. If force events
   are unavailable, record `not_tested` for native pressure and `pass` or `fail`
   separately for mouse fallback. Never infer pressure support from pen motion.
4. If the device supports simultaneous contacts, introduce a second contact while
   drawing. A foreign contact must not append pressure to the first stroke;
   observe cancellation and confirm existing artwork is preserved. Leave this
   observation `not_tested` for devices without multiple contacts.

## Explicit Metal experiment

The production editor continues to use the shared CPU compositor. The separate
GPU scene probe is an explicit experiment and requires a matching source checkout
and Rust toolchain on a physical Mac. It omits labels only with acknowledgement;
it neither enables a production GPU preview nor certifies full scene parity.
From the checkout root, with an existing evidence-directory parent and a new
absolute output directory, run:

```sh
cargo run --locked -p motion-render --features gpu --example gpu_scene_probe -- \
  --metal --omit-labels-acknowledged examples/painted-mask-input.kilsat \
  0 1.0 640 360 /Users/yourname/Desktop/new-painted-mask-metal
```

Retain the actual command/exit status, `report.json`, `scene.rgba`, exact input,
`replay.kilsat` if emitted, and CPU reference SVG. Record the named Metal adapter
and validation errors. Compare the reference raster and GPU at equal dimensions;
record measured differences and visual findings, not an assumed tolerance. The
report deliberately retains `pixel_parity=not-measured` and
`full_scene_parity=false`; neither field becomes a pass through packaging or a
successful render. An unsupported capability must report an actionable error,
not a silently unmasked frame. Record unavailable Metal/toolchain as
`not_tested`, not as a rendering success.

## Retained observation record

Create a separate record per build/run. Every field below starts unobserved:

| Observation | Status | Actual result / evidence |
| --- | --- | --- |
| M1–M7 individually | not_tested | No native Mac execution in this change |
| Mouse fallback with pressure enabled | not_tested | No native pointer execution |
| Actual varying native pressure capture | not_tested | No physical force-capable event source available |
| Multi-contact cancellation | not_tested | No multi-contact device available |
| Metal painted-mask render and CPU comparison | not_tested | No physical Mac / Metal adapter available |

For each executed row replace the status with `pass` or `fail` and retain exact
steps, expected/actual behavior, project/export filenames and SHA256 values. Keep
unavailable rows `not_tested` with a reason. The existing
[Mac observation collector](release-license-report.md#mac-observation-collector)
has broad release gates, not mask-specific gates: attach this detailed record as
bounded evidence for `full-workflow` only when that whole gate was exercised;
a successful mask test alone does not pass full workflow, audio or accessibility.
A human/authority review remains separate from evidence collection.
