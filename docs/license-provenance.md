# Retained managed-install license evidence

The managed installer now retains local environment evidence **after the already
consented installation**, without importing Kokoro, Torch, spaCy or package code.
This is evidence collection, not distribution approval. Existing profiles remain
readable; opening one never installs anything or manufactures missing evidence.

## Retained files

`<managed-root>/evidence/index.json` binds the profile's environment revision to:

- `freeze.txt`: complete `pip freeze --all` output, bounded to 1 MiB. URL user
  information, query strings and fragments are removed before retention and
  hashing. The revision's suffix hashes these retained bytes.
- `distributions.json`: installed distribution name/version, license declaration,
  classifiers and available license/NOTICE/COPYING files identified through
  `importlib.metadata`. Package modules are not imported. Exact license bytes are
  retained as base64 with size and SHA256, at most 256 KiB per file and 4 MiB total;
  the complete JSON is bounded to 8 MiB. Missing file inventories remain marked
  absent; absence of collected text is not evidence that no obligations exist.
- The exact setup manifest, including pinned model/config/voice and English-wheel
  source URLs, immutable revisions, sizes and hashes, with a canonical JSON hash.
  This records installer inputs, not a second verification of installed weights.

The profile's optional `license_evidence` object has schema version 1, an absolute
index path and the index SHA256. The index has schema version 1, the exact profile
revision, `distribution_approved: false`, the setup manifest and its fingerprint,
and exactly two relative file references with byte counts and SHA256 values.
Evidence reads enforce canonical directory containment and resource budgets.
Tampering with an evidence file fails validation instead of treating it as an
old profile. Local hashes establish integrity relative to the selected profile;
they do not authenticate the installer or replace a signed release attestation.

An installation fails and removes its owned incomplete root if collection fails,
so a successful new profile cannot silently omit its promised evidence. License
files exceeding the per-file or aggregate text budget are explicitly recorded as
`not-collected-size-budget` or `not-collected-total-budget`, without content or
a hash claim. These partial inventories remain unresolved; a text-collection
budget does not prevent an otherwise valid installation. Paths and license text can still
contain personal/local information: review evidence before publishing it.

## Offline inventory

Select the exact existing profile explicitly:

```sh
python3 scripts/license_inventory.py \
  --managed-profile /absolute/managed-root/profile.json \
  --output dependency-inventory.json --strict
```

No subprocess, install, download or inference is performed by this command. The
report verifies retained index/file hashes, environment revision, exact manifest
binding and individual license-text hashes. It labels collected text **unreviewed**,
compares managed direct pins to observed installed versions, and records whether
the current checkout's manifest matches the installed manifest. Select either
managed evidence or a `--pip-inspect` input; ambiguous mixtures are rejected.
An old profile without evidence produces an actionable error rather than
reinstalling. `--strict` continues to exit 1 while release-review gaps remain.

## Still open

- Full transitive wheel/platform/ABI hash locking and original wheel hashes;
  installed license hashes do not identify the original installation artifact.
- Human review of all actual license texts, SPDX alternatives, bundled native
  libraries, fonts, data and NOTICE/source obligations for a defined Mac release.
- Exact model/voice/language revision notices and applicable voice provenance and
  redistribution rights. A pinned voice tensor is not proof of rights.
- FFmpeg/eSpeak/Python and target-specific Rust artifact provenance and the final
  assembled application's shipping boundary.

Only offline fixtures were used to test this change. No real managed environment,
license corpus, model, voice or speech was downloaded or exercised here. See
[dependency-audit.md](dependency-audit.md) for the broader release audit.
