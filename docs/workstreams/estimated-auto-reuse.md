# Automatic estimated-result reuse workstream

Status: input-index foundation integrated in PR #43 after Linux/macOS CI,
2026-10-18. Production automatic engine memoization is **not enabled or accepted**. This document
does not establish the contents, behavior or licenses of a real Rhubarb package.
No recognizer, model download or hardware test was run for this inspection.

## Current implementation

`motion-lipsync/src/engine_reuse.rs` now supplies a separate input-index storage
API. Schema-v1 request identity covers exact decoded PCM (including sample rate,
channels and frame count), exact transcript bytes, recognizer, the fixed
JSON/GHX/two-thread/converter contract, and OS/architecture. PCM hashing checks
cancellation between 1,024-sample chunks. This identity is not environment
verification: a public request hash cannot authorize automatic reuse.

`VerifiedEnvironment` has private fields and no production constructor. Only
module-local test fixtures construct synthetic tokens. No supported real package
profile, resource discovery, dependency closure, controlled execution snapshot or
runtime-policy verification has been implemented. Production Estimate therefore
continues to execute explicitly and displays the reason automatic reuse is
unavailable. Worker and panel distinguish `Fresh` from `ExplicitArchiveLoad`;
they do not advertise synthetic index tests as production hits.

The storage API uses `estimated-alignments/engine-index-v1` when given the existing
archive root. Domain-separated request/environment keys select strict schema-v1
index records limited to 16 KiB. Lookup rechecks exact request inputs, schema,
identity, original executable/version provenance and the validated immutable
archive payload. Only a missing index is a miss; malformed records, oversized or
nonregular files, dangling archive keys and conflicting results are errors.
Publication writes the archive first, then uses an exclusively created owned
staging file and a no-replace hard link for the index. Identical concurrent
publication is idempotent; conflicting archives remain preserved and never
silently replace the index winner.

Cancellation is cooperative and checked around index I/O, hashing and publication.
A cancellation before the hard-link commit point removes owned staging while a
complete archive entry may remain. Cancellation after that commit point may leave
a complete index; it still prevents successful candidate delivery. This does not
promise rollback of committed immutable files or hard deadlines for disk stalls.
The existing archive's bounded 4 MiB reads/writes do not yet check cancellation
between chunks. Filesystem checks are not a complete symlink-swap or package
execution TOCTOU defense; no production verification or automatic hit is enabled
on their strength.

Six new synthetic index tests cover exact input/environment invalidation,
corruption and dangling archive errors, immutable conflicting outputs, concurrent
publication, cancellation during fingerprinting/before I/O and cancellation after
archive/staging creation. They establish storage control flow only; execution and
quality evidence for a real offline package remain separate. These tests are
passed in Linux/macOS CI; see [master progress](../master-progress.md) for exact
evidence.


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

Input identity, bounded index storage and explicit worker origin/bypass status are
integrated in PR #43 with synthetic fixtures after Linux/macOS CI. Enabling automatic hits for arbitrary configured Rhubarb
executables cannot be justified from this repository: there is no inspected,
pinned execution-resource/dependency closure or controlled process environment.
The next deliverable must connect lookup/miss execution/publication using one
real verified package profile, only after its resource-loading, controlled
execution and license evidence is reviewed. Until then production eligibility
remains fail-closed. A synthetic profile
must remain test-only and must not complete the master-progress automatic reuse
checkbox.

See [saved estimated timing](../estimated-cue-cache.md),
[Rhubarb execution](../rhubarb-estimated-cues.md) and
[alignment workstream](alignment.md).

## Explicit local package enrollment (implementation awaiting CI)

The new opt-in local profile is an explicit trust boundary: the operator asserts
that a selected Phonetic package is self-contained. This is not an externally
reviewed Rhubarb distribution, a proof of filesystem isolation, a license audit
or measured speech-quality approval. No real package was supplied or executed.
Unknown packages and PocketSphinx retain explicit Estimate and archive Load.

In the local alignment panel, select **Phonetic**, enter the exact expected
`--version` output and use **Trust, enroll and estimate package**. Enrollment
creates `kilsat-rhubarb-package-v1.json` alongside the executable; existing
manifests are never overwritten. Use a dedicated package directory, with the
archive/cache outside it. Review changed package contents before explicitly
removing an obsolete enrollment and enrolling again. A version mismatch fails;
no automatic index is published from that run.

The profile hashes all regular package files, rejects symlinks/special files,
and caps inspection at 4,096 entries, 512 MiB, 4 KiB paths and cooperative
30-second inspection intervals with 64 KiB read/copy chunks. Linux accepts static
little-endian ELF64 executables only. macOS accepts host arm64/x86_64 Mach-O64,
including supported fat headers, with absolute Apple system dylibs only;
`@rpath`, local/non-system dylibs, loader environment commands and scripts are
ineligible. The macOS runtime identity hashes SystemVersion.plist; this is an
explicit Apple OS runtime policy, not a hash of every transitive system library.
No actual native package compatibility or physical Mac approval is inferred.

Normal Estimate performs exact input-index lookup after package inspection. A
hit launches no recognizer or version process. A miss executes an owned bounded
byte-for-byte package snapshot with fixed cwd, cleared environment and fixed
locale/PATH/HOME; it checks original package and snapshot identities around
execution/publication/delivery. The snapshot is not an OS filesystem sandbox:
a trusted executable can still use absolute files, network or other OS APIs.
Package completeness is the operator's explicit local trust assertion; it is
not established by hashing. Malicious same-user filesystem races and stalled
filesystem calls are not promised to be contained by cooperative checks.

The bounded desktop worker distinguishes fresh, automatically reused and
explicitly loaded timing. Every candidate still passes exact prepared PCM/text,
individual and scene-map limits, stale-generation protection and explicit Apply.
Corrupt/dangling/conflicting entries remain visible errors and are preserved.
**Run fresh (bypass reuse)** bypasses automatic lookup and publication while
retaining controlled execution for an enrolled package. Cancellation prevents
candidate delivery; complete immutable orphan archive/index entries may remain.

New synthetic tests cover explicit enrollment, resource mutation with unchanged
binary, undeclared files, symlinks, wrappers, cancellation, malformed manifests,
Mach-O loader policy and a real desktop worker exact-hit path with a deliberately
non-executable native identity fixture and absent WAV path. These fixtures prove
control flow only. Real package closure/compatibility, first-miss recognizer
behavior on a supplied package, real-audio quality and physical Mac testing
remain separate acceptance work.
