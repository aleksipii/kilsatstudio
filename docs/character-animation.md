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

## Rule-based animation layers

The built-in humanoid is driven by a small keyframe clip library
(`motion_animation::evaluate_humanoid`) instead of hand-written pose math. Every
clip is an additive translation offset over the rest pose, and every layer is
sampled from scene time, so seeking never replays earlier frames:

1. **Posture** from the current emotion (happy upright, sad drooping, surprised
   hands raised, angry hands in). A change blends from the previous posture over
   `POSTURE_BLEND_SECONDS` (0.25 s) with `blend_pose`.
2. **Walk** cycle (`WALK_CYCLE_SECONDS`, 2/3 s) while a move is in progress.
3. **Gesture**: `wave`, `point`, `nod` or `shrug`. The clip runs over normalized
   event progress and is weighted by a sine in/out envelope, reported as
   `ActorPose::wave`. When gestures overlap, the strongest envelope wins.
4. **Speech**: `motion_scene::apply_speech` dips the head by mouth openness for
   the same per-actor visemes the mouth renders. Desktop preview and export call
   it with the visemes from their one audio sample clock; no alignment data means
   no motion. This is a rule, not a perceptually validated lip-sync measure.

Unknown gesture names are rejected as unsupported by MotionScript validation.

## Completion and remaining work

- [x] Transform hierarchy and deterministic evaluation.
- [x] Original procedural humanoid used in shared scene evaluation and SVG output.
- [x] Walk and wave presets using the existing Move/Gesture scene events.
- [x] Keyframe clip library: posture blending, walk, wave/point/nod/shrug and speech head motion in shared evaluation.
- [x] Transform keyframe interpolation API and numerical validation tests.
- [ ] Persistent custom rigs and editable animation tracks in project files.
- [ ] Keyframe editor, joint inspector, inverse kinematics and animation blending.
- [ ] Licensed textured body parts, deformation and replaceable character assets.
- [ ] Mac visual acceptance and motion quality review.

This is a skeletal foundation, not a complete character animation milestone.
There are no external character assets, copied character systems or added model
requirements.
