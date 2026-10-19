# Custom rigid characters

A character may optionally include a version 1 inline `rig` definition. Existing
characters without one retain the original procedural humanoid. The desktop
custom-rig editor provides a detached YAML draft with Apply, Reset and Use original
humanoid actions. Apply uses the existing source history and full script validation;
invalid definitions do not replace the project. The definition lives in screenplay
source and therefore saves and reopens in `.kilsat` projects.

```yaml
characters:
  PUPPET:
    asset: custom
    rig:
      version: 1
      id: puppet
      joints:
        - {id: root, parent: null, x: 0, y: -40}
        - {id: head, parent: root, x: 0, y: -40}
      parts:
        - id: body
          joint: root
          points: [[-20, -20], [20, -20], [20, 40], [-20, 40]]
          color: [70, 160, 220, 255]
        - id: head_shape
          joint: head
          points: [[-18, -18], [18, -18], [18, 18], [-18, 18]]
          color: [245, 200, 160, 255]
scenes:
  - {id: intro, duration: 2, events: []}
```

Stable definition, joint and part IDs contain 1–128 ASCII letters, digits,
underscores or hyphens. Joint IDs and part IDs are unique within their respective
collections. Joints appear before children; unknown parents and cyclic hierarchies
fail validation. Joint `x`, `y` and radian `rotation` default to zero; `scale_x` and
`scale_y` default to one. Parents compose translation, rotation and scale into child
coordinates. A part names its attachment joint and supplies 3–64 joint-local polygon
vertices and RGBA bytes. Parts draw in source order. Definitions contain 1–256
joints and 1–256 parts. Both authored and evaluated coordinates must remain finite
and within the rig's existing bounds. Collapsed polygons fail validation.

The validated scene snapshot prepares rest hierarchy and rigid part geometry once.
Preview and export use those same evaluated local polygons and the actor's existing
root translation/rotation/scale and camera transform. Custom rigid artwork replaces
the procedural body and face; the character label remains. Existing humanoid
procedural clips, seven-joint masks and visemes do not deform these custom parts.

This increment supports editable rigid colored polygons. Texture bindings, mesh
skinning, custom-rig animation clips, retargeting and a visual joint/vertex canvas
remain separate work. Physical Mac editing and rendering acceptance require actual
device evidence; source fixtures and CI do not establish that acceptance.

## Try the integrated example

Open `examples/studio-production-demo.kilsat` with the desktop Open project action.
It contains only original colored shapes and inline source; it has no asset paths
to resolve, no speech events and no model or network requirement. Play or seek the
four-second scene. The polygon puppet moves to the right through a root movement;
the original humanoid moves left and loops two hand clips in the same normalized
group. The warm and cool background shapes belong to a nested isolated artwork
group, with separate group opacity and screen blending.

Select PUPPET and open Custom rigid character. Change the head polygon color or
its joint's `rotation`, Apply, then Undo/Redo. Save a copy and reopen it to check
persistence. Select EMMA and inspect the adjacent `high` and `out` live rows: both
use group `reach` and weight `0.6`. Changing a row weight changes the normalized
contribution; changing row order inside this group preserves the weighted result.
Seek repeatedly across the two-second loop boundary. The puppet intentionally has
no humanoid bone tracks, live clips, gesture events or speech deformation.

This example makes the source changes reviewable. Its JSON structure was checked
when created; repository CI must establish full Rust project decoding, evaluation
and rendering. Manual desktop actions above remain a physical Mac acceptance task.
