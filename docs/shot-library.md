# Shot library and the night-room reference sequence

Workstream 5 of the [professional animation plan](professional-animation-plan.md),
built only on what exists on main: scene-local camera keys, actor tracks, speak
events, custom rigid rigs, artwork layers and hard scene cuts. Nothing here
depends on captions, lip-sync data, acting variants or lighting; those are
hooks (below).

## Shot library (`motion_script::expand_shots`)

A `ShotList` is declarative YAML/data (strict schema, unknown fields are syntax
errors). `expand_shots(&base_script, &list)` is a pure, deterministic function
returning an ordinary validated `Script` (one scene per shot, appended after the
base scenes) plus a `plan` of cut times. It adds no new screenplay field, so
existing screenplays, history and the editor are unaffected; the list is a
separate input like the camera-preset API.

Framing presets (camera zoom at the start, default end/start push):

| preset | zoom | default push | notes |
| --- | --- | --- | --- |
| `establishing` | 1.0 | static | wide |
| `medium` | 1.5 | 1.04 | |
| `close_up` | 2.4 | 1.06 | |
| `over_the_shoulder` | 1.8 | static | needs `side: left|right`; subject shifts 18% of the canvas width off-centre |
| `reaction_hold` | 2.0 | static | used by reaction shots |

The camera centres `actor` (or `point`) plus `offset` (scene pixels). Actor
targets follow the actor's `place` position and `move` events, with a camera key
at every move boundary. Camera keys are ordinary `composition.camera` keys.
`place` (held absolute position) and `off_stage` (moved three canvas widths
away) expand to single-key actor tracks, so an actor can be shown or hidden per
shot. Authored actor tracks are not part of the framing computation and conflict
with `place`/`off_stage` for the same actor.

Cut rules (`cut:` defaults, overridable per shot with `lead_in` and `hold`):

- A shot with a `line` starts `lead_in` seconds before the line (default 0, max
  2), so the cut lands on the dialogue start or just ahead of it, and ends `tail`
  seconds after it (default 0.5, max 5). The line `duration` is supplied by the
  author (the length of the imported or placeholder audio); the module reads no
  files.
- A shot without a line holds for `hold` seconds (default `reaction_hold`, max 10).
- `reaction: {actor|point, offset, hold}` appends a `<id>_reaction` scene after a
  line's shot with static `reaction_hold` framing and a bounded pause. It
  inherits the shot's `place`/`off_stage` unless it names its own.
- Only hard cuts are supported; a crossfade in a shot's composition is an
  `Unsupported` error, and `composition.camera` must be empty.

Bounds: 64 shots, 120 s per shot, 64 extra events per shot, line 60 s. Errors are
`ShotError` (`UnknownReference`, `Unsupported`, `Conflict`, `Bound`, `Invalid`)
and convert to `motion_script::Error`. Nothing is clamped; non-finite or
out-of-range values are rejected. Unit tests (in `shots.rs`) cover cut timing,
framing geometry through the camera projection, moving targets, over-the-shoulder,
stage tracks, determinism and every error class.

## Reference sequence

See [examples/night-room-reference/](../examples/night-room-reference/README.md):
`base.yaml` (original polygon rigs), `shots.yaml`, generated `night-room.kilsat`
and placeholder `audio/*.wav`. The helper
`cargo run -p motion-export --example night_room_reference` prints the shot
start/end times and cut points from the 24 kHz sample-clock timeline;
`-- write` regenerates the committed files.

`crates/motion-export/tests/night_room_reference.rs` (6 tests) loads the example
and renders frames through the shared CPU sequence renderer (SVG then resvg). It
checks: committed files equal the generator output; 640x360 dimensions; cut
times on the sample clock; line audio starts 0.15 s after each cut with silence
before it; frames are non-blank and opaque; flat colours at world points located
through the evaluated camera (headboard, beanie, hallway light, work shirt,
floor, lamp, pyjamas, hall wall, skin); each cut visibly changes the frame while
frames just before it stay continuous; repeated, reversed and shuffled seeks and
the retained `SequencePreview` produce byte-identical frames.

No hashes or binary goldens are stored: resvg antialiasing can change between
versions and platforms, so exact-byte goldens would be brittle. Exact-pixel
checks are limited to flat-colour interiors with a tolerance of 2. To refresh
after an intentional change, run the example with `write` and commit the
regenerated files; set `KILSAT_NIGHT_ROOM_DUMP=<dir>` to dump one PNG per shot
and review it by eye.

## Real today versus placeholder

Real (checked by tests on Linux/macOS CPU paths): shot expansion, cut timing on
the sample clock, camera framing, stage placement, original rigs and artwork,
deterministic seeks, placement of audio on the shared clock.

Placeholder or absent: captions (no text is drawn); mouth shapes (static
`mouth_placeholder` parts, no viseme data); eyes, blinks and expressions;
lighting (no glow from the phone, hallway or lamp); the phone screen (flat
rectangle); dialogue audio (generated tones, not speech; no Kokoro run); art
quality, framing taste and comic timing, which need human review. The GPU path
and the desktop UI were not exercised, and the editor does not display shot
lists. Hooks for later workstreams are the `TODO(...)` comments in `base.yaml`
and `shots.yaml`, the named `*_placeholder` rig parts, and the `line` records,
whose text and audio reference are what captions and alignment will consume.
