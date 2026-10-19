# Character and animation: next implementation handoff

Planning-only owner: `character_animation_owner`. Inspected 2026-10-18 on
the integrated PR #43 source baseline. This document changes no production behavior and
marks no acceptance gate complete. The parent owns integration and shared-file
scheduling.

## Delivery update: PR #45

The first package below is implemented and integrated in PR #45 (2026-10-19),
including the checked additive helper, compatible schema, shared evaluator,
native mode control, history/persistence, conservative work limits and independent
SVG/PNG/sample-clock preview tests. Final Linux/macOS CI passed; exact evidence
is in [master progress](../master-progress.md). Physical Mac motion quality remains
open. The original inspection and contract below are retained as the baseline,
not a claim that additive mode is still unimplemented. Package 2 is the next
animation implementation boundary; broader rigs and product gates remain open.

## Inspected implementation

- `motion-rig/src/lib.rs` supplies a render-independent, validated 1–256 joint
  hierarchy, unique names, parent-before-child order, local transforms, bounded
  world evaluation and rest poses. It has no serialized character asset format,
  texture attachments, mesh/UV data or deformation system.
- `motion-animation/src/lib.rs` supplies deterministic transform tracks,
  shortest-arc rotations, complete-pose `PoseTrack`/`blend_pose`, and the checked
  original-rig `blend_humanoid_offset`. Complete local poses and authored offsets
  have different rest semantics and must remain distinct.
- `motion-animation/src/clips.rs` implements an original seven-joint procedural
  humanoid: hip, chest, head, hands and feet. Emotion posture, walking and four
  gestures precede authored parent-local offsets. There is no expressive idle
  cycle. Static/animated posture, walk and gesture strengths are already usable.
- `motion-script/src/lib.rs` persists named project-local `AnimationClip`
  definitions and ordered `ClipInstance` references. Capture, explicit overwrite,
  copied application/retiming, reference-protected deletion and atomic source
  edits exist. Clips contain original-rig offsets, not root, speech or face data.
- `motion-scene/src/lib.rs` prepares only referenced clip tracks in shared
  immutable `ValidatedScript` snapshots. Its accumulator is seven slots;
  `ActorPose.skeleton` is `[Vec2; 7]`. Active matching rows interpolate accumulated
  offsets in source order. Output then enters procedural/hierarchy evaluation,
  root placement and the separate speech stage. This is a real restriction on
  arbitrary characters, despite the generic low-level rig API.
- Current live layers have half-open windows, stretch-once or original-speed
  modulo loops, constant/keyed scalar weights, linear entrance/exit fades and
  explicit original-joint masks. Empty masks mean all authored joints; masks do
  not expand descendants. Missing joints retain earlier values. Fades/key curves
  use instance time and continue across loop resets. Override weights are neither
  additive nor normalized. Loop endpoints can visibly jump.
- `apps/studio-desktop/src/live_clip_editor.rs` and `clip_library.rs` expose
  detached explicit Apply actions using source history, context guards and saved
  projects. Export `live_clip_frame_work` conservatively includes inactive and
  zero-weight rows; SVG/PNG/sample-clock preview fixtures live in
  `motion-export/src/live_clip_tests.rs` and `clip_library_tests.rs`.

The master tracker records PR #43 integration and CI evidence. This inspection
ran no Rust checks, renderer, GUI, model, GPU or physical Mac acceptance.

## First package: explicit additive original-rig live offsets

Implement one useful end-to-end feature before opening a custom-character asset
architecture: an opt-in additive live-layer mode. Artists can add a hand motion
on top of an existing reach without progressively replacing the reach. This uses
the current prepared clips, editor and shared evaluator; it needs no new crate,
dependency, asset or download. Do not call this general arbitrary-pose mixing.

### Contract to agree before edits

Add strict serializable `ClipBlendMode { Override, Additive }` in `motion-script`
and defaulted `ClipInstance.blend_mode`. Persist snake-case values `override` and
`additive`; omitted means override with exactly the old behavior. Unknown modes
fail parsing. Keep current IDs, reference checks, envelopes, timing, loop mapping,
mask semantics and all count/key limits. Rust struct literal consumers must be
updated explicitly; defaulting only solves source-file compatibility.

Add `motion_animation::add_humanoid_offset(base, delta, weight) ->
Result<Transform2, Error>` alongside the existing checked interpolation helper.
It owns numeric semantics and depends on no script type:

- Validate both offsets and finite weight in `[0,1]`, including at weight zero.
- At zero return the original base exactly. Translation is
  `base.translation + weight * delta.translation`.
- Rotation is `base.rotation + weight * delta.rotation` in authored radians;
  additive rotation expresses a signed delta, not shortest-arc override motion.
  Do not wrap/clamp to hide overflow; validate the result against existing bounds.
- Scale is componentwise `base.scale * (1 + weight * (delta.scale - 1))`.
  Identity offsets are neutral, and weight one composes the full authored scale.
- Reject invalid/out-of-bounds results with the existing typed animation error;
  never mutate input or publish a partial scene. Large valid individual layers
  can exceed combined bounds, which must produce an actionable evaluation error.

In `PreparedAnimation::offsets`, preserve saved row order and branch by mode only
at the per-joint combination step. Additive uses the current accumulated authored
offset or identity. Override continues to use `blend_humanoid_offset` unchanged.
Sampling occurs once per participating joint and effective weight is applied once.
An additive row followed by override can differ from the reverse order: document
this explicitly. Keep masks, missing joints and envelopes shared with override.
Do not add per-frame key copies or rebuild prepared snapshots during seeking.

### Files and sequence

1. `crates/motion-animation/src/lib.rs`: checked additive helper and independent
   numeric tests. No changes to generic `Rig` or `PoseTrack` are required.
2. `crates/motion-script/src/lib.rs`: enum/default field, strict round trips,
   atomic upsert tests and existing instance literals. Preserve old YAML output
   semantics; choose whether to omit default serialization consistently.
3. `crates/motion-scene/src/lib.rs`: mode dispatch, independent accumulated-offset
   expectations and existing instance literals. Public scene APIs stay stable.
4. `apps/studio-desktop/src/live_clip_editor.rs`: detached mode selector,
   mode-specific explanatory text/weight label, default override for new rows,
   reset preserving chosen mode, and context/history tests. Route through the
   existing Upsert action; no second persistence or playback mechanism.
5. `crates/motion-export/src/lib.rs`: account conservatively for additive
   arithmetic in work estimates, even for dormant layers. Update instance
   literals in export/project/desktop tests found with repository-wide `rg`.
   `crates/motion-export/src/live_clip_tests.rs`: independent rendered fixtures.
6. `docs/live-clip-blending.md`: explain signed additive deltas, scale formula,
   mixed-mode order, bounds and compatibility. The parent alone updates master
   progress after actual integration/check evidence.

### Required tests and acceptance

- Numeric independently calculated translation, signed rotation and scale at
  0/0.5/1; identity neutrality; finite/invalid weight and transform rejection;
  composition exceeding bounds; exact zero and legacy override endpoints.
- Old documents deserialize to override and retain existing evaluated output.
  Explicit modes round-trip; unknown spelling fails; failed edits leave source,
  Undo/Redo and selected context unchanged. Save/reopen preserves mode/order.
- Additive over a nonidentity authored base, additive/additive and both mixed
  orders; selected/missing joints; parent rotation propagation; sampled envelope
  applied once. Exercise before/start/interior/end, repeated/backwards seeks,
  original-speed loops and instance-local fades across cycle resets.
- Actual SVG and PNG frames compare against independently authored expected
  tracks, not a second call to the new helper. Sample-clock preview matches those
  frames with root/procedural/speech stages and a scene boundary. Include a
  visible translated joint, not only a rotated leaf point.
- Over-budget inactive/zero rows still fail preflight before output creation;
  prepared track sharing remains intact. No claim of measured throughput follows
  from a work estimate.
- Run repository-required workspace fmt/check/Clippy/tests and Linux/macOS CI
  when Rust is available. Record exact blocked checks otherwise. Physical Mac
  editing, smoothness and motion quality are separate evidence gates.

The package is accepted only when an artist can create, Apply, save/reopen and
Undo an additive instance, independently expected geometry/pixels pass, defaults
remain compatible and all applicable checks pass. A new enum/helper alone is
incomplete. Physical acceptance must stay open until actually exercised.

## Ordered remaining packages and dependencies

| Order | Concrete next outcome | Required boundary and acceptance |
| --- | --- | --- |
| 2 | Normalized original-rig override groups | Specify explicit group IDs and position in the ordered stack before schema edits. For each joint normalize only active, selected, present contributors; define base residual below total weight one, all-zero fallback and deterministic rotation reference/ties. Preserve ungrouped override/additive order. Independent order-invariance and mask/weight tests are mandatory; do not silently reinterpret existing rows. |
| 3 | Artist idle and reusable preset selection | Deliver one authored breathing/weight-shift cycle through current original-rig clip instances first, with explicit opt-in authoring and independently checked looping/envelope behavior. An automatic idle rule later needs movement/gesture priority, absolute phase/seek policy and a backward-compatible strength field. Do not silently animate old projects. |
| 4 | Versioned editable custom rig plus one drawable rigid part | Agree stable rig/joint/part IDs and character-definition asset ownership with project/render owners. Persist rest hierarchy and a single rigid part attachment, edit it, save/reopen and render it through the shared evaluator. Keep original characters compatible. Generic `Rig` is a validator, not the persisted asset schema. |
| 5 | Textured rigid character parts | Build on package 4: project-owned image IDs, pivot/local transform, joint binding, draw order and alpha; bounded decoding/resource resolution outside the event loop; shared CPU preview/export and explicit GPU support boundaries. Prove one multi-part character and missing-resource diagnostics. Existing foreground artwork/images do not establish this capability. |
| 6 | Deformation and custom-rig clips | Only after texture/rig identity exists: define bounded mesh topology, UVs, bind pose, per-vertex influences and normalized weights; CPU reference skinning and persistence before editor/GPU expansion. Clips must bind a declared rig identity/version and reject incompatible joints rather than guessing retargeting. Test zero/duplicate influences, invalid indices, bounds and independent deformed pixels. |
| 7 | Cross-project libraries | Versioned bounded exchange of clip definitions plus explicit rig identity; original-rig clips can be the first supported payload. Atomic import, duplicate-ID resolution, explicit overwrite/reference warnings and independent copies after import. Asset-bearing custom rigs require dependency closure, project asset IDs and provenance/license handling first. No automatic network library. |

Custom rig/texturing work is a cross-crate architecture gap; additive original-rig
clips are implementation-ready. They can proceed independently only while the
parent serializes shared `motion-script`, scene and desktop edits. Coordinate
renderer ownership for evaluated custom parts, project ownership for asset
persistence and import, timeline ownership for future general compiled tracks,
and QA ownership for physical Mac evidence. Do not create parallel samplers,
competing character schemas, empty crates or unvalidated compatibility layers.
