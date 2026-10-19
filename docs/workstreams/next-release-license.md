# Release and license remaining-work handoff

## PR #46 delivery and remaining scope

PR #46 implements the offline corpus/review-gap report described below and a
separate artifact-bound Mac observation report. See [report guide](../release-license-report.md)
and [master progress](../master-progress.md) for actual checks. Supplied evidence
is checked for integrity; final distribution/device approval is never granted
automatically. Actual upstream corpus, qualified review, fulfilled notices,
production signing and release authority remain open.

Owner: `release_license_owner`. Status: assigned, open; specification only.
Inspected 2026-10-18 on `docs/remaining-work-owners`, HEAD
`f433c902492d4c4978950e32262746859ca102ac`. This handoff implements no
collector, packaging change, clearance or release. Parent owns integration,
shared progress and acceptance marking. Preserve unrelated working-tree edits.

## Existing evidence and its limits

The root MIT license covers original Kilsat Studio source, copyright 2026 Kilsat
Studio contributors. `scripts/package_macos.py` copies its exact bytes to the ZIP
root and app resources. It neither relicenses third-party material nor replaces
third-party attribution, notices, source duties or asset rights review.

| Existing files/evidence | Established behavior | Remaining uncertainty |
| --- | --- | --- |
| `scripts/package_audit.py`, `tests/test_package_audit.py`, [guide](../final-package-audit.md) | Bounded offline final ZIP inventory; member hashes/modes; exact resource/build-info/license boundary; rejects unknown additions; observes CodeResources/AppleDouble; strict unresolved exit 1 | Signature metadata presence is not signature verification; hashes establish observed identity, not origin or permission. AppleDouble remains opaque/unreviewed. |
| `scripts/build_evidence.py`, `tests/test_build_evidence.py`, [guide](../build-evidence.md) | Binds ZIP/audit, separate desktop/helper Cargo closures, lock declarations, compiler/Cargo text and post-signing executable hashes to supplied native observations | Target-filtered metadata closure includes build/dev edges and feature unification; not exact compiler units or embedded component proof. Lock checksums are declarations. Tool output is caller-supplied. Both executables are required here although package audit permits no helper. |
| `scripts/license_inventory.py`, `tests/test_license_inventory.py`, [dependency audit](../dependency-audit.md) | Lock identity inventory, exact-source metadata joins, direct setup pins, optional installed metadata, selected external binary hashes, manifest asset pins; strict remains unresolved | Lock entries are not shipped components; declarations are not reviewed texts or SPDX choices; installed metadata/binary hashes do not prove original wheel/build provenance. |
| `services/kokoro-runtime/setup_kokoro.py`, `apps/studio-desktop/src/managed_license_evidence.rs`, [retained provenance](../license-provenance.md) | Profile-bound sanitized freeze, exact setup manifest and bounded collected license bytes, with hash validation and explicit missing/over-budget text | Local integrity is not upstream authentication, complete transitive wheel locking, native inventory or rights clearance. Legacy profiles cannot supply missing retained evidence. |
| `.github/workflows/macos-preview.yml`, `scripts/package_macos.py` | macOS-15 arm64 debug desktop/helper; strip, ad-hoc sign/verify, ZIP, audit/build sidecars and raw inputs | No production signing, notarization, physical-device acceptance or release-profile performance acceptance. CI artifact retention is only 14 days. |

[Master progress](../master-progress.md) records PR #43 source
`de1dbc446510e2a895b7138612d80a292814c015`, Mac run `39156985436`, artifact
`12147780773`, final preview ZIP SHA256
`331db8d6e9c873c17c3e63b640e930ff4cae1569fb9d947ba149bcc4de1a5be6`
and container SHA256
`42877ed0a8258ac459c7d59640aaec0448d50dc32817f303825993aa4b493be9`.
It records 21 accepted member records and successful build collection with
`distribution_approved: false`. These are existing tracker observations, not a
fresh artifact inspection by this owner. Historical package hashes and audit
counts in earlier sections must not be substituted for this or a future release.
Older documents saying graphs are deferred predate PR #43: graph observations
exist; exact compile/embedded proof and rights review remain open.

This turn read source, tests and documentation only. No test execution, asset
retrieval, packaged binary execution, signing, model run or legal conclusion is
claimed. The previous license workstream records 16 inventory tests; PR #43
records 59 repository Python tests per CI platform. They are historical results,
not validation of this specification or a new release.

## Shipping boundary to preserve and review

1. Define one candidate by full source commit, version, target, profile and chosen
   helper presence. Retain final ZIP, audit, build report and raw inputs together;
   verify their identity before review. Preserve their distinction from the
   GitHub artifact container and from unsigned/pre-strip executable hashes.
2. Inventory desktop and helper separately, including statically embedded Rust
   material, egui/eframe default font data and generated/copied resources. Collect
   exact upstream source/artifact license and NOTICE bytes for each candidate
   graph component; classify inclusion as proven, conservative candidate or
   unresolved. Do not silently treat every lock entry as bundled or remove a
   candidate merely because native dynamic linkage shows only system libraries.
3. Keep host Python and user-consented managed installs separate from bundled
   files. Current preview ships scripts/manifest/requirements, no interpreter,
   wheels, model, voice or English data. Capture selected interpreter origin,
   version/ABI/platform and exact transitive wheel filenames, bytes/hashes,
   source revisions/URLs and embedded native/data inventory when reviewing that
   installer boundary. Freeze and installed license files are supplemental.
4. For manifest model revision `f3ff3571791e39611d31c381e3a41a3af07b4987`,
   collect exact config/weights and `af_heart`/`am_michael` notices and provenance.
   For English wheel revision `8fb6f8155286360bf5486fe399e2738d1df7ded2`,
   collect exact wheel/data attribution. Pins do not prove inspected bytes or
   voice rights. Kokoro approval does not approve every dependency or asset.
5. Record FFmpeg, Rhubarb and eSpeak/native loader paths as external, installed or
   bundled only from selected-artifact evidence. Review native contents and
   loader/linkage relationships of the actual Python wheels. Do not infer terms
   from a top-level package declaration or a sidecar process boundary. Any future
   bundling requires a separately reviewed boundary and updated package allowlist.
6. Obtain human component review of exact texts, copyright/NOTICE, any selected
   license alternative, modifications and applicable redistribution/source duties.
   Record unknowns explicitly. No third-party SPDX choice or legal clearance is
   supplied by this handoff or inferred from older mutable upstream summaries.

## First implementable package: offline corpus and review-gap report

Implement this before production packaging changes. Own new
`scripts/release_license_report.py`, `tests/test_release_license_report.py`,
`docs/release-license-report.md` and synthetic fixtures under
`tests/fixtures/release_license/`. Keep existing collector interfaces stable;
coordinate any later changes to packager/auditor/workflow with parent first.
Do not add real upstream texts or assets until exact supplied inputs and their
publication suitability have been reviewed.

Proposed CLI: `python3 scripts/release_license_report.py --archive ZIP --audit
AUDIT --build-evidence BUILD --corpus-index INDEX --corpus-root DIRECTORY
--output REPORT [--strict]`. The implementation reads explicitly supplied local
files only: no subprocess, Cargo/pip invocation, network, archive extraction,
package import or executable launch. Input corpus is a dedicated supplied
staging directory, not a scan of developer home/caches.

Contract version 1:

- Revalidate the final package using trusted adjacent package-audit logic and
  exact commit/target/profile from the bound report. Check ZIP SHA256/size,
  audit/build input fingerprints and final executable member identities against
  the supplied ZIP. Require both existing build graphs for the first increment;
  report helper-free support as unsupported rather than inventing its graph.
  Reject stale, forged, duplicate or mixed-artifact references and unsupported
  schemas. Sidecar hashes alone are not independent compiler attestation.
- `corpus-index.json` identifies each component by a stable ID, boundary
  (`bundled`, `local-install`, `external`), exact name/version/source/revision,
  artifact digest when available, separate desktop/helper graph references and
  member paths/hashes when known. Every graph candidate needs a component record
  or an explicit unresolved mapping. Embedded font/data/native subcomponents
  receive their own IDs and parent relationship; no empty graph is success.
- Each evidence reference contains a canonical relative path, byte count,
  SHA256, kind (`license`, `notice`, `copyright`, `source`, `modification`,
  `provenance`) and exact source/artifact association. Hash the supplied bytes;
  reject traversal, symlink components, special files, duplicate IDs/references,
  ambiguous identities and outside-root paths. Missing upstream identity remains
  unresolved; local matching bytes do not authenticate the claimed publisher.
- Optional review records contain reviewer identity/date, reviewed component and
  evidence digests, verbatim declared expression separately from any reviewed
  alternative, rationale, and obligations. Each obligation has status
  `unreviewed`, `required`, `satisfied` or `reviewed-not-applicable`, supporting
  evidence references and rationale. Categories include notice/copyright,
  modification notice, source/archive/offer and asset redistribution/provenance.
  Missing categories stay unknown; an empty list is not a review. Reject dangling
  review references; changed texts invalidate their old review binding.
- Notice obligations name canonical future release-relative destination paths
  and exact source text digests. The first report verifies prepared notice bytes
  in the supplied corpus and reports `not-packaged` until a later audited ZIP
  actually contains matching notices. Source/archive/offer claims require supplied
  evidence plus reviewer rationale, rather than a boolean. This validates record
  completeness/integrity, never whether a human legal judgment is correct.
- Emit deterministic sorted JSON: schema/input identities, artifact, components,
  evidence, review records, obligations and actionable unresolved codes/paths.
  Always `distribution_approved: false`, with physical acceptance not granted.
  Exit 0 means valid collection; strict exit 1 means unresolved review/package
  evidence; exit 2 means invalid input or output failure. Even complete records
  do not authorize distribution; release authorization remains a separate human
  decision tied to exact artifact identity.
- Use bounded streaming reads: retain existing ZIP limits, at most 2048 graph
  components per graph, 4096 corpus files, 1 MiB per text, 32 MiB corpus total,
  16 MiB per JSON input/output. Fail explicitly on exceeded limits or truncated
  text. Use a new regular output path, reject input/output aliases, atomically
  publish through an owned temporary file, and clean it on failure. Do not
  overwrite existing evidence or follow output symlinks.

Required synthetic tests: determinism; separate graph mappings; changed ZIP,
member, audit, build/corpus bytes; identical names with different sources;
missing font/native/license/NOTICE/source evidence; unselected SPDX alternatives;
missing reviewer/rationale; stale review digest; empty obligation lists; prepared
notices absent from ZIP; explicit external versus bundled classification;
unknown component/subcomponent; invalid schema/shape; duplicates, symlinks,
hardlink aliases, traversal, special files and resource budgets; atomic-write
failure and CLI exits. Instrument subprocess/network/package import boundaries
to prove they are not used. No valid or malicious fixture may set distribution
approval true or physical acceptance granted.

Run the new suite plus existing `test_package_audit.py`, `test_build_evidence.py`,
`test_license_inventory.py` and `test_package_macos.py`. Document actual counts
and limitations. Use no inference or external binaries for these offline tests.
Only after fixture review should CI collect a supplied real corpus; a green
collection job must remain labeled evidence collection.

## Ordered follow-through after that increment

1. Acquire and preserve exact release candidate inputs and complete upstream
   corpus from authorized suppliers/maintainers. Fill mapping gaps, inspect
   font/data/native contents, and obtain qualified human rights/obligation review.
   Retain conservative candidates until exclusion has evidence. Produce a
   reviewable third-party notice bundle and source/offer materials where required.
2. Extend `package_macos.py`, `package_audit.py` and their tests together to admit
   only reviewed notice/source destinations with exact manifest-bound bytes.
   Repackage, then collect reports from the new final artifact; earlier ZIP
   hashes cannot cover added notices. Keep original MIT copies intact. Any new
   model/wheel/native payload is a new review scope, not a general allowlist hole.
3. Define production version/build identifier, application identity, minimum OS,
   supported architecture, helper inclusion, release profile, setup UX, update
   policy and release notes. Build with explicit `--release` and matching locked
   inputs; `package_macos.py --profile release` currently labels the artifact but
   does not prove the supplied binary was built in that profile. Retain actual
   build invocation/compiler inputs and run physical Mac acceptance separately.
4. Obtain an explicit Developer ID/notarization decision and authorized Apple
   team/signing identity, credentials and signing environment. If selected,
   implement nested signing order, reviewed entitlements/hardened runtime,
   timestamping, verification, notarization submission/log retention, stapling
   and verification, and Gatekeeper checks on a clean physical Mac. Do not put
   secrets in reports/repository. Ad-hoc verification cannot satisfy this gate.
   If declined, record the intended distribution channel and its accepted
   limitations; do not describe that artifact as Developer ID signed/notarized.
5. Finish all byte-changing stages including signing/stapling before final ZIP
   inventory and native observations. Stapling may change the package boundary;
   update audited expectations explicitly. Retain final identity, signing and
   notarization evidence, notices, source materials, review decision and device
   acceptance together. Obtain parent/release authority approval before publishing.

## Inputs and release criteria still blocked

External inputs required: retained actual candidate ZIP/raw CI inputs (not merely
tracker URLs); exact upstream source/wheel/license/NOTICE corpus and modification
history; selected Mac Python/ABI/platform and wheel/native provenance; exact
model/voice/language rights evidence; qualified reviewer identity and obligation
assessment; production channel/version/helper decision; physical Mac observations;
and, if selected, authorized Developer ID team/certificate and notarization
access. This owner cannot manufacture those from hashes, declarations or tests.

Release criteria are cumulative: exact artifact identity and full boundary mapped;
all required texts/notices/source materials packaged or otherwise fulfilled with
reviewed evidence; every unresolved rights/component gap resolved or release
scope narrowed and re-audited; human approval tied to final hashes; production
build/package checks and signing decision fulfilled; separate Mac/offline speech,
audio/GUI/project/export, Metal/performance and perceptual acceptance gates closed
by their owners. No collector success, CI green result, MIT declaration or signed
bundle alone closes these criteria. This remains an agent handoff, not a finished
release or redistribution approval.
