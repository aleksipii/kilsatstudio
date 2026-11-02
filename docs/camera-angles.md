# Camera angles

A camera angle says which side of the set the camera films from. The story
format, MotionScript, scene evaluation, artwork and every renderer use the same
four names (`motion_script::CAMERA_ANGLES`):

| Angle | Meaning |
| --- | --- |
| `front` | The set as drawn. Default. |
| `reverse` | The opposite side. Actors are mirrored about the canvas centre and turn around. The set shows artwork drawn for `reverse`. |
| `high` | Looking down. Shows artwork drawn for `high`. Actors are unchanged. |
| `low` | Looking up. Shows artwork drawn for `low`. Actors are unchanged. |

## MotionScript

A camera key may name an angle. It holds from that key until the next key and is
a cut, never blended. A key without `angle` means `front`.

```yaml
composition:
  camera:
    - {at: 0, x: 0, y: 0, zoom: 1}
    - {at: 3.999, x: 0, y: 0, zoom: 1}
    - {at: 4, x: -120, y: 0, zoom: 1.6, angle: reverse}
```

Unknown names are rejected as unsupported, and the message lists the four names.

## Evaluation (one rule for preview and export)

`motion_scene::evaluate` returns `camera.angle`. In `reverse`:

- Each actor's root is reflected about the canvas centre. That means `x → width − x`,
  rotation and `scale.x` are negated, and the camera offset `x` is negated. The
  framing is therefore the same shot, flopped.
- The view turns around before the rig is evaluated: front ↔ back, three-quarter →
  back, side stays side. Per-view rig drawings ([custom rigs](custom-rigid-characters.md))
  apply to the turned view.

## Artwork

An art layer may set `angle` ([art format](art-format.md)). A layer without
`angle` shows from every angle. A layer with `angle` shows only from that angle.
`Document::composition` takes the angle, and the SVG, GPU and desktop renderers
pass `scene.camera.angle`. Editors pass `None` to list every layer.

`ProjectDocument::validate` checks two things:

- Layer angles are known names.
- A scene that has artwork and cuts to an angle other than `front` has at least one
  layer drawn for that angle.

Otherwise turned-around actors would stand in front of an unchanged set.

`Layer::mirrored_x(axis)` reflects vector content (strokes, rectangles, ellipses)
and geometric clips. It rejects images, image and painted masks, animated clips and
actor-attached layers rather than drawing them unmirrored.

## Story format

A beat may set `angle`. It holds until changed and is a cut. Every scene starts
at `front`. The catalog lists each set's angles in the authoring guide, and an angle a
set does not offer is reported as unsupported at its beat path.

A set offers every angle it has layers drawn for. It also offers `reverse` when
its shared (untagged) layers can all be mirrored. The importer copies into each
scene only the layers for the angles that scene films, so long stories stay under
the 1024-layer limit. A scene that cuts to `reverse` uses the set's drawn reverse
when it has one. Otherwise it gets a mirrored copy of every shared layer. Chairs
added by `pose: sit` are always mirrored.

### Bundled sets

All thirteen bundled sets are drawn for all four angles: the original four in
`scripts/stick_scenery.py` and the nine city sets in `scripts/stick_city.py`
(reverse drawn by hand, high/low by the same horizon move, which also moves clip
rectangles). The original four:

- `reverse` is the opposite side, drawn by hand. The café shows its entrance, menu
  board and coat rack. The office shows a whiteboard, filing cabinet and printer.
  The bus stop shows the houses and kiosk across the street. The graveyard shows
  the chapel and its gate.
- `high` and `low` redraw the front with the horizon moved: to y=640 looking down
  (more floor) and to y=880 looking up (more wall; indoors also a ceiling band).
  Round shapes such as the moon and the clock stay round. Actors keep standing at
  y=960.

## Desktop

A layer drawn with the pen or shape tools, or an image added in the art panel,
belongs to the camera angle at the playhead. At a `reverse` key it gets
`angle: reverse`, for example. In front shots new layers stay shared by every
angle. The inspector's angle picker changes this later. Importing an image while
previewing an angle is therefore how a hand-drawn per-angle background is added.

## Limitations

- Mirroring is a flop: any lettering in a mirrored set reads backwards. Draw a
  `reverse` layer where that matters.
- Draw order does not change with the angle. Seen from behind, a chair back stays
  behind the actor, and a hand prop stays in front of the actor's body.
- Lights and actor-attached artwork are not mirrored.
- `high` and `low` do not change actors or perspective; they only select artwork.
  The bundled high/low sets are a vertical redraw of the front, not a new layout.
- The desktop camera-key editor and layer inspector have angle pickers, but they
  were not exercised by hand in the running application.
