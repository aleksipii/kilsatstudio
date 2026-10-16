# Geometric masks and CPU backdrop blending

Implemented scope: persisted ellipse and simple-polygon clips, and normal,
multiply and screen artwork blending in the shared SVG/resvg CPU renderer.
Whole-project preview, PNG frame sequences and MP4 sequences use that renderer.
Legacy rectangular `clip` and ordinary source-over rendering remain readable
and retain their previous representation when the new fields are absent.

## Project data and editing

A layer may contain either legacy `clip: {x, y, width, height}` or optional
`clip_shape: {type: ellipse, rect: {x, y, width, height}}` /
`clip_shape: {type: polygon, points: [{x, y}, ...]}`. Both together are rejected.
The optional `blend` field accepts `normal`, `multiply`, or `screen`; absent
means normal. Unknown fields, shape tags and blend names fail deserialization.

Ellipse rectangles have finite bounded coordinates and positive dimensions.
Polygons have 3–256 vertices, finite coordinates within +/-1,000,000, nonzero
area and no self-intersections, self-touching or overlapping adjacent edges.
Do not repeat the first vertex. Polygon vertices count toward the existing
262,144-point document budget and sequence geometry-work budget. Existing
scene, raster, PCM, image and output-byte limits still apply.

The layer inspector exposes clip type, ellipse bounds and a detached polygon
text draft. Polygon vertices apply only after parsing and validation. Edits use
the existing validated artwork transaction and history. Polygon text is whitespace
separated `x,y` pairs. Blending and masks apply to the isolated layer, in local
coordinates before actor translation/camera projection; clip coverage and layer
opacity are not added separately to each source primitive.

## Color contract and evidence

These new modes use the existing SVG/resvg/tiny-skia sRGB blend behavior and
straight stored RGBA inputs. This increment does **not** change legacy projects
to a new linear-light compositor or claim color-managed native preview parity.
Multiply uses `s*d`; screen uses `1-(1-s)*(1-d)` on normalized sRGB channels.
The same layer group carries blend mode and opacity, retaining backdrop blending
at partial opacity. SVG `mix-blend-mode` is supported by pinned usvg/resvg 0.45.1
and is verified through decoded raster pixels, rather than accepted by markup
inspection alone.

Automated tests cover schema roundtrip and invalid/ambiguous shapes; simple
concave and intersecting polygons; decoded multiply/screen backdrop colors at
opacity 1 and 0.5; ellipse/polygon inside/outside pixels; deterministic repeated
renders; and decoded PNG export matching the shared preview frame. Existing
plane, camera, active-time and fade tests remain applicable to the wrapper.
No physical Mac GUI or display-color inspection was performed.

## Open acceptance and features

- The selected-scene direct egui painter cannot represent arbitrary shape clips
  or backdrop blend modes. Its canvas must be suppressed for these layers with
  an actionable Whole project preview hint; it must not silently show source-over.
- Full native painting parity, physical Mac visual acceptance, linear-light
  compositing, feathered masks, mask images/references, groups and mask animation
  remain open.
