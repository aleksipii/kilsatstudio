# Final license audit workstream

Status: open, evidence collection only. Owner: final-license-audit agent.
Repository inspected on 2026-10-18 at commit
`e2befcd0365a19d9cc7ae736532bc0f995e6b8b9`.

## Evidence established here

- [x] Read repository instructions, root LICENSE/workspace declarations,
  desktop/render dependency features, preview workflow, packager, inventory
  collector and retained-install evidence implementation/documentation.
- [x] Confirm the original project source declares MIT, copyright 2026 Kilsat
  Studio contributors. The preview packager copies the root license both into
  `Contents/Resources/LICENSE` and beside the app. This covers original source;
  it does not replace third-party notices or resolve third-party obligations.
- [x] Execute the existing offline license-inventory suite: **16 tests passed**.
- [x] Execute the existing collector with `--strict`: JSON was produced and exit
  **1** correctly retained nine unresolved items and
  `distribution_approved: false`. Current lockfile inventory is 423 packages,
  SHA256 `f4f12a581319301f3f1caa358b0874e7db109f7cfe2dd8f623c8ec20e8f94a8a`.
  These are lock entries, including build/other-platform packages, not 423
  proven shipped components. The older hash in dependency-audit.md is historical.
- [ ] Inspect an actual selected release ZIP, target dependency graph, installed
  Mac environment, upstream artifact license corpus and legal obligations.

No model, wheel, voice, external binary or release ZIP was downloaded, executed
or cleared in this workstream. No external license assertion was added: source
references below are repository implementation evidence.

## Shipping boundaries and unresolved evidence

The current boundary is defined by [package_macos.py](../../scripts/package_macos.py)
and [macOS preview CI](../../.github/workflows/macos-preview.yml), rather than
everything in Cargo.lock or a developer machine. CI builds an arm64 debug desktop
and a separate GPU fixture helper, strips/signs the selected files and creates an
ad-hoc-signed preview ZIP. Signing/notarization and license review are separate.

| Boundary/component | Evidence available | Evidence/obligations still to resolve |
| --- | --- | --- |
| Desktop executable | `Contents/MacOS/studio-desktop`; CI builds `studio-desktop` for `aarch64-apple-darwin`; eframe enables default fonts and glow; macOS adds rodio | Exact resolved target/features/build graph, embedded fonts/assets, transitive source licenses and copyright/NOTICE texts, final signed executable identity and native linkage. A declared SPDX alternative needs an explicit reviewed choice. |
| Optional GPU helper | `Contents/Resources/bin/gpu_probe`; CI separately enables `motion-render/gpu`, with wgpu Metal/WGSL | Inventory this executable separately; the desktop graph cannot cover its different enabled dependencies. Retain its own build inputs, linked libraries and notices. |
| Packaged resources | Original LICENSE, top-level example YAMLs, generated icon/plist/build info, runtime/setup/doctor/smoke/acceptance Python scripts, setup manifest, requirements, runtime README and practical-testing guide | Match exact paths and final bytes; identify any embedded or copied third-party data and attach applicable notices. Python scripts are shipped source; their import dependencies are not included by this packager. |
| Host Python and consented local environment | Packager explicitly excludes Python and installed dependencies; setup uses host Python 3.10–3.12 and pip-resolved wheels | Separate local-install boundary. Capture selected interpreter provenance, ABI/platform and complete transitive wheel filenames/hashes/URLs; retained installed files do not identify original wheels. Do not classify host or local components as bundled solely because used. |
| Python packages/native contents | Direct pins Kokoro 0.9.4, Misaki English 0.9.4, Torch 2.6.0, NumPy 1.26.4, spaCy 3.7.5; optional retained distribution metadata and bounded license text | Review exact installed/transitive artifacts, embedded native libraries, fonts/dictionaries/data and corresponding copyright/NOTICE/modification/source duties if applicable. Native phonemizer/eSpeak loader relationships require selected-environment evidence; do not infer license or packaging from the top-level Python declaration. |
| Model/config/voices | Manifest pins model revision `f3ff3571791e39611d31c381e3a41a3af07b4987`, full hashes/sizes and af_heart/am_michael tensors | Manifest identity is not proof of inspected bytes or rights. Collect exact-revision model/config/voice license and notices; resolve voice provenance and applicable redistribution permissions separately. These assets are excluded from preview ZIP and downloaded only through consented setup. |
| English language wheel | Manifest pins revision `8fb6f8155286360bf5486fe399e2738d1df7ded2`, spaCy English 3.7.1 filename/hash/size | Inspect exact wheel, language-data license/attribution and native/transitive content; keep the manifest URL filename normalization distinct from actual artifact identity. Excluded from preview ZIP. |
| FFmpeg and other selected executables | Export requires a selected absolute FFmpeg executable; inventory can hash supplied binaries without execution. Packager does not copy FFmpeg/Rhubarb/eSpeak | Exact binary origin, build configuration, linked libraries and license/source/notice duties remain unknown. Record external versus bundled explicitly. If a future build bundles one, review its exact build before changing boundary status; a binary hash alone gives no permission. |

[Retained-install evidence](../license-provenance.md) binds local freeze,
distribution metadata, collected license bytes and setup manifest to a profile.
It rejects tampering and leaves missing/over-budget text unresolved. It does not
authenticate upstream origin, capture every native dependency, establish wheel
hashes or turn collected text into reviewed obligations.

## Artifact-specific audit manifest

The next report must bind one concrete ZIP and avoid a repository-wide approval
flag. Proposed schema (not yet implemented):

```json
{
  "schema_version": 1,
  "distribution_approved": false,
  "artifact": {
    "filename": "Kilsat-Studio-Preview-arm64.zip",
    "sha256": "<actual final ZIP SHA256>",
    "size_bytes": 0,
    "commit": "<actual source commit>",
    "target": "aarch64-apple-darwin",
    "profile": "debug",
    "workflow_run_id": "<observed run>"
  },
  "files": [],
  "build_evidence": [],
  "components": [],
  "external_requirements": [],
  "unresolved": ["Third-party notices and obligations require review"]
}
```

For each ZIP file record normalized relative path, size, final-byte SHA256, mode
and classification (executable/resource/notice/signature/unknown). Executable
records link separate desktop/helper target and feature graph fingerprints,
compiler/Cargo identity and saved Mach-O dependency observations. Component
records bind exact source/version/checksum or wheel identity to the files or
build graph; retain license/NOTICE text fingerprints and release-relative notice
paths, selected license alternatives, provenance and a human review record.
Record applicable source offer/source archive/modification obligations and their
evidence, or an explicitly reviewed not-applicable rationale. Missing evidence
stays unknown, not an empty obligations list. External requirements carry a
boundary label, not a claim that they were packaged or reviewed.

The final ZIP hash belongs in an external sidecar: embedding the hash of the
containing ZIP inside that ZIP would create a self-reference. A pre-signing hash
must never be represented as the final signed-file hash.

## Next bounded implementation: final-package inventory validator

Add an offline `scripts/package_audit.py` plus
`tests/test_package_audit.py`; invoke it after signing and ZIP creation in preview
CI. This increment collects package facts and validates the defined boundary;
it cannot close the final license review gate.

1. Accept explicitly supplied final ZIP, expected commit/target/profile and
   optional existing build-evidence JSON; never install, download, run Cargo/pip,
   execute packaged binaries or import package code.
2. Stream hashes under explicit archive/member/count/report budgets. Reject
   traversal/absolute paths, backslash ambiguity, duplicate normalized paths,
   encrypted entries, symlinks/special files and decompression budget violations.
   Do not extract the archive. Emit deterministic sorted records.
3. Validate both LICENSE copies against the checkout's exact bytes, expected
   BUILD-INFO fields, executable paths and current resource allowlist; enumerate
   signing metadata explicitly. Unknown members fail boundary validation rather
   than silently broadening the shipped set. The optional helper is recorded as
   present or absent and requires its own supplied graph when graph validation
   is requested. Unknown entries remain visible in a failure report.
4. Write the external sidecar atomically, bind it to the exact input ZIP hash,
   retain `distribution_approved: false` and unresolved obligations. Distinguish
   invalid package/evidence (exit 2) from successfully collected but unresolved
   strict review (exit 1); ordinary collection success is not clearance.

Acceptance tests:

- [ ] Valid desktop-only and desktop-plus-helper fixtures produce deterministic
  final-byte records and distinct executable graphs; changing one byte changes
  both the member and ZIP identities.
- [ ] Missing/altered LICENSE copies, inconsistent build info, unexpected bundled
  model/wheel/FFmpeg/native files and missing helper graph fail explicitly.
- [ ] Traversal, duplicate names, symlinks, special files, oversized/expanding
  members and aggregate/count limits fail without extraction or execution.
- [ ] Missing notices, SPDX-only declarations, unknown native provenance and
  unreviewed obligations can never produce distribution approval or strict exit 0.
- [ ] Existing packaging and license-inventory suites remain green; macOS CI
  attaches the real ZIP and hash-bound audit sidecar from the same build.

## Release blockers

Actual target graphs, signed artifact audit and complete selected environment are
not available in this mapping. Original wheel hashes/transitive lock, exact
upstream license/NOTICE corpus, native-library/source obligations and pinned
model/voice/language rights remain unresolved. Human review must assess the
assembled boundary before any redistribution clearance claim. This document
creates the work plan; it does not claim the proposed validator is implemented.
