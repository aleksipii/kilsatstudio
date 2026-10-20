# Scene lighting and animated image props

Workstream 4 of the [professional animation plan](professional-animation-plan.md).
Lights and animated props are part of the persisted project artwork (`art` in a
`.kilsat` project, type `motion_art::Document`). They are not MotionScript
screenplay commands, and there is no native inspector for them yet: they are
authored as project JSON (or through `motion_art::Command::{AddLight,
ReplaceLight, RemoveLight}` / `Command::Add` for the prop layer) and are covered by
the shared project history. Projects without lights or animated props serialize
and render exactly as before (`lights` is omitted when empty).

## Lights

`Document.lights` is a list of lights, each with `id`, `scene_id`, optional
`visible` (default true), `active` (`{start, end}` scene seconds, half-open),
optional `flicker`, and a `source`:

| `source.type` | Fields | Rendering (shared SVG path) |
| --- | --- | --- |
| `ambient` | `color`, `strength` 0-1 | full-canvas rectangle, `multiply` blend, opacity = strength |
| `vignette` | `strength` 0-1, `inner` 0 to below 1 | black radial gradient, clear inside `inner` |
| `radial` | `position`, `radius`, `aspect`, `softness` 0-1, `color`, `intensity` 0-1, `blend` (`screen` default, `multiply`, `normal`), `attachment`, `track`, `screen_glow` | ellipse with a smoothstep radial gradient |

Colours must be opaque (`a: 255`). Draw order: all artwork, then ambient and
vignette lights in list order (canvas space, unaffected by the camera), then
radial lights (scene space, moved by the camera). Glows are drawn after the
darkening on purpose, so a `screen` glow stays bright in a dark room regardless of
list order.

- **Position.** Scene-space `position`. With `attachment: "ACTOR"` the actor's
  translation is added (like layer attachment); a missing actor hides the light.
  A non-empty `track` of `{time, x, y}` keys replaces `position` (linear, endpoints
  hold, strictly increasing times within the scene).
- **Softness.** 0 is a hard disc; 1 falls off smoothly from the centre. `aspect` is
  height/width of the ellipse.
- **Flicker.** `{mode: sine|noise, amount 0-1, rate_hz 0.01-60, seed}` multiplies
  the intensity (or strength) by `1 - amount * w(t)`, `w` in `[0, 1]`. `sine` is a
  sine whose phase comes from the seed; `noise` is smooth value noise over an
  integer hash (splitmix64) of `(seed, floor(t * rate))`. Both are pure functions of
  scene-local time: seeking, preview and export give identical results. There is no
  RNG or wall clock. (The sine uses the platform `sin`, so cross-platform bit
  equality of flicker is not claimed; same-machine determinism is tested.)
- **Screen glow.** `screen_glow: {layer_id, gain 0-4, tint 0-1}` ties a radial
  light to an image or animated-image layer in the same scene. The light is centred
  on the prop rectangle (plus `position` as an offset); its colour moves toward the
  displayed frame's average colour by `tint`; its intensity is multiplied by
  `gain` and the frame's luminance, so a black frame emits nothing. The average
  uses a fixed 16x16 sample grid (bounded work, deterministic). The glow is hidden
  while the prop layer is inactive.

Limits: 64 lights per document, 16 per scene, 128 track keys per light (2048
total), radius 0.5-20000. Each light adds 24 work units per frame to the export
preflight visit budget (four million visits, the same budget as other artwork), and
`validate_scene_duration` rejects tracks or intervals beyond the scene in project
validation and in export/preview preflight. A light attachment must name a
character in the project.

## Animated image props

A layer with `content: {"type": "animated_image", "rect", "frames", "playback"}`
shows one project image asset at a time. `frames` is a list of
`{asset_id, duration}` (1-256 frames per layer, 4096 per document, durations 0.001
to 3600 s, total at most 86400 s). `playback` is `hold` (default, last frame stays)
or `loop`.

Time is measured from the layer's `active.start` (scene start when `active` is
absent). Frame `i` covers `[start_i, end_i)`, so a time exactly on a boundary shows
the next frame; boundaries are cumulative `f64` sums in list order (for example
durations of 0.1 are not exact multiples). Only the selected frame is embedded in
each SVG.

Every frame asset is reported by `Layer::image_assets`, so the existing checks
cover all frames, not just the first: project validation (`must reference an image
asset in this project`), history restore, asset removal, and export/preview
preflight (`missing artwork image resource`, checked for future frames too).

## Example: night room

`examples/night-room-lighting.json` is an original artwork document for a
640x360 scene `intro`: a dark violet wall, a warm flickering lamp at the upper left,
a face and a looping three-frame phone screen (`phone-home`, `phone-chat`,
`phone-dark`) with a cool screen glow aimed up at the face, an ambient violet
multiply tint and a vignette. A project using it needs a scene `intro` and three
image assets with those ids. `crates/motion-export/src/lighting_tests.rs` renders
it with solid-colour stand-in frames and asserts that the lamp area is warm, the
face is cool while the screen is bright and loses the cool glow on the dark frame,
and the room corners stay dark.

## GPU path

The experimental GPU path does not draw lights or animated props.
`scene_commands::build_with_art` returns `Error::UnsupportedFeature` for any scene
that has a light or an animated-image layer (even an inactive one), rather than
render a different picture. Use the SVG/resvg path for those scenes.

## Validation actually run

Unit tests cover frame selection at exact boundaries, flicker purity/bounds/seed
dependence, track interpolation, validation failures (non-finite values, bad
ranges, duplicate ids, light counts, screen-glow references, unknown fields),
command atomicity, project/history/export preflight failures, and pixel tests:
screen glow brightening/tinting only its area, multiply and hard-edged lights,
ambient strength, vignette, glow over ambient, flicker identical after seeking
away and back, track and actor attachment, frame selection at exact times
(hold/loop/active offset), screen-glow following the displayed frame, native
preview rgba equal to the resvg raster, PNG export frames matching the SVG
reference at lit points, and legacy documents unchanged. These establish
determinism and structural/pixel correctness of the CPU path only; they are not a
perceptual or hand-run GUI review.
