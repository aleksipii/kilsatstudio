# Soft and animated elliptical artwork masks

Integrated in PR #43 after Linux/macOS CI verification. This feature adds
inward feathering and scene-local animation to elliptical artwork masks. It does
not complete the artist-tool workstream or GPU rendering parity.

## Editing a mask

1. Select an artwork layer in **Artwork / layers**.
2. Set **Clip mask** to **Soft / animated ellipse**.
3. Edit X, Y, W and H for the base ellipse. Set **Feather (inward)** between
   zero and one: zero keeps a hard boundary; one fades continuously from the
   center to the outer boundary. The feather is a fraction of the ellipse radius,
   not a pixel blur radius. Content outside the ellipse is hidden.
4. To animate the mask, enter one whitespace-separated row per key, with six
   comma-separated values: `time,x,y,width,height,feather`.
5. Click **Apply mask animation keys**. Invalid drafts show an error and preserve
   the saved mask and shared Undo/Redo history. To remove animation, clear the
   key text and click Apply.

For example, these rows move and reshape an ellipse while softening its edge:

```text
0,10,10,80,80,0
0.5,10,10,80,80,1
1,30,10,60,80,1
```

Times are seconds from the beginning of the owning scene, independent of a layer's
visibility interval. Rectangles and feather interpolate linearly between keys.
The first and last keys hold before and after their times. When keys exist, they
supply the entire mask; the base rectangle and feather are used only when the
key list is empty. Changing the base therefore does not move an animated mask.
Coordinates follow the existing artwork convention: world-local unless the layer
follows an actor's translation. The existing scene camera also affects the mask.

Mask changes use the shared project history and persist in `.kilsat` project
artwork. Detached key text itself is not saved; Apply commits the valid track.
Existing hard ellipses, polygons and rectangular clips retain their previous
behavior. Changing mask type replaces its previous mask definition; use Undo to
restore it.

## Persisted artwork representation

This is an artwork `clip_shape` value inside a project document, not a new
MotionScript screenplay command:

```json
{
  "type": "soft_ellipse",
  "rect": { "x": 10, "y": 10, "width": 80, "height": 80 },
  "feather": 0.25,
  "keys": [
    {
      "time": 0,
      "rect": { "x": 10, "y": 10, "width": 80, "height": 80 },
      "feather": 0
    },
    {
      "time": 1,
      "rect": { "x": 30, "y": 10, "width": 60, "height": 80 },
      "feather": 1
    }
  ]
}
```

Omitting `keys`, or using an empty array, creates a static soft ellipse. Feather
and base rectangle remain required. Unknown fields and mask types are rejected.
A layer cannot use both its legacy rectangular `clip` and `clip_shape`.

## Limits and evaluation

- At most 128 keys per mask and 8192 mask keys across the artwork document.
- Times must be finite, nonnegative, strictly increasing and within the owning
  scene's duration. A key at the exact scene end is permitted.
- Feather must be finite and between zero and one, inclusive.
- Rectangles follow the existing finite coordinate bounds of ±1,000,000, require
  positive width and height, and must have bounded right and bottom edges.
- The native key draft is limited to 24 KiB. The existing layer, geometry,
  decoded-image and export work budgets continue to apply. Export charges bounded
  track lookup and gradient work even for layers whose mask animation is dormant
  at the start of the scene.

The evaluator searches the validated key track without allocating. Feather zero
uses the same hard ellipse clipping geometry as the existing mask. Positive
feather uses an SVG radial alpha gradient whose coverage decreases inside the
ellipse. Shared CPU scene preview and SVG/PNG exports use the same evaluation;
video export consumes that PNG composition. This does not establish accelerated
GPU mask support or physical Mac rendering acceptance.

## Verification status and remaining work

Added tests cover interpolation and held endpoints, document round trips,
transactional rejection, individual/document key budgets, native draft validation
and history behavior. Pixel tests compare independently authored static masks
with animated samples, repeated preview seeks, SVG rasterization and PNG output.
They also check known radial coverage, rejection of keys beyond the scene and
export work rejection before a destination is created. Linux/macOS CI passed these tests; exact source, runs and package evidence are
recorded in [master progress](master-progress.md). This does not establish
physical Mac rendering acceptance.

Still open: arbitrary
animated mask paths, mask painting and erasing, pressure-sensitive brush input,
image retouching, GPU parity and physical Mac acceptance. The elliptical feather
is an inward radial coverage ramp; it is not a general Gaussian blur or painted
alpha mask.

## Feathered polygon masks

A polygon `clip_shape` accepts an optional `feather`: a Gaussian standard deviation
in scene units (0–1000, default 0, omitted from saved files when zero), so older
documents load unchanged. Unlike the ellipse's inward ramp, the blur is centred on
the boundary: the edge has about half coverage, with softening both inside and
outside. Vertex rules are unchanged. The inspector exposes a logarithmic slider.

```json
{ "type": "polygon", "points": [{"x":20,"y":20},{"x":80,"y":20},{"x":80,"y":80},{"x":20,"y":80}], "feather": 4 }
```

CPU preview, SVG and PNG exports render it as an SVG mask holding a blurred polygon
(filter region four standard deviations wide). Polygon feather is not animated and
the experimental GPU path rejects it with an explicit unsupported-feature error.
Export work budgets do not yet charge for blur area beyond the canvas bound.
Tests cover hard-edge equivalence, edge coverage, monotonic left/right-symmetric
ramps, validation bounds, persistence, legacy JSON, and identical preview/PNG/SVG
pixels. They do not establish physical Mac rendering acceptance.
