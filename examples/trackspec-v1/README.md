# TrackSpec v1 examples

Screenplays for `crates/motion-scene/tests/timeline_determinism.rs`.
Contract: [editable timeline § TrackSpec](../../docs/editable-timeline.md#trackspec-overlap-order-and-time-conversion-a04).
Resolved tracks are evaluated state and are not serialized, so the examples are
authored MotionScript.

| File | Expected result |
| --- | --- |
| `valid.yaml` | validates; EMMA's three touching moves hand over exact endpoints; direct seek equals frame-by-frame playback bit for bit |
| `invalid-overlap.yaml` | validation error `EMMA: overlapping move events in scene one` |
