# Night-room reference sequence

An original four-shot dialogue sequence in a flat cut-out style, built only from
this repository's features. No third-party art, characters or audio. It follows
the shot list in [docs/professional-animation-plan.md](../../docs/professional-animation-plan.md)
with neutral dialogue.

| # | Shot id | Framing | Content |
| --- | --- | --- | --- |
| 1 | `bed_closeup` | close-up, slow push | Boy (pompom beanie, football pyjamas) with a phone; silent reaction hold |
| 2 | `doorway_wide` | establishing | Father (work shirt) in a lit doorway; line 1 |
| 3 | `boy_phone_medium` | medium, slow push | Boy with the phone, lamp on the bedside table; line 2 |
| 4 | `dad_closeup` | close-up, slow push | Father close-up; line 3 |

## Files

- `base.yaml` - project settings and both custom rigid rigs (colored polygons).
- `shots.yaml` - the shot list and cut rules (see [docs/shot-library.md](../../docs/shot-library.md)).
- `night-room.kilsat` - generated project: expanded screenplay, bedroom/doorway/hallway
  artwork layers (rectangles and ellipses defined in
  `crates/motion-export/tests/support/mod.rs`) and audio references. Open it with the
  desktop app's Open project action.
- `audio/*.wav` (gitignored: not in the repository; created by `-- write`, and regenerated in a temporary directory by the tests when absent) - generated 24 kHz mono placeholder tones with a crude syllable
  rhythm. They are **not speech**; they only give the sample clock real audio.

## Commands

```sh
export CARGO_TARGET_DIR=/private/tmp/claude-501/shared-target   # optional
cargo run -p motion-export --example night_room_reference            # timing report
cargo run -p motion-export --example night_room_reference -- write   # regenerate files
cargo test -p motion-export --test night_room_reference              # golden-property tests
KILSAT_NIGHT_ROOM_DUMP=/tmp/night-room cargo test -p motion-export --test night_room_reference
```

The report prints each shot's start/end, dialogue start and the cut points on the
24 kHz sample clock. After changing `base.yaml`, `shots.yaml` or the artwork code,
run `write` and commit the regenerated `night-room.kilsat` and `audio/`; the test
`committed_example_matches_the_generator` fails while they are stale. The dump
variable writes one PNG per shot for visual review. No pixel hashes or binary
frames are stored (see the docs for why).

## Placeholders and TODO hooks for later workstreams

- `TODO(captions)`: speaker-coloured captions (workstream 1). No text is drawn.
- `TODO(lip-sync)` / `TODO(visemes)`: the `mouth_placeholder` parts are static; no
  phoneme or viseme data is attached to the lines (workstream 2).
- `TODO(acting)`: eyes and brows are static polygons; there is no gaze, blink or
  expression variants (workstream 3). Emotion events are metadata only on custom rigs.
- `TODO(lighting)` and `TODO(props/phone-screen)`: no light sources; the phone is a
  rectangle with a flat `phone_screen_placeholder`, and the lamp, window and
  hallway are flat shapes (workstream 4).
- Dialogue audio is a placeholder tone. Replace `audio/*.wav` with imported or
  synthesized speech and update each line's `duration` in `shots.yaml`.
