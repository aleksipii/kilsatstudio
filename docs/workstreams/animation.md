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

This original library foundation is now integrated with persisted original-rig
per-bone editing and shared preview/export. The current status and next bounded
work package are recorded below. Custom/textured rigs and joint masks remain open.
No Mac/device acceptance is implied by these unit tests.

Run the two-joint sampling/blending example with:

```sh
cargo run -p motion-animation --example blend_pose
```

It samples a root track at one second, retains the hand's rest transform,
then prints root/hand world positions at blend weights 0, 0.5 and 1.

## Current integrated state (2026-10-18, through PR #38)

- Original seven-bone authored offsets, per-bone native editing and shared
  preview/export evaluation are implemented. Static and animated procedural
  Posture/Walk/Gesture strengths run before authored offsets.
- PR #37 added `Script.animation_clips: BTreeMap<String, AnimationClip>` with
  duration and bone tracks; capture, confirmed replacement/deletion and copying
  into target tracks are implemented.
- PR #38 maps **copied** keys into an explicit start/duration. Copies replace
  all target bone tracks and remain independent after library replacement or
  deletion. Track endpoint holding continues outside the copied range. This is
  neither a live reference nor an activation window.
- `motion-scene::evaluate` and `ValidatedScript::evaluate` share evaluation.
  Authored offsets enter `evaluate_humanoid_with_weights_and_offsets`, then root
  tracks apply; `apply_speech` subsequently adds mouth-driven head motion.
- `studio-desktop/src/clip_library.rs` returns detached actions to existing
  source history. `.kilsat` persists the MotionScript source. See
  [current controls](../animation-clip-library.md).

## First live-instance package (PR #40, integrated 2026-10-18)

The following bounded contract is integrated and passed Linux/macOS CI. It does
not establish physical motion acceptance. It supports constant-weight **ordered override blending of local authored
offsets** for the original seven bones first. Defer additive arbitrary-pose mixing,
normalized mixing, loops, fades, animated instance weights, clip events, masks,
custom rigs and cross-project exchange.

### Schema, identity and time

Keep existing library map keys as stable clip IDs (the existing 1–64 ASCII
identifier rule). Do not reference mutable display names or list indices. The
first package offers no clip-ID rename; later labels can be independent.
Add strict `ClipInstance { id, clip, actor, at, duration, weight }` and
`Composition.clip_instances: Vec<ClipInstance>` with an empty serde default:

```yaml
composition:
  clip_instances:
    - {id: tilt_a, clip: head_tilt, actor: EMMA, at: 1, duration: 2, weight: 0.5}
```

Instance IDs use the same identifier rule and are unique within their scene.
Vector order is persisted layer order, bottom to top; IDs never determine order.
Duplicating a scene preserves its scene-local IDs; duplicating an instance needs
a fresh ID. Scene/actor identity and stale-document checks remain mandatory.

Validate existing clip/actor, finite nonnegative start, finite positive duration,
finite weight in `[0,1]`, finite `end = at + duration`, `end > at` and
`end <= scene.duration`. Reject precision collapse and unknown fields. Weight
zero does not bypass reference, time or resource validation.

An instance is active on the half-open window `[at, end)`. Adjacent windows are
unambiguous; at the exact scene endpoint an ending instance contributes nothing.
Map to clip time with `(scene_time - at) / duration * clip.duration`. Use the
existing validated scene-local clock. Clip tracks hold first/last keys inside
their activation window if keys do not span the duration; outside it there is
no contribution. Validate numerical mapping; do not silently trim or extrapolate.

### Explicit first blend semantics

1. Sample ordinary scene bone tracks as authored local-offset transforms,
   using identity offsets for absent bones.
2. Visit active matching instances in source order. On bones present in each
   clip, interpolate the accumulated offset toward its sampled offset by weight.
   Absent clip bones preserve accumulated offsets. Weight zero preserves the
   previous value exactly, one replaces it exactly. Translation/scale interpolate
   linearly, rotation uses the existing shortest-arc rule and half-turn tie.
   Weights are not normalized; two half-weight clips are order-dependent.
3. Pass the combined offsets once into the existing humanoid evaluator:
   procedural posture/walk/gesture first, then combined authored offsets, root
   tracks and the existing mouth/head speech stage. Facial metadata is unchanged.

Blend **offsets with offsets**, not world joint points or complete procedural
poses. `blend_pose` validates complete rig-local poses; feeding offsets directly
into it would use incorrect rest semantics. Add a checked offset interpolation
helper in `motion-animation`, reusing existing transform interpolation. Preserve
empty-list output exactly. Handle equivalent rotation representations within
`valid_humanoid_offset` bounds with tests, rather than clamping. Propagate typed
errors instead of extending existing `expect` assumptions to unchecked data.

### References, editing and persistence

- Explicit library replacement warns with affected scene/instance counts.
  Validate the complete candidate. Instance duration stays fixed; changing clip
  duration changes its sample mapping. Previously copied tracks remain unchanged.
- Reject deletion of a referenced definition and identify referencing scenes
  and instances. Users must remove instances first. Never silently cascade,
  bake or retarget. Unreferenced deletion retains current behavior.
- Character deletion removes its instances in the same source transaction;
  scene deletion removes its instances. Scene duplication shares definitions.
- Add/update/remove/reorder by scene-local instance ID through atomic source
  edits. Each Apply is one existing Undo/Redo transaction. Invalid edits preserve
  source/history/preview. Native drafts reject stale source/scene/actor and busy
  state. Reordering is explicit; list filtering must not change layer order.
- Persist in source inside `.kilsat`; no new independent project format or
  editor playback model. Keep YAML-normalization/comment-removal warnings.

### Bounded preparation and interfaces

Enforced ceilings: 128 instances per scene, 32 per actor per scene and 1024 per
project, counting inactive/zero-weight instances. Retain current 64 clips,
600-second clip duration, 4096 keys per bone, 65,536 aggregate library keys,
original finite offset bounds, one-MiB source-edit limits and 100-KB native source
limit. Boundary tests cover these ceilings; they are protective limits, not measured capacity.

Prepare library tracks once per immutable `ValidatedScript` and share by clip ID.
Do not copy keys per instance/frame. The seven-transform accumulator and at most
32 active instances bound per-actor work. Sampling must use bounded lookup/search
without per-instance track allocations. The current evaluator reconstructs Track
vectors per sample; amplifying that pattern for every instance is unacceptable.
Preserve public `ValidatedScript::new`, `Clone`, `evaluate` and direct `evaluate`.
Prepared state is rebuildable runtime data, invalidated on accepted source change.

| Owner | Proposed interface/change |
| --- | --- |
| `motion-script` | Strict `ClipInstance`; atomic `upsert_clip_instance`, `remove_clip_instance`, `reorder_clip_instance`; reference/budget validation and deletion guards |
| `motion-animation` | Checked local-offset interpolation using existing transform rules, without script dependencies |
| `motion-scene` | Prepared shared tracks and ordered instance sampling inside the existing evaluator; typed failures |
| `studio-desktop` | Separate **Add live instance** action and detached instance controls; retain explicit existing copy semantics |
| `motion-export` | Shared-evaluator regression fixtures; no separate sampler |

No per-frame jobs, model loading, encoding or GPU work is added to the UI loop.

### Regression scope and remaining physical acceptance

- Schema defaults, unknown fields, duplicate IDs, unknown references, all numeric
  invalidities/precision collapse and every budget boundary.
- Independently expected offsets at weights 0/0.5/1; overlapping same-bone clips
  in both orders; missing bones, authored bases, scale and shortest rotations,
  half-turn ties, extreme valid angles and exact empty-list compatibility.
- Before/start/interior/end/after, adjacent windows, endpoint key holding inside
  windows, compression and repeated/backwards seeks at audio-sample boundaries.
- Referenced replacement/duration changes, guarded deletion, actor/scene cleanup,
  duplication and unchanged copied tracks; invalid coupled edits remain atomic.
- Native invalid/stale/busy drafts, reset confirmation on edits, one-step source
  Undo/Redo and `.kilsat` round trips preserving instance order.
- Actual SVG/PNG output against independently calculated authored-track fixtures;
  shared sample-clock preview with root transforms, procedural strengths,
  speech head motion and sequence transitions. Two wrappers of one sampler are
  insufficient independent expected evidence.
- Workspace fmt/check/Clippy/tests, Linux/macOS CI and prepared-sampling allocation
  inspection. Physical/perceptual motion acceptance remains separately open.

Hard activation edges may pop: describe this limitation in the UI. Ordered
override layers are not additive or normalized crossfades. Live library edits
affect many scenes and require clear affected-reference warnings and Undo tests.
Animated weights/fades, joint masks and looping require later explicit semantics.


PR #40 integrated this first package. See [the user guide](../live-clip-blending.md)
and [master progress](../master-progress.md) for exact checks and remaining scope.
Perceptual motion acceptance, animated weights/fades, loops, masks, custom rigs
and cross-project exchange remain open. Runtime tracks are prepared in immutable
snapshots; preflight and snapshots may prepare separately, while production frames
reuse the retained snapshot. Export work limits count all live instances, including
inactive and zero-weight rows.


## Live weight envelopes (PR #41, integrated 2026-10-18)

The integrated bounded extension adds `fade_in`, `fade_out` and local
`weight_keys: [{time, weight, easing}]` to live instances, defaulting to zero,
zero and empty. Effective weight multiplies the static value by an endpoint-held
key curve and linear entrance/exit factors. Keys use instance-local seconds;
fade durations cannot overlap, and keys remain inside the instance duration.
No looping, implicit retiming or normalized mixing is introduced.

Scalar sampling uses binary search without key copies or per-sample allocations;
the surrounding evaluator retains its existing bounded pose allocations. Schema
limits all keys, including inactive/zero layers: 256 per instance, 8192 per scene
and 65536 per project. Export work accounting includes curve searches/fades.
Native drafts preserve invalid values and apply through existing source history.
See [live layer controls](../live-clip-blending.md). Final Linux/macOS formatting,
check, strict Clippy and tests passed: 420/419 default Rust tests, 14 adapter-free
GPU tests and 55+29 Python tests per platform, plus three actual encoder tests.
Exact source/run/artifact evidence is in [master progress](../master-progress.md).
Hardware/perceptual acceptance stays open. Earlier deferred weight/fade entries
above describe the historical first-package boundary; PR #41 completes that
bounded extension. Loops, joint masks, custom rigs and general mixing remain open.
