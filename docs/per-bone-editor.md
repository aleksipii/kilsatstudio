# Authored per-bone transform tracks

`composition.bone_tracks` stores scene-local actor and stable bone names. The
original rig names are `hip`, `chest`, `head`, `left_hand`, `right_hand`,
`left_foot`, and `right_foot`. The existing root `actor_tracks` remain independent.

```yaml
composition:
  bone_tracks:
    EMMA:
      chest:
        - {at: 0, rotation: 0}
        - {at: 1, rotation: 0.3, easing: smoothstep}
      right_hand:
        - {at: 0, x: 0, y: -12}
```

Each bone samples the same scene clock as existing motion clips. Translation
adds to the bone's parent-local position after posture/walk/gesture rules;
rotation adds in radians and scale multiplies. Parent changes propagate through
the original hierarchy. Root tracks then transform the resulting actor. Speech
motion remains its existing subsequent layer. Keys hold their first/last values
outside their key span; an identity first key avoids a pre-roll offset.

The editor is a detached draft. Apply validates the complete screenplay and
returns one source transaction for the caller's Undo history; Reset restores the
loaded track. Empty Apply removes the selected track. YAML normalization removes
comments, as in the existing keyframe editor. No artwork or texture rig is implied.

Validation rejects unknown actors/bones, empty persisted tracks, duplicate or
unordered times, nonfinite values and times beyond scene duration. A track allows
1–4096 keys; all bone tracks in a scene share a 65536-key ceiling. Translation and
rotation have absolute limits of 1000; each local scale is 0.1–4. The original rig
has only four levels, so these conservative bounds also constrain parent-composed
transforms well inside the generic rig's world limits. Invalid edits leave source
unchanged. Runtime hierarchy evaluation still validates the complete result.

Mac visual acceptance, sprite textures/deformation, arbitrary rig authoring and
artist clip-library authoring remain separate open work.
