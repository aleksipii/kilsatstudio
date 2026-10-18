# Next artist tools implementation handoff

Status: source inspected 2026-10-18; specification only. The parent integrator
owns scheduling, shared-file allocation and completion evidence. This document
changes no production behavior and closes no product or hardware gate.

## Inspected baseline

`motion-art/src/lib.rs` currently stores a flat `Document.layers` list, with
`Layer` scene, plane, half-open visibility interval, opacity, actor-translation
attachment, legacy rectangle clip, tagged clip shape and normal/multiply/screen
blend. Content is an image, constant-width stroke, rectangle or ellipse.
`Document::apply` clones, validates and atomically swaps Add/Replace/Remove/Reorder.
Project-level scene/actor/image references and mask duration checks live in
`motion-project::ProjectDocument::validate` and `apply_art`.

PR #43 already implemented `ClipShape::SoftEllipse { rect, feather, keys }`.
`sample_ellipse` linearly samples scene-local geometry/feather without allocation,
with held endpoints. Feather is inward normalized radial coverage, not Gaussian
blur. Hard ellipses and validated simple polygons also exist. Limits include
128 mask keys per layer, 8192 document mask keys, 256 polygon vertices, 1024 layers,
262144 total geometry points and finite coordinates bounded to +/-1000000.
The older `workstreams/masks-blending.md` is historical planning; its assertions
that blend enums, polygons and feathering are absent are superseded by this code
and `soft-animated-masks.md`.

`motion-render::append_art` emits each active layer separately in plane/document
order, with actor translation, layer opacity, SVG blend style and local masks.
Soft ellipses emit radial alpha masks; CPU SVG/resvg output feeds PNG/MP4.
`studio-desktop::requires_composited_preview` selects the existing bounded shared
compositor worker for active tagged masks/blends. The direct egui artwork painter
cannot establish arbitrary masking or group isolation parity.

`art_editor::Editor::commit` validates candidates and exposes the prior document
through `take_transaction_before`; mask text is a detached validated Apply draft.
Gestures collect at most 2048 points and create a fixed width 4 stroke. There is
no pressure sample model, mask brush, group hierarchy or retouch command.
`project_history::Snapshot` copies source, asset references and artwork metadata,
never decoded images. History is 64 entries / 8 MiB; navigation validates before
moving stacks. Its image-reference check currently visits only `Content::Image`.

Remaining distinct areas are groups/nested isolation, image alpha masks,
polygon feathering/arbitrary animated paths, mask painting/erasing,
pressure-sensitive strokes and image retouching. None follows automatically
from the soft ellipse implementation.

## Ordered delivery packages

1. **Next package: bounded isolated groups**, including nested groups, persistence,
   shared CPU rendering, native controls and chronological history described below.
2. **Image alpha masks:** add a distinct asset-reference clip variant after groups
   stabilize. Specify alpha-only coverage, explicit rect mapping, transparent
   outside behavior and no external SVG. Extend project/history references,
   deletion guards, loader/cache identity and export resource preflight together.
   Reuse canonical image decoding; charge masks even when dormant at frame zero.
3. **Polygon feathering:** specify an inward distance-to-edge coverage ramp and
   zero-feather hard-polygon equivalence before implementation. Bound raster
   resolution and edge-distance work; do not label SVG blur equivalent. Animated
   topology is a separate increment with explicit correspondence rules.
4. **Mask painting/erasing:** introduce non-destructive, versioned alpha edits with
   bounded dab/stroke storage, worker rasterization, cancellation and atomic Apply.
   Use immutable derived asset identities if raster snapshots are persisted;
   history must not copy pixel buffers or overwrite original assets.
5. **Pressure:** first verify the actual egui/platform input path, then introduce
   bounded pressure samples and deterministic variable-width evaluation. Define
   missing-pressure fallback and sampling independent of display refresh rate.
6. **Retouch:** scope one reversible operation first (for example crop), with
   original asset preserved, explicit operation order and derived-resource cache.
   Clone/heal/color adjustments need separate contracts and resource estimates.

Each package must deliver schema through editor/export together; a schema-only
fixture is not acceptance of the corresponding artist workflow.

## Next package contract: isolated groups

Keep existing flat layers and their order for old documents. Add default-empty
`Document.groups` and default-absent `Layer.group_id`. A group has a globally
unique ID, scene ID, plane, optional parent ID, name, visibility, opacity and
normal/multiply/screen blend. Group list order orders sibling groups; root units
need an explicit persisted sibling order shared by groups and ungrouped layers
(for example typed IDs in default-empty per-scene/plane root order). Do not infer
mixed ordering from unrelated vector positions. An empty order is legal only
for a legacy document with no groups; migration derives its existing layer order.
Layers within a group retain document order, with the same explicit sibling-order
representation when nested groups and layers are mixed. Require each unit exactly
once in its owning sibling list, never silently repair missing/duplicate entries.

The schema owner should finalize names in a small reviewed interface diff before
consumers begin. Preserve strict unknown-field rejection. Keep current version-2
files readable with absent fields; if a format-version bump is chosen, provide an
explicit v2 migration and update every version/history constructor in that same
change. A new grouped document must never be loaded by discarding its hierarchy.

Validate an acyclic forest before evaluation: parent exists; IDs do not collide
with layers; every child shares parent scene and plane; depth at most 8; at most
256 groups and 128 direct children per group. First package groups introduce no
transforms, attachments, time curves or group masks. Child actor attachments keep
their existing translation semantics. A hidden group hides descendants; leaf
half-open timing remains unchanged. Group opacity applies once to the completed
isolated subtree, rather than multiplying each child opacity. Blend applies to
that isolated result against the parent backdrop, in existing sRGB SVG/resvg
semantics. Preserve current ungrouped pixel behavior; do not introduce a color
space migration in this package. Plane separation remains before/after all actors.

Expose transactional group creation, reparent/reorder, property update and
ungroup operations through the artwork command boundary. Reparent must move the
unit and both sibling orders atomically and reject cycles/cross-scene/cross-plane
moves. Ungroup preserves children in order at the former group's position and
explicitly warns that removing group opacity/blend may change appearance.
Delete-group-and-descendants is a separate explicit action; ordinary ungroup
must not delete artwork. Scene cleanup and actor-reference guards must traverse
all applicable entries without competing source mutation paths.

Prepare validated hierarchy/order once per artwork revision, with deterministic
bounded traversal reused by preview and export; do not recursively rebuild trees
per UI paint. Renderer emits isolated nested SVG groups with collision-free mask
IDs and applies existing leaf clips inside their group. Native preview must route
any scene containing groups through the shared compositor, including temporarily
invisible groups whose descendants become active later. Invalidate worker/cache
identity on hierarchy, sibling order, visibility, blend or opacity edits; preserve
stale/cancelled-frame guards and the last complete result. Keep existing public
render/export wrappers and `ArtContext` usable.

Native controls: indented bounded tree, selected group properties, explicit create,
move into/out of group, sibling raise/lower and ungroup. Use detached invalid
reparent/property drafts and a validated Apply. A completed action yields one
shared history transaction; selection/draft changes yield none. Context changes,
Undo/Redo, project load and scene deletion clear stale selection/drafts. Persist
only committed document state. Do not add a second artwork Undo stack.

## Ownership and coordination

| Owner | Exclusive production files in this package | Required handoff |
| --- | --- | --- |
| Artwork schema owner | `crates/motion-art/src/lib.rs` | Types, limits, migration/order rules, atomic commands, prepared traversal contract |
| Persistence integrator | `crates/motion-project/src/lib.rs` | Strict project references/version policy and scene cleanup |
| CPU composition owner | `crates/motion-render/src/lib.rs` | Nested isolation and stable mask IDs using approved traversal |
| Export integrator | `crates/motion-export/src/lib.rs`, relevant sequence export code | Preflight hierarchy/surface work before destination creation |
| Native artist owner | `apps/studio-desktop/src/art_editor.rs` | Tree/drafts/actions, one transaction per Apply |
| Desktop/history integrator | `apps/studio-desktop/src/main.rs`, `project_history.rs` | Worker routing/cache invalidation, chronology and stale-state guards |

The parent assigns each shared file once; coordinate GPU/timeline/native owners
before editing `main.rs`, render/export modules or project validation. Schema
approval precedes renderer/editor work; prepared traversal precedes work-budget
integration. Existing GPU experiment must explicitly reject unsupported grouped
art rather than flattening it. No GPU implementation is part of this package.

## Bounded work and verification

Retain current limits: 32 MiB encoded / 16 million decoded image pixels, 48 MiB
art SVG per frame and four million layer/point frame visits. Add group traversal
and depth costs to checked/saturating export preflight, counting inactive groups
and descendants. Proposed additional cap: 64 million isolated-surface pixel visits
per frame and 256 MiB simultaneous intermediate surface estimate (width * height
* 4 times live isolation depth); reject before raster allocation/output creation.
Finalize estimates against resvg allocation behavior; if full-canvas isolation
cannot fit, fail clearly rather than silently flattening. Preserve worker queue
bounds; no decode/raster work on the UI event loop.

Required automated acceptance:

- Exact old-document ordering/pixels and strict round trips; new grouped project
  save/load; cycle, depth/count, orphan, duplicate order and invalid opacity errors.
- Atomic failed reparent/delete/apply leaves document/history unchanged; mixed
  source/art/asset Undo/Redo restores hierarchy and order; invalid drafts persist
  until corrected without committing.
- Independently calculated overlapping translucent children show opacity applied
  once; nested multiply/screen against colored backdrops; zero/one opacity,
  masks/images, both planes and translation-attached leaves.
- Shared worker decoded RGBA equals export at equal resolution, with declared
  antialias tolerance; scene fade applies once; backwards/repeated seeks and
  interval boundaries use existing sample timing. Test actual SVG/PNG output,
  not only generated strings or a renderer's comparison against itself.
- Hidden/future-active subtree preflight, overflow and surface-budget rejection
  before output creation; cancellation/stale revision cannot replace current frame.
- Run repository-required fmt/check/Clippy/workspace tests and relevant optional
  renderer checks. Record unavailable tools/checks exactly; do not invent results.

A reviewed implementation plus passing Linux/macOS CI closes only this bounded
CPU/editor package. Physical Mac tree interaction, shortcuts, accessibility,
pressure hardware and display/performance inspection remain explicit device gates.
Groups do not complete image masks, feathered polygons, painting, pressure,
retouching, full GPU parity or the full artist-tool milestone. No model download,
GPU/device execution, production edit or test execution was performed for this
specification.
