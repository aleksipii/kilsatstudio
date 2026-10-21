# Shot library and the night-room reference sequence

Workstream 5 of the [professional animation plan](professional-animation-plan.md),
built on scene-local camera keys, actor tracks, speak events, custom rigid rigs,
artwork layers and hard scene cuts. The reference sequence (below) now also
integrates captions, text-estimated mouth variants, the acting layer, lights and
an animated phone prop; the shot library itself does not depend on them.

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
`base.yaml` (original polygon acting rigs, `project.captions`), `shots.yaml`,
generated `night-room.kilsat`, three tiny generated phone-app PNGs in `assets/`
and placeholder `audio/*.wav` (gitignored). ONE project renders everything:

- **Captions**: white (`DAD`) and yellow (`STAN`) via `speaker_fill`, uppercase,
  bottom-centred, Anton (licence not approved, see captions.md).
- **Mouths**: each rig maps the 12 non-rest visemes to its own mouth variant;
  the viseme track is the text estimate of the real line text (no alignment is
  attached; source `Estimated kilsat-english-estimator`).
- **Acting**: sclera + pupil parts with gaze (boy at the phone, then a glance
  towards the door; father towards the bed, an off-screen point), seeded blinks
  (2.2 s / 2.3 s windows), brows via `sad` / `surprised` expressions.
- **Lighting** (`tests/support`): violet ambient multiply plus vignette in every
  shot, cool phone glow tied to the phone layer by `screen_glow`, warm noise-flicker
  lamp in the boy medium shot, green hallway light in the doorway and father shots.
  Lights are drawn after artwork and actors, so they glow over the characters
  rather than behind them (a known limitation).
- **Phone prop**: an `animated_image` layer of three generated 40x68 PNG frames
  (looping, 0.6 s each) over an upright rig phone; the layer is positioned for the
  boy placed at (320, 300), the placement both phone shots use.

The dialogue is original and neutral: "Hey, kid. What are you doing? Are you
playing a game?" / "No. I'm trying a face-editing app." / "Oh OK. Is that a game?"

`cargo run -p motion-export --example night_room_reference` prints the timing
report: shot start/end, cut points, dialogue starts, caption windows (speak `at`
to clip end on the sample clock) and per-line mouth interval and variant-switch
counts. `-- write` regenerates the project, PNGs and tones; `-- frames <dir>
[wav-dir]` writes one frame per shot plus `contact-sheet.png` (set
`KILSAT_NIGHT_ROOM_SIDE` to change the preview size); `-- export <dir> [wav-dir]
[--ffmpeg path]` writes the PNG sequence and, with an explicit FFmpeg, the MP4.
`*.wav` is gitignored, so tests regenerate the tones in a temporary directory.

`crates/motion-export/tests/night_room_reference.rs` loads the example and renders
through the shared CPU sequence renderer (SVG then resvg). Default tests (no real
audio needed): committed files and PNGs equal the generator; cut times on the
sample clock; every feature configured; flat-colour probes through the camera on
an unlit copy; each cut visibly changes the frame; repeated, reversed, shuffled and
fresh-preview seeks are byte-identical (lamp flicker, blinks, mouths and captions
included); captions differ from hidden captions only in a bottom-centred area, in
the speaker colour, exactly within the clip window; no missing glyphs; the mouth
area follows the estimated visemes; blinks occur in each shot and pupils follow
the gaze; raised brows; phone frames animate and their glow follows the shown
frame; tint, vignette, lamp, phone and hallway lights act on the expected areas;
preview frames equal the PNG-sequence export frames. No binary goldens or hashes
are stored (resvg antialiasing can change); exact-pixel checks use flat interiors.
The rendered frames were not committed (about 1 MB per contact sheet).

`real_speech_rebuilds_the_sequence_and_renders_everything` is `#[ignore]`d: it
re-times the shot list to real WAV lengths and checks structure. Produce the WAVs
offline with `scripts/night_room_real_speech.py` (Kokoro venv python, outside the repository).

## Real-speech run (one hand run, 2026-10-20, Apple M1, macOS 27.0.1 arm64)

Command: `"$HOME/Library/Application Support/Kilsat Studio/Kokoro/venv/bin/python" -I
scripts/night_room_real_speech.py` (offline, no downloads; it drives
`services/kokoro-runtime/runtime.py` through the same request file protocol as the
app, one process per line). Stack: Python 3.12.15, kokoro 0.9.4, torch 2.6.0,
numpy 1.26.4, misaki 0.9.4, spacy 3.7.5 with en-core-web-sm 3.7.1, Python/PyTorch
CPU, model `kokoro-v1_0.pth` (SHA-256 `496dba11...f18ad1e4`), voices `am_michael`
(father) and `af_heart` (boy; a female preset, chosen only because it is installed).
Output WAVs: `~/Library/Application Support/Kilsat Studio/night-room-audio/audio/`
(outside the repository, gitignored).

| line | voice | audio | wall time | wall/audio |
| --- | --- | --- | --- | --- |
| `dad_doorway` | am_michael | 3.650 s | 8.69 s | 2.38 |
| `stan_phone` | af_heart | 2.625 s | 6.17 s | 2.35 |
| `dad_closeup` | am_michael | 2.575 s | 4.81 s | 1.87 |

Wall time includes a fresh Python process and model load per line, so it is not a
steady-state real-time factor. Rebuilt with these WAVs (`-- report <dir>`) the cuts
move to 2.400, 6.800 and 10.175 s, total 13.500 s; dialogue starts 2.550, 6.950 and
10.325 s; caption windows 2.550-6.200, 6.950-9.575 and 10.325-12.900 s; mouth
intervals / variant switches per line 27/28, 20/21 and 11/12 (placeholder tones:
cuts 2.400, 6.450, 9.400, total 11.650 s). A PNG-sequence export of the real-audio
version (324 frames plus mixed WAV, debug build) took 180 s. Not done: MP4 (no
FFmpeg on this machine), listening review, any lip-sync measurement against the
audio. "Oh OK. Is that a game?" came out 2.575 s long, longer than the words
suggest (possible trailing silence; not checked).

## Real today versus placeholder

Real (checked by tests on Linux/macOS CPU paths): shot expansion, cut timing,
framing, deterministic seeks, captions, text-estimated mouth variants, gaze,
blinks, brows, lights, animated prop, preview/export agreement.

Placeholder or unreviewed: all art, style, framing taste and comic timing; the
mouth shapes (flat polygons, estimated from text only, no forced alignment, never
compared with the audio); eye and brow look; the font and lexicon licences (not
approved); default dialogue audio (generated tones, not speech); the real-speech
run is a structural run on one Mac, with no listening review. The GPU path (no
captions, lights, animated props) and the desktop UI were not exercised, and the
editor does not display shot lists. Human review is still required before any
claim of a professional result.

### Backlights

Scene lights accept `behind_actors: true` to composite after background artwork
but before characters and foreground props. Use this for a hallway glow behind
a silhouette. The omitted/default value is `false`, preserving the historical
overlay behavior for lights intended to tint characters. Camera transforms,
active intervals, flicker and animated screen sampling use the same evaluation
in preview and export. This is compositing order, not physical shadow casting.
