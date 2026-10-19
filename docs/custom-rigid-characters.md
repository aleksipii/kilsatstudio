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
