# Editable artwork and project version 2

`.kilsat` version 2 stores a required `art: {layers: [...]}` object alongside
exact MotionScript source and the asset inventory. Version 1 is migrated explicitly
on load to empty artwork; source bytes and asset references are preserved. Saves
always write version 2. Unknown fields, future versions, a missing v2 art field,
and artwork smuggled into a v1 file fail rather than silently losing edits.

Each layer has a stable globally unique `id`, a stable `scene_id`, editable name,
visibility, opacity (0–1), optional actor identifier `attachment`, optional
rectangular `clip`, and typed content. Vector order is drawing order, back to
front. Content is an image referencing an image inventory asset, freehand stroke,
filled rectangle, or filled ellipse. RGBA colors use integer bytes. Geometry uses
finite scene coordinates; rectangles have positive dimensions. Attached artwork
and its clip use actor-local coordinates translated by the actor position; this
version does not bind to individual bones or inherit rotation/scale. Clipping is
a rectangle, not an arbitrary painted mask. Raster contents are linked resources,
not editable pixels.

`motion_art::Command` provides atomic add, replace, remove and reorder operations.
`ProjectDocument::apply_art` also checks all scene, character and image references
and the encoded document budget. Deleting an image asset in use is rejected.
Source changes that orphan artwork must be resolved explicitly before saving;
validation never silently deletes a layer. Limits: 1024 layers, 65536 points per
stroke, 262144 total stroke points, absolute coordinates at most one million,
stroke width 0.1–4096, and the existing four MiB project-document limit.

`motion_art::load_image` accepts an already resolved local path (use
`motion_project::resolve_asset` for project assets). It checks regular-file status,
reads at most 32 MiB encoded bytes, verifies PNG/JPEG from its header rather than
extension, and checks dimensions before full decode. Maximum side is 8192 and
maximum area is 16 megapixels. Strict decoder dimension limits and a best-effort
128 MiB decoder allocation limit complement the explicit pixel bound; this is not
a process-wide allocator cap. Decoded RGBA and re-encoded canonical PNG come from
the same pixels, so native textures and self-contained export data URIs share the
same image interpretation. No network resource or SVG image script is loaded.

PNG alpha is preserved. JPEG is decoded to RGBA. Unsupported, oversized, truncated
or unavailable resources return actionable errors; callers must display those
errors instead of treating inventory presence as successful image rendering.
