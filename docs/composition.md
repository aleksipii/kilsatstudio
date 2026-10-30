# Scene composition

MotionScript scenes optionally contain `composition`. Existing files default to the
original dark background, identity camera, visible actors in ID order, and no fade.

```yaml
composition:
  background_color: '#203040'
  background_keys:
    - {at: 1, color: '#304050'}
    - {at: 3, color: '#506070'}
  camera:
    - {at: 0, x: 0, y: 0, zoom: 1}
    - {at: 4, x: 100, y: 30, zoom: 1.5}
  layers:
    EMMA: {z: 1, visible: true}
    ALEX: {z: -1, visible: false}
  fade_in: 0.5
  fade_out: 0.5
```

A key may set `angle` (`front`, `reverse`, `high`, `low`); it is a cut held until the next key, see [camera angles](camera-angles.md). The camera uses scene seconds, linear interpolation and endpoint holds. Positive
camera offsets move the viewport right/down; zoom is centered on the canvas.
For each world point the shared transformation is:
`canvas_center + (world_point - canvas_center - camera_offset) * zoom`.
Camera motion affects character geometry, not the solid background color. Native
preview and SVG/PNG/MP4 export use the same evaluated camera and actor ordering.
Canvas clipping prevents camera motion from drawing into letterboxing.

Keys must be strictly increasing within the closed scene interval, with finite
positions bounded to one million pixels and zoom between 0.05 and 20. There are
at most 4096 keys per scene. Layer entries must reference existing character IDs;
actors draw in ascending z order, with ID order resolving ties. Hidden actors
still retain their screenplay events and dialogue; visibility does not mute audio.

Colors require exactly `#RRGGBB`. Fade durations must be finite, nonnegative, and
their sum cannot exceed scene duration. Fades affect the entire visual canvas over
black; they do not fade audio or crossfade two scenes. No independent clock or
randomness is introduced, so seeking produces the same evaluated result as export.

`motion_script::apply_composition` validates a complete candidate and returns
normalized YAML atomically. Like the other structured editing helpers, it removes
comments, enforces one MiB input/output bounds, and has no file side effects.
Background asset paths remain metadata; this feature renders a solid color and
does not load background images, masks, or external SVG files.

Deleting an unused character removes its layer metadata atomically. Event references
continue to reject deletion; no dialogue or movement events are silently removed.

## Timed background colors

`background_keys` is an optional list of `{at, color}` entries. The base
`background_color` is used before the first key. A key takes effect exactly at
its scene-local time and holds until the next key; colors do not interpolate.
Keys must have finite, strictly increasing times in `[0, scene.duration]`, with
at most 4096 entries. Every color uses `#RRGGBB`. A key at the scene end is valid
for endpoint inspection; an exported frame sequence samples before that endpoint.

The native composition editor edits these rows in its existing detached draft.
Apply validates the entire scene atomically and participates in source Undo/Redo.
Scrubbing and SVG/PNG/MP4 export share the same stateless color evaluation.
Background images and cross-scene transitions remain separate work.

## Depth order and framing checks

The full back-to-front order (background artwork, actors by `z` then ID, attached
props, unattached foreground, lights) and its tie rules are specified in the
[render contract](render-contract.md#depth-order) and reported by
`motion_render::contract::paint_order`. Wide and close-up framing margins are
checked on the evaluated camera with `motion_render::framing::check_framing`
([render contract § Framing](render-contract.md#framing-wide-and-close-shots)).
