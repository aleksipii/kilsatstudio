# Automatic estimated-result reuse workstream

Status: implementation plan based on repository inspection, 2026-10-18.
Automatic engine memoization is **not implemented or accepted**. This document
does not establish the contents, behavior or licenses of a real Rhubarb package.
No recognizer, model download or hardware test was run for this inspection.

## Current implementation

`motion-lipsync/src/estimated_cache.rs` stores immutable complete
`EstimatedVisemeAlignment` documents. Its result key includes cues, executable
hash, version, recognizer, exact decoded PCM identity and transcript hash. It is
a result archive, not an input-to-result index: the key cannot be computed before
the result exists. Loading a user-selected key requires no executable.

`alignment_worker::Action::Estimate` always calls `rhubarb::align`; `Load { key }`
validates the saved result against prepared audio and exact text. Estimate saves
after the editor and combined scene-map 100 KB limits pass. Save failure leaves
usable timing and an explicit warning. The panel requires explicit Apply and
rejects stale generation, line, path and transcript completions. Queue and update
channels each have capacity one; submissions are rejected while work is active.

The Rhubarb adapter bounds WAV input to 32 MiB and five minutes of mono 24 kHz
audio, executable bytes to 128 MiB, transcript to 64 KiB, result JSON to 4 MiB,
version output to 4 KiB/5 seconds and recognition to 120 seconds. It hashes the
executable before and after execution and snapshots the WAV. These are useful
execution bounds, not a complete environment fingerprint. File reads/hashes do
not currently offer chunk-level cancellation.

## Identity gaps that prohibit transparent reuse

| Input | Current evidence | Requirement before automatic hits |
| --- | --- | --- |
| Decoded audio and text | Exact PCM/rate/frame-count and exact text hashes | Preserve; never replace with path, duration or normalized text |
| Executable and version | Binary hash and executed version string | Bind executable to a supported package profile and prevent mutation during use |
| Recognizer and arguments | Enum plus fixed JSON/GHX/two-thread options in adapter | Version the complete output-affecting option/conversion contract |
| Acoustic/language/dictionary data | No complete resource discovery or hash manifest | Inventory actual resources for each supported recognizer/package, including embedded versus external assets |
| Shared libraries and loader paths | Not enumerated or constrained | Establish actual dependency closure and loading policy; hashing the executable alone is insufficient |
| Process environment and working directory | Commands inherit environment and working directory | Define controlled environment/cwd and identify permitted output-affecting values |
| Platform/runtime | No memoization identity | Include supported OS/runtime and architecture profile; invalidate when profile changes |

Do not assume PocketSphinx and Phonetic use the same resources or that either is
self-contained. A user-supplied digest string or a manually checked box cannot
create a `VerifiedEnvironment`. Resource names, loader behavior and redistribution
rights must be established for an actual pinned package. Filesystem hashing also
does not prove the process used those bytes: mutable symlink targets, library
search overrides and time-of-check/time-of-use changes need explicit handling.

## Next implementation package

Keep the explicit archive API backward compatible. Add a separate versioned
input index under `estimated-alignments/engine-index-v1`; do not scan the archive
for a newest or approximately matching result. Existing entries without an
execution-environment record remain available only through explicit Load.

Proposed ownership and interfaces (not existing APIs):

```rust
// motion-lipsync: constructors validate; callers cannot assert verification.
struct EngineRequestIdentity { /* exact PCM/text, recognizer, adapter profile */ }
struct VerifiedEnvironment { /* private fields; canonical manifest digest */ }
enum EnvironmentCheck { Eligible(VerifiedEnvironment), Ineligible(String) }
enum MemoLookup { Hit(EstimatedVisemeAlignment, MemoEvidence), Miss }
enum MemoError { Cancelled, Corrupt(String), Io(String), IdentityChanged }

fn inspect_environment(profile: &SupportedProfile, cancel: &dyn Fn() -> bool)
    -> Result<EnvironmentCheck, MemoError>;
fn lookup(identity: &EngineRequestIdentity, environment: &VerifiedEnvironment,
          cancel: &dyn Fn() -> bool) -> Result<MemoLookup, MemoError>;
fn publish(identity: &EngineRequestIdentity, environment: &VerifiedEnvironment,
           result: &EstimatedVisemeAlignment, cancel: &dyn Fn() -> bool)
    -> Result<(), MemoError>;
```

1. Implement canonical, domain-separated schema-v1 request identity and strict
   manifest serialization. Include exact audio/text, recognizer, full argument
   profile, converter revision, environment manifest and platform profile. Sort
   relative paths deterministically and reject traversal, duplicate paths,
   symlinks, special files and unknown schema. Do not include ephemeral staging
   paths in keys. Proposed manifest budget: 4,096 regular files, 4 KiB path names,
   512 MiB total hashed bytes, bounded 64 KiB read chunks and a 30-second deadline.
   Exceeding limits makes the profile ineligible, never partially verified.
2. Supply one explicitly supported, inspected package profile. Stage or otherwise
   constrain the complete resource/dependency closure used for execution, use a
   fixed working directory and controlled environment, reject loader injection
   variables, and verify identity again before publication and cache delivery.
   If supported OS system libraries cannot be pinned by file, define and justify
   an OS/runtime identity policy. Unknown executable/package profiles bypass
   automatic lookup and retain explicit execution/archive behavior with a reason.
3. Add a bounded index record (proposed 16 KiB maximum) mapping the full request
   identity to an immutable archive result key and original environment evidence.
   Validate both index and archived payload; check result executable/recognizer,
   PCM/text and provenance against the request. Use no-replace publication with
   operation-owned staging cleanup. Publish the result first, index second:
   interruption may leave an orphan archive entry but never a partial hit.
4. Split Rhubarb execution preparation from process execution so the same
   verified snapshot/options drive lookup and a miss execution. Avoid an
   unconditional `--version` subprocess on a hit: the verified profile supplies
   the expected version, while stored original version remains result provenance.
   A miss may execute only within the user's existing explicit Estimate action;
   line preparation alone does not gain recognizer execution authorization.
5. Integrate lookup into the current single bounded alignment worker. Preserve
   the manual Load path and explicit Apply, editor/map limits and stale checks.
   Distinguish `fresh`, `automatic reuse`, and `explicit archive load` in result
   status rather than stretching the current boolean `CacheInfo.hit`. Show why
   reuse was unavailable and retain original execution metadata. Do not relabel
   reused estimates as forced alignment or quality-approved output.

For `NotFound`, return Miss. Corruption, schema mismatch, dangling archive keys,
permissions and unexpected file types are visible recoverable errors, not silent
misses; never delete or overwrite unknown entries. An explicit retry can execute
fresh without trusting that index. An ineligible environment may execute through
the existing explicit action but must not publish an automatic-reuse index.
Conflicting results for the same identity must be reported and preserved, not
silently resolved by timestamp; assess nondeterminism before selecting a policy.

Cancellation checks are required before I/O, between bounded read chunks, before
execution, during existing process polling, before index publication and before
candidate delivery. A completed immutable orphan is harmless after cancellation;
no cancelled or stale candidate may Apply. Keep at most one worker request active
and do not start per-resource threads. Disk stalls cannot be promised to stop
within the cooperative deadline; document that limitation and use bounded local
storage rather than claiming hard real-time cancellation.

## Acceptance checklist

- [ ] Exact repeated request/profile produces a hit with zero process launches,
  including zero version invocations; first miss executes once and publishes.
- [ ] Same-duration changed PCM, changed text bytes, rate/frame count, recognizer,
  adapter/options, binary, model/resource, library, controlled environment,
  architecture or OS profile cannot hit the prior identity.
- [ ] External resource mutation with an unchanged binary is rejected; mutation
  during hashing/execution/publication and symlink/path swaps cannot create a hit.
- [ ] Unknown/incomplete package closure is ineligible. Tests cannot manufacture
  production verification through a public boolean or arbitrary digest field.
- [ ] Missing entry is Miss; truncated/tampered/oversized index or archive,
  unsupported schema, missing result, wrong provenance and permission errors are
  explicit errors; unrelated/corrupt entries remain untouched.
- [ ] Concurrent identical publication is idempotent; interrupted staging and
  conflicting output never expose a partial or silently replaced result.
- [ ] Cancellation during fingerprinting, lookup, execution and publication;
  full/busy queue; and stale scene/line/source/PCM/text completion preserve UI
  state. No I/O or hashing occurs during panel painting.
- [ ] Automatic hit still requires Apply, enters the same scene-map validation,
  and produces the same sample-clock mouth evaluation in preview and export.
- [ ] Synthetic stubs establish control flow only. A separately reviewed real
  offline package fixture establishes resource loading/profile compatibility;
  corpus quality and physical Mac acceptance remain separate gates.

## Actual blocker and delivery boundary

Input identity, bounded index storage and worker status can be implemented with
synthetic fixtures now. Enabling automatic hits for arbitrary configured Rhubarb
executables cannot be justified from this repository: there is no inspected,
pinned execution-resource/dependency closure or controlled process environment.
The next deliverable should therefore ship the index/control-flow tests with
production eligibility fail-closed, then enable one real package profile only
after its resource-loading and license evidence is reviewed. A synthetic profile
must remain test-only and must not complete the master-progress automatic reuse
checkbox.

See [saved estimated timing](../estimated-cue-cache.md),
[Rhubarb execution](../rhubarb-estimated-cues.md) and
[alignment workstream](alignment.md).
