# Final preview package inventory

`scripts/package_audit.py` observes the final signed preview ZIP without
extracting files, executing binaries, installing dependencies or downloading
anything. It writes an external JSON sidecar with the ZIP SHA256/size, expected
source commit/target/profile, and sorted final-byte file hashes, sizes, modes and
classifications. The commit and architecture are checked against BUILD-INFO;
they are not independent proof of compiler inputs or Mach-O architecture.

```sh
python3 scripts/package_audit.py \
  --archive dist/Kilsat-Studio-Preview-arm64.zip \
  --commit FULL_40_CHARACTER_SOURCE_SHA \
  --target aarch64-apple-darwin --profile debug \
  --require-signature-metadata \
  --output dist/Kilsat-Studio-Preview-arm64.audit.json
```

The matching checkout supplies exact LICENSE copies, top-level example YAMLs,
runtime/setup/diagnostic source, setup manifest, requirements and documentation.
The auditor shares trusted adjacent packager code for generated icon and plist
expectations; it never imports anything from the archive. BUILD-INFO must match
the supplied full commit, supported target and profile exactly. The desktop is
required; the GPU helper is optional and has its own executable component record.
Both must be nonempty regular files with executable permissions.

Only the current packager boundary is admitted. Unknown resources (including
models, wheels, FFmpeg or native libraries) are visible errors, rather than
silently added to the distribution. The exact CodeResources path is inventoried
as unverified signature metadata. `--require-signature-metadata` requires its
presence, not a valid signature. CI separately runs the existing codesign checks.
macOS ditto AppleDouble files are admitted only for a present known file or an
ancestor directory. Their opaque resource-fork/extended-attribute bytes are hashed
and explicitly unreviewed; their presence proves no origin, rights or safety.

Limits are 128 MiB ZIP, 128 MiB per member, 512 MiB expanded total, 4096 entries,
4 MiB central directory, 4096 UTF-8 bytes per path, 1 MiB generated metadata and
8 MiB report. Hashing uses 64 KiB chunks. Before opening ZipFile, the collector
bounds its central-directory allocation. ZIP64, multivolume, encrypted,
unsupported compression, duplicate/noncanonical/traversing paths, symlinks,
special files and privileged modes are rejected. No archive member is extracted.
ZIP identity is checked again on the same open descriptor after inspection.
This detects observed mutation; it is not an immutable upstream provenance proof.

Reports replace a regular output atomically through an operation-owned temporary
file. Output symlinks and aliases to the input archive are rejected. Invalid
inputs still produce a report where possible. Exit codes distinguish:

| Exit | Meaning |
| --- | --- |
| 0 | Package facts collected and boundary valid; review still unresolved |
| 1 | `--strict` collection valid, but redistribution review remains unresolved |
| 2 | Invalid package/evidence or report could not be written |

`distribution_approved` is always false. Exact target/feature graphs, compiler
identity, native linkage, upstream source/wheel identity, copyright/NOTICE texts,
license alternatives and source/redistribution obligations remain unresolved.
The report does not accept a caller-provided approval boolean or digest as review.
Build-graph ingestion/linkage is a later extension, not part of this increment.

macOS preview CI collects after signing and ZIP creation, requires package-boundary
success and attaches the ZIP and sidecar together. A green collection step is
not a license release gate. Existing GPU/hardware/model/quality acceptance gates
are unaffected. See [license audit workstream](workstreams/final-license-audit.md)
and [master progress](master-progress.md).
