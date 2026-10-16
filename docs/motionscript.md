# MotionScript supported subset

See `examples/hello-emma.yaml` for the combined movement and Kokoro dialogue demo.
The full target specification is `docs/master-build-prompt.md`; this page describes
what this implementation actually accepts.

- `project` is optional. Its default is title `Untitled`, 24 fps, 1920×1080.
  Explicit projects may override defaults; title must be nonempty, fps 1–240 and dimensions 1–16384.
- `characters` maps stable IDs to a nonempty `asset`, optional `default_pose: idle`,
  and optional `voice` (`provider: kokoro`, English language, preset, speed 0.5–2).
  Asset strings identify future artwork; the desktop currently draws its own
  procedural placeholder actors and does not load a rig from those paths.
- `scenes` contain unique IDs, positive finite durations, optional background names,
  and events. Background names are metadata; `composition.background_color` and
  timed `composition.background_keys` control the rendered solid backdrop. See
  [scene composition](composition.md).
- Each event starts at a finite nonnegative `at` less than scene duration and names
  an existing `actor`. Unknown fields and commands fail explicitly.

| type | Required additional fields | Behavior |
| --- | --- | --- |
| move | x, y, duration | Linear movement to project-space coordinates, from the last completed pose; positive duration, ending inside the scene |
| emotion | value | neutral, happy, sad, surprised or angry |
| gesture | value, duration | wave only, deterministic envelope over a positive duration |
| speak | text | Dialogue job; requires character voice or event audio; optional emotion and audio |

Move intervals for one actor cannot overlap. At time zero actors begin at the
project center horizontally and 70% of height vertically. Stable source order
resolves simultaneous expression changes. Seeking calls the same stateless
`motion_scene::evaluate` as offline pose export; it does not replay stateful steps.
Speech emotion changes the visual expression but remains metadata for Kokoro
synthesis: no acoustic emotion conditioning is implemented.

Imported `audio` is a project-relative nonempty 24 kHz mono PCM16 WAV. Speech text,
preset, language, speed, model/runtime identity and local asset hashes determine
the synthesis cache. Voices need preinstalled, approved local model assets; scene
movement preview works without those assets. Kokoro tokens have no certified
phoneme timestamps; supply manual or separately aligned timing for mouth shapes.

The desktop supports silent scene playback, selected-line previews, and prepared
full-scene dialogue mixes. Source playback position drives movement and timed
mouth tracks when audio is loaded; a silent transport advances quantized sample
time otherwise. Full-scene preparation preserves leading silence and rejects
same-actor overlapping dialogue. Device latency and actual macOS GUI execution have not been validated here.

Offline pose sampling:

```sh
cargo run -p motion-scene --example evaluate_scene -- examples/hello-emma.yaml 0 /absolute/new-poses.json
```

The output is sampled actor poses at project fps, not rendered images or video.
The command refuses to replace an existing output and caps frame/actor counts.
