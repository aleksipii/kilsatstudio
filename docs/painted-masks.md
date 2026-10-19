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
painting also works over the background compositor preview. The experimental GPU
renderer explicitly rejects painted masks. Pressure, soft brush edges, animated
paint strokes, raster retouch and physical Mac input acceptance remain open.

Validation bounds stroke count, individual and aggregate point counts, finite
coordinates and radius. No model download or mask asset IO is required.
