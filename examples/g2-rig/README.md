# G2 rig fixture: two stick characters and a phone

`two-sticks-phone.json` is the KS-003a test scene for A08 (KS-003c): Veikko and
Kipinä from `examples/stick-studio/characters/`, the `puhelin` catalog item in
Veikko's right hand, a right-arm IK reach (at least one target out of reach), a walk across
the stage and a wide/close camera pair.

It is test data, not a `.kilsat` project. `crates/motion-rig/tests/g2_rig.rs` reads
it and checks that the phone stays within `tolerance_px` of the hand's grip point in
every pose, walk step and camera. The contract is in
[docs/rig-attachment-ik.md](../../docs/rig-attachment-ik.md).
