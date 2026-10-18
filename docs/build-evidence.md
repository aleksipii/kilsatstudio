# Build observations bound to a preview artifact

`scripts/build_evidence.py` extends the final-package inventory with saved build
observations. It reads explicit local inputs only: no subprocesses, dependency
installation, downloads, executable launch or model inference. Python 3.11 or
later is required (`tomllib`); preview CI uses Python 3.12.

The [macOS preview workflow](../.github/workflows/macos-preview.yml) now captures
these inputs and requests collection after the signed ZIP and package-audit
sidecar exist. Integration is pending CI verification; this document records no
new physical Mac or model run.

## Inputs and command

Use a checkout and final package from the same build. Save separate metadata
invocations: enabling the GPU helper changes resolved features. CI records:

```sh
mkdir -p dist/build-inputs
cargo metadata --locked --filter-platform aarch64-apple-darwin \
  --format-version 1 > dist/build-inputs/desktop-metadata.json
cargo metadata --locked --filter-platform aarch64-apple-darwin \
  --format-version 1 --features motion-render/gpu \
  > dist/build-inputs/helper-metadata.json
rustc -vV > dist/build-inputs/rustc.txt
cargo -V > dist/build-inputs/cargo.txt
cp Cargo.lock dist/build-inputs/Cargo.lock
```

These Cargo commands are capture steps, separate from the offline collector.
They resolve the checkout and can access configured registries if inputs are not
already available. The collector itself never invokes Cargo. For a previously
captured CI artifact, use its retained inputs without rerunning resolution.

After [package inventory](final-package-audit.md), CI reads only the two known
executable members into temporary files, hashes their final post-signing bytes,
checks them against the inventory and obtains `lipo -archs` / `otool -L` output.
It invokes the inspection tools, never either packaged executable. The final
observations must be captured after stripping/signing; earlier build-file hashes
cannot stand in for the final executable identities.

`native.json` is an object with exactly these two keys:

- `Kilsat Studio Preview.app/Contents/MacOS/studio-desktop`
- `Kilsat Studio Preview.app/Contents/Resources/bin/gpu_probe`

Each value contains `sha256` (the final executable member digest), `lipo_archs`
(raw text) and `otool_libraries` (raw text). CI requires arm64 and system-library
paths. The collector preserves tool output as caller-supplied observations, not
an independent claim of architecture, linkage, signature validity or hardware
identity. This extension requires both executables even though the package-only
inventory permits a desktop-only archive.

```sh
python3 scripts/build_evidence.py \
  --archive dist/Kilsat-Studio-Preview-arm64.zip \
  --audit dist/Kilsat-Studio-Preview-arm64.audit.json \
  --lock dist/build-inputs/Cargo.lock \
  --desktop-metadata dist/build-inputs/desktop-metadata.json \
  --helper-metadata dist/build-inputs/helper-metadata.json \
  --rustc dist/build-inputs/rustc.txt \
  --cargo dist/build-inputs/cargo.txt \
  --native-observations dist/build-inputs/native.json \
  --output dist/Kilsat-Studio-Preview-arm64.build-evidence.json
```

The output must be a new path. Exit 0 means bounded observations were collected;
exit 2 means invalid/missing input or output failure. Neither grants approval.
CI retains the ZIP, audit, build-evidence report and raw `build-inputs` together.

## What the report establishes

The collector checks the audit's ZIP SHA256/size against the supplied archive,
requires a valid package inventory and full source commit, then rehashes both
executable members without extraction. It checks the native observations against
those member hashes, rejecting mismatches and duplicate/missing executables.
The report fingerprints the audit, Cargo.lock, each metadata document, native
observations and compiler/Cargo text.

Each graph is a dependency closure rooted separately at `studio-desktop` or
`motion-render`. Resolved package name/version/source must match Cargo.lock;
registry checksum declarations, enabled features, dependency edges and declared
license fields are retained. Helper metadata must enable `gpu`.

**This is a target-filtered Cargo workspace metadata closure, not an exact
compiler-unit graph or proof that every listed component was shipped.** Resolved
build/dev edges and workspace feature unification may be present. Cargo.lock
checksums are declarations, not a new hash of fetched upstream source. Local
source contents, embedded assets, compiler inputs and observation authenticity
are not attested by this report. Metadata declarations never replace collected
license/NOTICE text, reviewed SPDX choices or applicable source obligations.

Inputs are limited to 16 MiB each, compiler text to 64 KiB each, archive and each
selected member to 128 MiB, graph packages/nodes to 2048 each and output to
16 MiB. The prerequisite package auditor imposes its additional ZIP boundary
limits. The collector validates document shapes and does not import package
code. Its output is a separate sidecar, avoiding a containing-ZIP hash cycle.

`distribution_approved` remains false; each component's license status remains
`unreviewed`. `physical_device_acceptance` remains `not_granted`. There is no
success flag that promotes these observations into redistribution clearance or
physical acceptance.

## Physical acceptance remains separate

Run the [physical Mac procedure](workstreams/mac-device-acceptance.md) using a
trusted exact build, preserving BUILD-INFO, archive/member hashes, reports and
human observations. The required checks still include Finder/UI/project round
trips, actual Metal fixture execution, uncached offline Kokoro speech, named
audio-device playback/recovery, scene/export synchronization and perceptual
mouth timing. The [practical testing guide](practical-testing.md) gives the app
steps. This collector performs none of them.

The [final license workstream](workstreams/final-license-audit.md) still requires
review of actual upstream source and notices, embedded/native contents, selected
local Python environment, model/voice/language rights and release obligations.
A green CI observation step closes neither that review nor the device gates.
