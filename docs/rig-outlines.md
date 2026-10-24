# Rig outlines and round shapes

Custom rigid rigs (`custom-rigid-characters.md`) can draw a flat cut-out style
with bold outlines and round shapes. All fields are optional and omitted from
serialization when absent, so legacy rigs keep their exact YAML/JSON and SVG
bytes (a motion-render test checks both).

## Part fields

```yaml
- {id: head, joint: head, ellipse: {cx: 0, cy: -21, rx: 28, ry: 27},
   color: [240, 196, 160, 255], outline: {width: 1.8, mode: behind}}
- {id: hat, joint: head, points: [[-26, -41], [26, -41], [29, -41], [28, -58],
   [16, -71], [-16, -71], [-28, -58], [-29, -41]], smooth: 4,
   color: [210, 60, 70, 255], outline: {width: 1.8, mode: behind}}
```

- `ellipse: {cx, cy, rx, ry}` replaces `points` (exactly one of the two). It is
  expanded to a fixed 48-vertex polygon (`ELLIPSE_SEGMENTS`), vertex `i` at angle
  `2*pi*i/48`. Not combinable with `skin` (weights are per control point).
- `smooth: n` (0, or 2..=16) rounds every corner of the control polygon: a
  quadratic Bezier from the incoming edge midpoint to the outgoing edge midpoint
  with the vertex as control point, sampled `n` times. This is a closed uniform
  quadratic B-spline: it stays inside the control polygon's convex hull and has
  no overshoot. Because rounding spans edge midpoints, a rectangle becomes very
  round; add collinear points near a corner to keep its radius small (the night
  room phone body does this). Rounding is applied after posing and skinning
  (it is affine-invariant, so for rigid parts it equals rounding the rest shape).
- `outline: {color, width, mode}`: `color` defaults to opaque black; `width` is
  0..=64 in evaluated rig (actor) units, before actor scale and camera zoom
  (joint scale does not change it). Modes:
  - `over` (default): a stroke of `width` centred on the edge, drawn right after
    the part's fill (and texture). Good for facial features on top of a face.
  - `behind`: a stroke drawn just before the part's own fill with twice the
    width, so a band of `width` shows outside. A later part's fill covers it: a
    body outline sits under the head, while the head's own outline still
    separates it from the body.
  - `silhouette`: drawn in an actor-wide pass before any fill, so overlapping
    silhouette parts share one outer outline without seams at joints.
  All strokes use round joins. Fill alpha 0 (a hidden variant) hides the outline.

## Variants

`variants` may set `ellipse` (instead of `points`; not on skinned parts),
`smooth` (overriding the part's, 0 turns it off) and `outline` (replacing the
part's; width 0 removes it). Viseme, expression and blink variants therefore
inherit rounding and outlines unless they override them.

## Validation and bounds

Checked once in `RigDefinition::prepare`: finite ellipse centres and radii
(radius 0.001..=1e6), `smooth` range, outline width range, `points`/`ellipse`
exclusivity, at most 512 vertices per evaluated shape (`MAX_EVALUATED_POINTS`)
and 16384 expanded vertices per rig, and every rounded shape must still be a
simple polygon. Invalid input is `Error::InvalidDefinition`, never a frame-time
repair. The renderer accepts evaluated parts with 3..=512 vertices and rejects
non-finite or out-of-range outline widths (`InvalidActor`).

## Renderers

- SVG/CPU (preview, PNG, MP4): implemented as above.
- Experimental GPU path: ellipse and rounded parts are ordinary polygons and go
  through the existing polygon command (subject to the fragment-work budget).
  Outlines are refused with `UnsupportedFeature("custom rig part outlines are
  rendered only by the SVG path")`; no GPU parity has been measured for outlines.

## Tests

- motion-rig: ellipse expansion through joint transforms, rounding bounds,
  hull containment, cut corners, small turning angles, rigid commutation,
  skinned rounding after deformation, per-variant outline resolution, and
  rejection of invalid shapes/rounding/outlines.
- motion-render: stroke order per mode, round joins, legacy rig bytes/fields,
  GPU refusal of outlines.
- motion-export `rig_outline_tests`: rasterized black outline bands at head and
  body edges, body outline hidden under the later head fill, rounded corners
  (control corners show background), preview frames equal to SVG->PNG frames
  across shuffled seeks.
- night-room reference: black outline pixels beside both heads plus all the
  existing acting/lighting/caption probes.

These establish geometry and determinism, not artistic quality: the redesigned
night-room characters still need human art review.
