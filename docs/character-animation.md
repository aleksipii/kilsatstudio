# Character animation foundation

The shared scene evaluator now produces seven skeletal joint positions per actor.
Native preview and SVG frames consume these same positions. Scene translation,
expression and mouth timing retain their existing clocks. The original procedural
humanoid has a hip, chest, head, two hands and two feet; its hierarchy drives a
walk cycle during active movement and a wave during the existing gesture event.
Seeking reevaluates the pose directly from scene time, without accumulated state.

`motion-rig` implements local translation, rotation and nonuniform scale, affine
composition, parent-before-child hierarchy validation and world evaluation. Rigs
are limited to 256 joints; invalid references, duplicate names, nonfinite values
and unbounded composed transforms are rejected. No recursive traversal is used.

`motion-animation` implements validated transform keyframe tracks with linear,
smoothstep and hold interpolation and shortest-path angular interpolation. Tracks
clamp outside their key range; timestamps must be finite, nonnegative and strictly
increasing. A track is limited to 4096 keys. These are library APIs, not yet a
user-editable screenplay format or a keyframe editor.

## Completion and remaining work

- [x] Transform hierarchy and deterministic evaluation.
- [x] Original procedural humanoid used in shared scene evaluation and SVG output.
- [x] Walk and wave presets using the existing Move/Gesture scene events.
- [x] Transform keyframe interpolation API and numerical validation tests.
- [ ] Persistent custom rigs and editable animation tracks in project files.
- [ ] Keyframe editor, joint inspector, inverse kinematics and animation blending.
- [ ] Licensed textured body parts, deformation and replaceable character assets.
- [ ] Mac visual acceptance and motion quality review.

This is a skeletal foundation, not a complete character animation milestone.
There are no external character assets, copied character systems or added model
requirements.
