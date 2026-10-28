# Anchored asset library (ORK-LIB-01)

Owner: `crates/motion-library` (A08). Props: A05. CI workflow: A10.
Contract: [ORK-LIB-01](agents/00-orkestroija.md). Pack used by the repository:
`examples/stick-studio/library/`.

`motion-library` depends only on `motion-art`, `serde`, `serde_json`, `thiserror`
and `sha2`; no existing crate depends on it yet, so the crate graph stays acyclic.

| Module | Responsibility |
| --- | --- |
| `manifest` | Schema v1 types, `AssetId`, single-file validation |
| `library` | `Library::load`, cross-references, licence policy, `search` |
| `store` | Content-addressed store (CAS) for large blobs |
| `assemble` | `assemble_location` / `assemble_set` → `motion_art::Document` + world anchors, cameras, marks |
| `room` | Parametric room generator and its standard variants |

## Layout

```
<pack>/library/backgrounds/<id>.json
<pack>/library/props/<id>.json
<pack>/library/locations/<id>.json
<pack>/library/sets/<id>.json
```

One manifest per file; the file name is the id plus `.json` and the directory
must match `kind`. Other files (README) are ignored; unknown directories, nested
directories, symbolic links, stray `.json` files directly under `library/`,
manifests over 1 MiB and more than 10 000 manifests are errors. Files are read in
sorted order, and every error names the file (relative to the pack) and the field.

## Manifest v1

All structures deny unknown fields.

| Field | Rules |
| --- | --- |
| `schema_version` | `1` |
| `id` | `<kind>.<category>.<name>_v<N>`; segments `[a-z0-9]+(_[a-z0-9]+)*`; `N ≥ 1` without leading zeros. Parsed by `AssetId::parse` |
| `kind` | `background` \| `prop` \| `location` \| `set`, equal to the id prefix |
| `name` | non-empty display text (≤ 256 bytes) |
| `bounds` | `motion_art::Rect` (local, y down), `Rect::validate`; art assets require `x = y = 0` |
| `depth` | default depth `far` \| `mid` \| `near` |
| `tags` | 1–32 unique tags of lowercase letters (also ä/ö/å), digits, `_` |
| `license` | `{license, author, origin, source_url?, redistribution, notice?}` (see below) |
| `palette` | optional `{slot: Color}`; slot names `[a-z0-9_]{1,64}` |
| `geometry` | `art` \| `rig_project` \| `blob` \| `composite` |
| `anchors` | see below |
| `variants` | `[{name, palette?, layers?}]` (not for sets) |
| `room` | location only |
| `parts` | location and set: `[{instance, asset, at, scale?, flip_x?, depth?, variant?}]` |
| `location`, `cameras`, `marks` | set only; `location` is required for a set |

Geometry:

- `art`: `{"type":"art","layers":[{"id","depth"?,"paint"?,"content"}]}`. `content`
  is `motion_art::Content` limited to `rectangle`, `ellipse` and `stroke`; image
  content is rejected (`UnsupportedContent`) because a library asset must be
  self-contained. Each layer is validated with `motion_art`'s own
  `Document::validate` (finite bounded coordinates, positive rectangles, stroke
  width 0.1–4096, point budgets) and layer ids are unique. `paint` names a palette
  slot that replaces the content colour at assembly.
- `rig_project`: `{"path","sha256"}` for existing rig props. The path is relative
  to the pack root (no absolute paths, `..`, `.` or empty components), must not
  resolve outside the pack through a symlink, must be a regular file ≤ 64 MiB and
  must match the digest.
- `blob`: `{"sha256","bytes","media_type"}` describing a CAS object (64 lowercase
  hex digits, 1 byte – 1 GiB, `type/subtype`). The pack only references it; the
  bytes live in the store.
- `composite`: required for locations and sets, forbidden for backgrounds/props.

Variant palette slots must exist in the base palette (the manifest palette plus,
for rooms, the generated room palette). Variant layer ids must not clash with base
or room layer ids. A set declares no variants or anchors of its own: its variant
argument selects the location variant, and marks play the role of anchors.

## Anchors

| Type | Shape | Meaning |
| --- | --- | --- |
| `grip` | `at` | hand attachment point on a prop |
| `seat` | `at` | sitting point (hips) |
| `stand` | `at` | a standing spot (feet) |
| `ground` | `at`, `width` | floor contact segment `[at.x, at.x + width]` at `at.y` |
| `doorway` | `rect` | walk-through opening |
| `screen` | `rect` | display surface (TV, monitor, blackboard) |

Anchors are finite and inside `bounds` (with a 1e-6 tolerance for decimal
round-off); a rectangle must lie wholly inside, a ground segment from `at.x` to
`at.x + width`. Ids are unique per asset (and do not clash with room anchors).
Props follow the A05 convention: bounds origin 0, floor-standing props put the
bottom edge on the floor with a `ground` anchor at `(0, height)` spanning the width.

## Depth and planes

Assembly emits all `far` layers, then all `mid` layers (both
`Plane::Background`), then all `near` layers (`Plane::Foreground`, drawn over
actors). Within a band the order is: the location's own room and variant layers,
location parts in manifest order, then set parts; within an asset the declared
layer order, variant layers last. Effective depth is `part.depth`, else
`layer.depth`, else the asset `depth`. Layer ids are `<scene>_<instance>_<layer>`;
the location itself uses the reserved instance `room`. Parallax is metadata only:
**the renderer does not apply parallax**.

## Transforms, cameras and marks

`world = at + scale · local'` where `local'` is the local point mirrored about the
vertical centre line of `bounds` when `flip_x` is set. Rectangles and ground
segments keep positive widths after mirroring; stroke widths scale with `scale`
(0.01–100). Anchors are transformed the same way and returned as `WorldAnchor`
`{instance, asset, anchor}`.

Set cameras are data with MotionScript `CameraKey` semantics: `x`, `y` move the
camera centre from the canvas centre in scene pixels, `zoom` 0.05–20. Marks are
`{id, kind: stand|sit, at | anchor: "<instance>/<anchor>"}`; anchor marks resolve
to the anchor's reference point (the point itself, the middle of a ground
segment, or the bottom centre of a doorway/screen rectangle). The instance may be
a set part, a location part, or `room`. **MotionScript does not consume sets yet**
(A02 area): cameras and marks are not compiled into camera keys or actor tracks.

## Licence policy

`license` is an SPDX expression, a `LicenseRef-*` identifier or `unknown`.
`origin` is `original` \| `third_party` \| `ai_generated` (`third_party` requires an
`http(s)` `source_url`); `redistribution` is `allowed` \| `forbidden` \| `unknown`.

An asset is distributable when `origin ≠ ai_generated`, `license ≠ unknown` and
`redistribution = allowed`. Locations and sets inherit the result of everything
they reference, transitively (location parts, set parts, the set's location).
`Policy::Authoring` turns each non-distributable asset into a `Warning`;
`Policy::Distribution` fails with `LibraryError::NotDistributable`. All shipped
stick-studio content is original geometry under `MIT`, author `Kilsat Studio`.

## Content-addressed store

`ContentStore::open(root)` keeps objects at `<root>/sha256/<ab>/<64 hex>`. `put`
writes a temporary file under `<root>/tmp/`, syncs it and renames it atomically
into place (idempotent). `get` re-hashes the bytes; a mismatch deletes the file
and returns `StoreError::Corrupt`; a missing object is `StoreError::Missing`.
Objects are limited to 1 GiB by default (`with_max_object_bytes`). The root is
always an explicit parameter; `ContentStore::default_root()` only suggests
`$KILSAT_ASSET_STORE` or `~/Library/Caches/KilsatStudio/assets`. Never point it
into the repository; large files are not committed.

## Room generator and variants

`room` (locations only):

```json
{"width": 1920, "height": 1080, "floor_y": 810,
 "wall": {"r":226,"g":220,"b":204,"a":255}, "floor": {"r":190,"g":172,"b":146,"a":255},
 "windows": 1, "time_of_day": "day", "door": "right"}
```

`width`/`height` 64–8192, `floor_y` 0.5–0.92 of the height, `windows` 0–6 (each
needs at least 80 px and 6 % of the width beside the door), `time_of_day`
`day|evening|night` (window glass colour), `door` `none|left|right`. The output
is a pure function of the spec: wall, floor, three floor seams, skirting, the
door (frame, panel, handle) and evenly spaced windows (frame, glass, mullion,
sill), all `far` and painted from palette slots (`wall`, `floor`, `trim`,
`floor_line`, `frame`, `glass`, `sill`, `door`, `door_frame`, `handle`, `rain`,
`clutter_a|b|c`). Anchors: `ground` at the floor contact line
`floor_y + (height − floor_y)/2` across the width, and `doorway` on the door
panel. **Windows have no anchor**: `screen` means a display surface, and windows
are not one.

`room::standard_variants(spec)` returns `day`, `evening` (warm glass, warmer
wall), `rain` (grey glass and four rain strokes per window) and `messy` (four
small floor items, `mid`). Every location × variant in the shipped pack passes
`motion_art::style::check_background(BackgroundStyle::stick_studio(1920, 1080, 1.0))`
with no issues (test `stick_studio_library_is_valid_and_styled`), and the
generator test covers every valid window/door/time-of-day combination.

## Shipped content and tools

`cargo run -p motion-library --example generate_stick_library -- examples/stick-studio`
regenerates backgrounds, locations and sets deterministically:

- The four original backgrounds (`backgrounds/*.kilsat`) migrated to art
  manifests: sky/wall/floor/distant scenery `far`, furniture and props `mid`,
  nothing `near` (preserving the original stacking behind actors). The cemetery
  tree trunk (28 px) is clamped to 18 px because the library style gate uses actor
  scale 1 (limb 9 px, lines ≤ 2 limbs); the original pack was tuned for 1.6.
- New backgrounds: yard, street and a stairs piece for kit-bashing.
- Room-generated and kit-bashed locations and one set per location (cameras
  `wide`, `two_shot`, `medium`, `close`; stand/sit marks). Prop placement reads the
  A05 prop manifests (bounds, seat anchor); a missing prop falls back to an
  estimated size and is listed on stderr, so rerun after props change.

`cargo run --locked -p motion-library --example validate_library -- examples/stick-studio --distribution`
prints counts per kind, the anchor total (declared plus generated room anchors),
warnings, assembles every location × variant and set, and exits non-zero on any
error. `--preview-svg <dir>` writes flat SVG sketches with anchors (red) and marks
(blue) for authoring review; it is not the production renderer.

## Adding an asset

1. Pick an id `<kind>.<category>.<name>_v1` and write `library/<dir>/<id>.json`.
2. Art props: bounds at the origin, floor contact at the bottom edge with a
   `ground` anchor; add `seat`, `grip`, `screen` or `doorway` anchors where they
   apply. Keep large surfaces muted and strokes ≤ 18 px at actor scale 1.
3. Fill `license` truthfully; third-party assets need `source_url`. Put large
   binary data in the CAS and reference it with `blob`.
4. Run `validate_library <pack> --distribution` and `cargo test -p motion-library`.
5. A changed asset gets a new version suffix (`_v2`) rather than silent edits
   when published scenes depend on it.

## Limitations

- Parallax is metadata; nothing renders it.
- `rig_project` and `blob` assets cannot be kit-bashed: `motion_art` has no filled
  polygon or mesh content, so assembly returns `AssembleError::UnsupportedGeometry`
  instead of skipping them.
- MotionScript does not reference sets, cameras or marks yet (A02).
- `check_background` is a structural style rule, not a perceptual art review;
  generated layouts were only inspected as flat SVG sketches.
- No thumbnails, no editor UI, no CAS garbage collection or download flow.
