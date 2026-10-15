# Artwork rendering and export

The native artwork editor and scene export use the persisted `motion-art::Document`.
Artwork is foreground content: visible layers for the selected scene are painted
in document order after procedural characters. This initial implementation does
not interleave art layers with character layers. Hidden actor attachments are
hidden with their actor. Attachments inherit actor translation only; root rotation
and scale apply to procedural character bodies/faces, not attached artwork.

Images support PNG/JPEG local assets. `motion-art` checks encoded sizes and header
dimensions, decodes off the UI thread, and canonicalizes to PNG. SVG contains only
embedded PNG data URIs, never external image references or user-supplied SVG markup.
Rectangle clipping is scoped per layer, including strokes and images. Color alpha,
layer opacity, scene opacity, camera zoom/offset and canvas letterboxing apply in
the same order as the native preview.

`motion_render::render_svg_with_art` accepts the evaluated scene, output dimensions,
viseme mouths, selected scene ID, artwork document and decoded image map. Existing
render APIs preserve their previous output when artwork is absent. Export accepts
`ArtContext` through `export_scene_with_art`, `export_png_scene_with_art`, and
`export_mp4_scene_with_art`; old export APIs remain available. Every format uses
the existing bounded scheduler and `floor(frame * 24000 / fps)` sample clock.
The resvg `raster-images` feature is enabled so embedded images appear in PNG and
MP4 output, rather than only in SVG.

The `motion-export` `render_scene` example accepts either MotionScript YAML or a
`.kilsat` project as its first argument. Project image references resolve relative
to the project file with canonical root-containment checks. Loading and export are
blocking operations and must run off the native UI event loop. MP4 still requires
a caller-selected absolute local FFmpeg path; no encoder/model download occurs.

Export rejects more than 32 MiB of encoded image resources, 16 million decoded
image pixels, four million art layer/point visits across the sequence or one
million keyframe evaluation visits. Existing frame/output/raster budgets also
apply. Individual SVG frames have a 48 MiB artwork budget. Tests export embedded
images, rectangular clips and strokes and inspect the resulting raster pixels.
These checks establish deterministic compositing, not Mac GPU or physical display
validation.
