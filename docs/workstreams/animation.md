# Joint animation foundation

Implemented in `motion-animation`:

- `PoseTrack` owns a validated `Rig` and one optional local transform `Track`
  per joint, in the rig's parent-before-child order. Missing tracks preserve rest
  transforms; supplied values replace local transforms rather than adding offsets.
- Sampling is deterministic and seekable, uses existing easing and shortest-arc
  rotation interpolation, and validates composed world transforms before returning.
- `blend_pose` interpolates complete local poses with a finite weight in `[0, 1]`,
  preserves exact endpoints, and validates both inputs and the output. Invalid
  input cannot mutate callers' poses.
- Existing rig bounds remain 256 joints; tracks retain their 4096-key maximum;
  the pose-track aggregate is capped at 65536 keys.
- Unit tests cover hierarchy propagation, seeking, rest fallback, blend endpoints,
  shortest rotation, invalid weights and shapes, aggregate budget, and world overflow.

This is a library API foundation, not a completed per-bone editing feature.
Persisted custom rigs, scene/document integration, per-bone editor controls,
textured rig parts, joint masks, and render/export integration remain **OPEN**.
The existing procedural humanoid and transform-track behavior are preserved.
No Mac/device acceptance is implied by these unit tests.

Run the two-joint sampling/blending example with:

```sh
cargo run -p motion-animation --example blend_pose
```

It samples a root track at one second, retains the hand's rest transform,
then prints root/hand world positions at blend weights 0, 0.5 and 1.
