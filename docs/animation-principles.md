# Animation principles (`principles:`)

The procedural rig layer (`motion_animation::rig_motion`) shapes every gesture
with anticipation, an overshooting attack that settles, overlapping action and
follow-through. These are script-controlled values, not hard-coded constants.
They apply to characters with a custom `rig` (for example the stick-studio
characters). The original seven-joint humanoid has no rig layer, so a
`principles:` block on it is rejected as unsupported.

```yaml
characters:
  VEIKKO:
    asset: veikko
    rig: { ... }
    principles: cartoon            # preset shorthand
  SISU:
    asset: sisu
    rig: { ... }
    principles:
      preset: subtle               # optional starting point
      overshoot: 1.6               # explicit fields override the preset
      moving_hold: off             # on, off or a strength 0..1
      anticipation_frames: 5       # anticipation lasts 5 frames
      overlap_frames: 3            # forearms and head trail by 3 frames
scenes:
  - id: s
    duration: 4
    events:
      - { type: gesture, actor: VEIKKO, at: 0.5, value: jump, duration: 1.2,
          principles: { preset: snappy, squash_stretch: 0.2 } }
```

| field | range | default | effect |
| --- | --- | --- | --- |
| `anticipation` | 0–0.5 | 0.12 | depth of the move against the action before it starts |
| `overshoot` | 0–3 | 1.3 | "back" easing constant of the attack; 0 settles without passing the pose |
| `follow_through` | 0–1 | 0.25 | swing past rest when the pose is released |
| `overlap` | 0–0.3 | 0.08 | progress lag of forearms and head behind the joints that drive them |
| `squash_stretch` | 0–0.3 | 0 (off) | largest torso stretch or squash; the torso keeps its area (`sx·sy = 1`) |
| `moving_hold` | 0–1, `on`, `off` | 0 (off) | breathing and head sway during long holds |
| `anticipation_frames` | whole frames 1–24 | unset | length of the anticipation in frames, whatever the gesture's length |
| `overlap_frames` | whole frames 0–12 | unset | lag of forearms and head in frames; exclusive with `overlap` in one block |

| preset | anticipation | overshoot | follow_through | overlap | squash_stretch | moving_hold |
| --- | --- | --- | --- | --- | --- | --- |
| `subtle` | 0.06 | 0.6 | 0.12 | 0.05 | 0 | 0.5 |
| `cartoon` | 0.25 | 2.2 | 0.45 | 0.14 | 0.18 | 1 |
| `snappy` | 0.08 | 1.8 | 0.15 | 0.04 | 0.08 | 0.6 |

## Resolution

Values come from the defaults, then the character's block, then the gesture's
block. A `preset` restarts from that preset, and explicit fields override it.
A gesture block may not set `moving_hold`, because the moving hold plays
between actions. The character's value is kept.

Frame counts follow the same order: a gesture's own `anticipation_frames` or
`overlap_frames` wins; otherwise the character's applies, unless the gesture
block sets a `preset` (or, for the overlap, a fractional `overlap`).

## Timing in frames

Without frame counts the anticipation is 12 % and the overlap 8 % of the
gesture, so in frames they grow with the gesture: a 1 s gesture at 24 fps
anticipates for about 3 frames and a 2 s gesture for about 6. With
`anticipation_frames: 5` every gesture anticipates for 5 frames at the project
frame rate (the common 4–6 frame rule). The attack keeps its length (26 % of
the curve) and starts after the anticipation. The anticipation is fitted into
2–30 % of the curve, so a 5-frame anticipation in a 0.5 s gesture at 24 fps is
shortened; the staging hints report that. `overlap_frames` sets the lag of
child joints the same way and is at most 30 % of the gesture.

## Paths and staging

- **Arcs**: a move travels in a straight line unless it sets `arc` (a circular
  arc, at constant speed with a linear ease) or `bezier` (one or two control points). The
  move's `ease` applies along the curve. Bézier curves are parameterized, not
  arc-length normalized, so their speed varies with the control points. The
  feet keep the gait's step rate, so a long arc on a walk can slide slightly.
- **Stagger**: `op: stagger` in the schedule format offsets children by
  `delay`, which may be whole frames (`delay: 3f`).
- **Staging hints** (`motion_script::check_staging`, shown in the desktop app
  under "Animaatioperiaatteiden vihjeet"): two or more moves or gestures in one
  scene that start on the same frame *and* stop on the same frame get a hint to
  offset them by 2–4 frames. A gesture whose `anticipation_frames` does not fit
  gets a hint too. Hints never change the script or block rendering.

## Behavior

- **Defaults are unchanged.** With no block, or a block that repeats the
  defaults, every frame evaluates bit-identically to the earlier fixed constants.
  A unit test compares the curve against a frozen copy of the old formula.
- **Squash and stretch** scales only the `torso` joint, so arms stay attached at
  the shoulders and legs are untouched. The torso stretches while the arm moves
  into the pose and squashes as it settles or is released. `jump` squashes in
  the crouch and on landing and stretches on take-off. The stretch is at most
  ±0.3.
- **Moving hold**: after 0.5 s without a move, gesture or sit/stand, a slow
  breath (3.6 s) raises the chest and shoulders slightly and the head sways
  (5.3 s). It fades in over 1 s, uses no randomness and stops during actions.
- All layers are pure functions of scene time, so a seek equals frame-by-frame
  playback, and preview and export match.

Not covered yet: the attack length and the start of the release stay fixed
fractions of the curve; only the anticipation length is adjustable. P01's shared easings (`Back`,
`EaseInOut`) were not in `main` when this was written. The curve keeps its own
"back" formula, whose overshoot constant is `overshoot`.
