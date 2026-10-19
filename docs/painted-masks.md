# Vector mask painting and erasing

Select an artwork layer in the Inspector, explicitly choose a painted mask, and
choose its initial coverage. A visible base reveals the layer until erased; a
hidden base reveals only painted areas. Select the mask brush or eraser and draw
on the scene canvas. Radius is measured in scene units. Each completed gesture is
one shared Undo/Redo transaction. Changing editing context cancels the draft.

The project stores ordered vector strokes, without creating PNG assets:

```json
{"type":"painted","base_visible":false,"strokes":[
  {"points":[{"x":40.0,"y":50.0}],"radius":12.0,"erase":false},
  {"points":[{"x":40.0,"y":50.0}],"radius":4.0,"erase":true}
]}
```

The shared SVG renderer emits an explicit luminance mask: white reveals and
black hides. Later strokes replace coverage from earlier strokes; painting can
restore an erased area. Single points produce round dabs; connected strokes have
round caps and joins. The base covers the layer's content bounds, in local
coordinates, before actor attachment and camera transforms. Layer/group opacity
and blending apply to the resulting masked content.

The same mask is used by CPU scene preview and SVG, PNG and video export. Native
painting also works over the background compositor preview.

## Soft edges and sampled pressure

New strokes optionally store `hardness` (0–1, default 1) and `pressures` (one
0–1 radius multiplier per point; an empty array means constant full pressure).
Old projects omit these fields and retain their hard, fixed-radius appearance.
Zero pressure contributes no coverage. Pressure varies linearly between samples;
it changes radius, not layer opacity. Hardness 1 has a hard edge; lower values
retain a solid inner radius and fade toward the outer radius. Soft coverage uses
32 deterministic nested bands. Coverage within one gesture takes the maximum,
so repeated points and overlapping segments do not progressively darken the mask.
Separate gestures still compose in their recorded order.

The Inspector exposes hardness and optional input pressure. Pressure is captured
only when the event backend supplies a force value for the initiating touch
contact. Mouse input and missing force use full pressure. No speed-derived or
fabricated pressure is used. The current egui/winit path does not establish macOS
tablet/stylus support: that requires an actual device/backend test. Saved pressure
samples render independently of the current input device.

The experimental GPU compositor supports painted-mask commands using the same
coverage-band geometry and ordered paint/erase calculation. It remains an explicit
experiment; production preview/export use CPU. Command, geometry and fragment-work
limits can reject complex masks or high resolutions. This does not establish full
GPU/native pixel parity, Metal performance or support for image/feathered polygon
masks. Animated paint, raster retouch and physical Mac input acceptance remain open.

Validation bounds stroke count, individual and aggregate point counts, finite
coordinates and radius. No model download or mask asset IO is required.
