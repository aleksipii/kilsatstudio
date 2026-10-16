# Masks and compositing workstream

Status: scoped backlog, source inspected 2026-10-16. No new mask or blend
feature is implemented by this document. No physical GUI test was performed.

## Current implementation

- `crates/motion-art/src/lib.rs`: persisted `Layer.clip: Option<Rect>`, finite
  opacity in 0..=1, visible flag, background/foreground plane, document order
  within each plane, scene-local half-open active interval, actor translation
  attachment. Clip coordinates remain local to the translated layer.
- Content supports bounded PNG/JPEG images, polylines, rectangles and ellipses.
  There are no groups, arbitrary mask paths, feathering, mask references,
  per-layer blend enums, rotation or scale attachments.
- Limits: 1024 layers; coordinates +/-1,000,000; strokes 65,536 points each
  and 262,144 total. Image limits are 32 MiB encoded, 8192 per side and
  16 megapixels decoded (`motion-art/src/resource.rs`).
- `apps/studio-desktop/src/art_editor.rs`: rectangular clip editor, opacity
  slider, camera-projected egui clip rectangle and ordinary painted shapes.
  Image textures and colors enter egui through unmultiplied RGBA constructors.
- `crates/motion-render/src/lib.rs`: matching layer order and local rectangular
  SVG `clipPath`; layer opacity is an SVG group opacity. Image bytes are embedded.
- `crates/motion-export/src/media.rs`: SVG is rasterized through resvg/tiny-skia;
  PNG frames feed MP4 export. These paths are not a shared native pixel compositor.

## Parity gaps to resolve

The same persisted rectangle and layer order reach both renderers, but this does
not establish pixel-identical output. Native scissor edges and SVG antialiasing
can differ. egui per-shape opacity and SVG isolated group opacity can also differ
where a single semi-transparent stroke overlaps itself. Existing whole-scene
fade coverage does not prove every translucent layer matches. Blend modes cannot
be added solely as SVG attributes while native preview silently uses source-over.

## Next bounded increment: ellipse clip

1. Keep legacy `clip: Option<Rect>` readable. Add optional tagged
   `clip_shape: Ellipse { rect: Rect }` to `motion-art::Layer`, default absent;
   reject documents specifying both fields. This avoids changing the JSON shape
   of existing rectangular clips. Reserve polygon/mask-image variants for later.
2. Normalize both fields into a shared evaluated clip enum, in layer-local
   coordinates before actor translation and camera projection. Reuse `Rect`
   validation and the 1024-layer budget; no external resources or recursion.
3. Render SVG ellipse `clipPath`. Native preview needs true shape clipping:
   use a bounded offscreen alpha surface for the layer, or a shared raster layer
   renderer. A rectangular scissor around an ellipse is insufficient.
4. Bound temporary surfaces to canvas/image limits and an explicit total-byte
   budget before allocation; surface work belongs off the UI thread. Cache keys
   include content, clip, camera/output scale and image revision. Cancellation
   and stale-result guards must preserve the previous complete frame.
5. Add an ellipse/rectangle/none selector to the existing clip editor; commit
   through validated artwork edits and retain one-step Undo. Ship only after
   native and raster-export parity tests pass.

## Color and alpha contract before blend modes

Stored `Color` and image RGBA stay straight/unassociated 8-bit sRGB values.
Compositing must declare its working color space explicitly; neither the current
egui/resvg split nor PNG bytes alone establish a shared color-managed contract.
For a new common compositor, use linear-light floating-point premultiplied RGBA,
with explicit sRGB decode/encode at boundaries. Preserve current rendering for
legacy projects until the changed color behavior has a versioned migration.

For source-over, after coverage and layer opacity multiply both source RGB and
alpha: `Co = Cs + Cd * (1 - as)` and `ao = as + ad * (1 - as)`.
For multiply/screen, evaluate the blend function on straight linear RGB only
when alpha is nonzero, then composite using premultiplied terms:
`Co = (1-ad)*Cs + (1-as)*Cd + as*ad*B(cs,cd)`.
Zero-alpha RGB is zero; never divide by zero or premultiply twice. Clip coverage
multiplies alpha and premultiplied RGB, and layer opacity is applied once after
isolated layer rendering. Future groups require explicit isolation semantics.
Implement normal/multiply/screen only after this shared path exists; reject
unsupported serialized modes instead of degrading them silently.

## Required evidence

- Legacy project roundtrip; ellipse roundtrip; unknown tags, dual clips,
  nonfinite/negative/oversized geometry and allocation-budget rejection.
- Atomic invalid edits and Undo restoring exact prior artwork.
- Pixel fixtures inside/outside/edge of clips, camera zoom/translation, actor
  attachment, both planes, active interval boundaries and images with alpha.
- Native offscreen result versus decoded exported PNG at equal resolution;
  declare edge tolerance separately from exact interior colors.
- Transparent red over blue, zero alpha, opacity 0/0.5/1, crossing translucent
  stroke, overlapping layers and numeric multiply/screen reference vectors.
- Deterministic seek order, bounded memory and stale/cancelled frame rejection.
  Physical Mac display inspection remains separate acceptance evidence.
