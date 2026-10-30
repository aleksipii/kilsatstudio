# Actor transform keyframes

Each scene may store `composition.actor_tracks`, a mapping from a character ID to
1–4096 strictly increasing keyframes. Existing screenplays default to no tracks.
Tracks add root translation to the position evaluated from Move events; they do
not replace movement or change its timing. Rotation (radians) and independent
positive X/Y scale apply around the actor's root, including its skeleton, face,
and artwork.

```yaml
composition:
  actor_tracks:
    EMMA:
      - {at: 0, x: 0, y: 0, rotation: 0, scale_x: 1, scale_y: 1, easing: smoothstep}
      - {at: 2, x: 100, y: -20, rotation: 0.2, scale_x: 1.2, scale_y: 1.2, easing: hold}
      - {at: 4, x: 150}
```

Unspecified translation/rotation values default to zero, scales to one, and easing
to `linear`. The easing on a key controls the interval to the next key:
`linear`, `smoothstep` (3t²−2t³), `hold` (keep the value until the next key),
`ease_in` (t³), `ease_out` (1−(1−t)³), `ease_in_out` (cubic in, cubic out), or
`back(<overshoot>)`, an ease-out that passes the next key and settles back onto
it. The overshoot must be finite and within 0–10; plain `back` uses 1.70158
(about 10 %). The same curves apply to bone and clip keys. Scene and character
`timing` (on twos/threes, stepped blocking) is described in
[MotionScript timing](motionscript.md#timing-exposure-and-blocking).
Rotations interpolate by the shortest angular route. Before/after the track's
endpoints, the first/last value is held. Sampling uses the scene's seconds clock,
so seeking, preview and exports evaluate the same pose without accumulating state.

Times must be finite, nonnegative and at most the scene duration. Transform
components must be finite and have absolute value at most 1,000,000; scale must
also be positive. Duplicate times, empty stored tracks and unknown characters are
rejected. Shortening a scene past a key is rejected. Removing an unused character
removes its track; event references still prevent removing a used character.

`motion_script::apply_actor_track(source, scene_index, actor, keys)` validates the
complete candidate atomically and returns normalized YAML. An empty key slice
removes that actor's track. Callers keep source/history unchanged on failure.
The shared `motion_animation::Track` implements all keyframe interpolation;
`ActorPose::local_to_world` transforms local geometry through its sampled root.
