# Rig attachment, joint limits, IK and hand props

Owner: `motion-rig` (A05, ORK-G1-05, KS-003a). Everything below is pure,
deterministic Rust in `crates/motion-rig`; it renders nothing and reads no clock.

## Attachment contract (`attachment.rs`)

Stick characters use one hierarchy:

| Joint | Parent | Role |
| --- | --- | --- |
| `root` | — | Pelvis; the hierarchy root (`PELVIS`) |
| `torso` | `root` | Chest; the shoulders are the arm chains' rest offsets on it (`TORSO`) |
| `upper_arm_*` → `forearm_*` → `hand_*` | `torso` | Arms (`arm_l`, `arm_r`) |
| `thigh_*` → `shin_*` → `foot_*` | `root` | Legs (`leg_l`, `leg_r`) |

`STICK_LIMBS` names the four two-bone chains. `PreparedRig::stick_limbs()`
resolves them and returns `AttachmentError` if a joint is missing, the pelvis is not
the root or a parent link is different.

A limb stays attached while its three moving joints (upper, lower, end) only rotate.
A translation or scale offset on one of them moves it off the end of its parent bone;
`PreparedRig::check_limb_offsets` rejects such offsets and
`PreparedRig::attachment_gap` measures the gap (0 when attached). The torso and head
are not limb joints, so the breathing and nodding `y` keys in the stick-studio clips
are allowed.

The built-in seven-joint humanoid (`motion-animation`) parents hands to the chest and
feet to the hip, and draws each limb as one line from that anchor, so it stays
attached at any length.

Tests: `crates/motion-rig/tests/g2_rig.rs` checks every shipped stick character and
every joint track in `examples/stick-studio/**/*.kilsat` (rig clips and per-scene
tracks, at every key time and the midpoints between them).
`crates/motion-animation/tests/limb_attachment.rs` checks the built-in humanoid's
hierarchy and that no emotion, walk or gesture combination stretches a limb past 2×
its rest length (the largest seen is about 1.76×: pointing while surprised and
walking).

## Joint limits (`definition.rs`)

`RigDefinition::joint_limits` maps a joint ID to `{min_rotation, max_rotation}`
(radians, local rotation = rest + offset). Validation requires finite, ordered
bounds within ±2π that contain the rest rotation. `PreparedRig` clamps the rotation
in every evaluation (`evaluate_parts`, `evaluate_acting`, `world_transforms`).
Non-finite input stays non-finite so pose validation still rejects it.

## Two-bone IK (`ik.rs`)

`solve_two_bone(root, upper, lower, target, bend)` uses the law of cosines.

- `BendDirection` picks the side of the root → target line for the elbow or knee
  (sign of `(target − root) × (middle − root)`; `Positive` bends clockwise in
  y-down screen space).
- A target past `upper + lower` gives a straight limb pointing at it. A target nearer
  than `|upper − lower|` gives a fully folded limb. A target on the root hangs the
  limb straight down. `reached` reports whether the end lands on the target.
- Non-finite or out-of-range points and lengths return `IkError`; every accepted
  input gives finite values, and the same input gives the same output.

`PreparedRig::solve_limb_ik(limb, target, bend, offsets)` measures the current bone
lengths from the posed rig, solves, and returns replacement rotation offsets for the
upper and lower joints (translation and scale of any input offset are kept). Joint
limits apply; `LimbIk::limited` says whether they changed the solution and
`LimbIk::end` is where the end joint actually lands. Rotations are wrapped to the
turn nearest the limit centre (or the rest rotation) so they never pile up whole
turns. The parent and upper joints must have uniform, unsheared scale.

The IK target is a value. Its written timeline form is an A02 schema request and its
resolution is A04's `motion-scene` work (ORK-G1-02, KS-003d).

## Grip contract and hand-prop catalog (`props.rs`)

The catalog (`examples/stick-studio/hand-props.json`, schema version 1) moved from
`apps/studio-desktop/src/hand_props.rs` into `motion_rig::props`:
`HandPropCatalog::decode` parses and validates it (≤128 KiB, 1–32 items, parts on a
local `grip` joint, bounded untextured vector shapes). The desktop module is now a
thin wrapper that maps `PropError` to Finnish editor messages.

- `item.grip` is the item-local point held in the hand.
- `GripPlacement {x, y, rotation_degrees, scale}` is the hand-local placement of that
  point (±100, ±180°, 0.25–3).
- `attach_hand_prop` copies the item's parts onto `hand_l`/`hand_r` with IDs
  `heldprop_{l,r}_<item>_<part>`, so `item.grip` lands at `(x, y)` in the hand
  joint's frame. It replaces an earlier catalog prop on that hand, refuses a hand that
  holds the character's own prop, and leaves the rig unchanged on error.
- `GripPlacement::grip_world(hand_world)` is the grip point in rig space.

Because the prop is ordinary rig parts on the hand joint, the normal evaluator moves
it with the hand. The actor's transform and the camera transform apply to the hand
and the prop the same way. `phone_stays_in_hand_through_ik_walk_and_camera_change`
checks this with `examples/g2-rig/two-sticks-phone.json`: every phone vertex stays
within 0.5 px of the hand-grip transform across six IK targets (at least one out of reach) plus the rest pose,
nine walk positions and a wide and a 2.5× close camera. The camera there is a plain
affine stand-in (centre, zoom, viewport). Real renderer output and reference images
are A08's (KS-003c).

## Not included

- No IK editor in the desktop UI, no IK timeline track (KS-003d) and no
  scene-level IK evaluation.
- No mesh deformation, SkelForm/PSD import or `rusty_skelform`.
- No rendering or graphical check; the style stays the existing stick figures.
