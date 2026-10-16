# Dependency audit evidence and unresolved release work

Updated 2026-10-16. **Distribution is not approved by this inventory.**
The reproducible evidence collector is `scripts/license_inventory.py` (Python
3.11+ standard library). It reads files, hashes selected binaries and emits JSON;
it never installs packages, imports an inference library, downloads assets,
executes an encoder or runs Cargo/pip. No license is automatically accepted.

## Completed evidence tooling

- [x] Enumerate every Cargo.lock package with version, source and checksum.
- [x] Join optional Cargo metadata using exact name/version/source identity;
  missing or ambiguous declarations remain unknown. License-file paths are
  declarations, not collected license contents.
- [x] Enumerate the managed setup's direct Python pins separately from the broader
  development requirements and optional observed `pip inspect` distributions.
- [x] Record immutable model/config/voice and English-wheel manifest pins,
  full SHA256 values, sizes and source URLs without downloading them.
- [x] Hash explicitly selected external binaries without executing them.
- [x] Keep every entry unreviewed, distinguish absent installed evidence, and
  provide a strict exit code that fails while release-review gaps remain.
- [x] Ten offline tests cover identity matching, determinism, input changes,
  exact-pin validation, mismatched Python versions and nonexecuted binary hashes.

Reproduce an inventory from the checkout:

```sh
python3 scripts/license_inventory.py --output dependency-inventory.json
python3 -m unittest discover -s tests -p test_license_inventory.py -v
```

For an already installed, deliberately selected environment, collect supplemental
metadata explicitly (these commands do not install packages or run inference):

```sh
cargo metadata --locked --offline --format-version 1 > cargo-metadata.json
/path/to/selected/python -m pip inspect --local > pip-inspect.json
python3 scripts/license_inventory.py \
  --cargo-metadata cargo-metadata.json --pip-inspect pip-inspect.json \
  --binary ffmpeg=/absolute/path/to/ffmpeg \
  --output dependency-inventory.json --strict
```

Cargo metadata may fail if platform-specific source metadata is not locally
cached; the collector works without it and labels missing declarations. Keep the
original supplemental files beside the report. JSON input fingerprints use
canonical sorted JSON; repository source fingerprints hash exact file bytes.
Installed-environment metadata may contain local paths or direct installation
URLs: inspect before publishing. `--strict` currently exits **1 deliberately**;
zero means collection succeeded without strict gating, not release approval.
Exit 2 identifies invalid inputs or file errors.

## Repository evidence at this checkpoint

The unextended Cargo.lock contains **423 packages** (including workspace and
platform/build dependencies). Its SHA256 is
`7fb4c39b3ab75ca829e6487ac484f4b7cf6a27f827f1d81f7ec19a5a4c2149ed`.
This is a lockfile inventory, not a claim that all 423 enter a particular Mac app.
Generate a fresh report after dependency changes; this count is historical.

Managed setup pins `kokoro==0.9.4`, `misaki[en]==0.9.4`, `torch==2.6.0`,
`numpy==1.26.4`, and `spacy==3.7.5`. Development `requirements.txt` instead
contains broad Torch/NumPy constraints. Neither is a complete transitive wheel
lock. `setup_kokoro.py` includes the SHA256 of `pip freeze --all` in its profile
revision; it does **not** retain the complete freeze text or wheel hashes, so
that hash alone cannot reconstruct the environment.

The setup manifest identifies repository `hexgrad/Kokoro-82M`, revision
`f3ff3571791e39611d31c381e3a41a3af07b4987`, and four hashed files: config,
weights, `voices/af_heart.pt`, and `voices/am_michael.pt`. The English wheel is
`en_core_web_sm-3.7.1-py3-none-any.whl`, pinned to repository revision
`8fb6f8155286360bf5486fe399e2738d1df7ded2` with a full hash. These are verified
**manifest declarations**; this work did not download or inspect those artifacts.

## Upstream evidence and its limits

The [official Kokoro 0.9.4 release metadata](https://pypi.org/project/kokoro/0.9.4/)
was inspected on 2026-10-16. It declares an Apache license and Python
`>=3.10,<3.13`. This exact-release declaration is narrower than mutable upstream
main described in the older [preliminary review](kokoro-dependency-review.md).
The wheel contents, license files and notices have not been inspected here.

Attempts to inspect the exact model revision's
[license](https://huggingface.co/hexgrad/Kokoro-82M/blob/f3ff3571791e39611d31c381e3a41a3af07b4987/LICENSE)
and [model card](https://huggingface.co/hexgrad/Kokoro-82M/blob/f3ff3571791e39611d31c381e3a41a3af07b4987/README.md),
and the exact English revision's
[metadata](https://huggingface.co/spacy/en_core_web_sm/blob/8fb6f8155286360bf5486fe399e2738d1df7ded2/meta.json)
were unsuccessful through the available reader. They remain **unverified at the
pinned revision**. Mutable metadata from another revision is not substituted.
The prior preliminary review remains context, not evidence of selected-artifact
clearance or a completed voice-rights review.

## Still required before distribution clearance

- [ ] Capture the actual target-specific Cargo dependency graph and installed
  Python environment, exact wheel hashes/platform tags, Python ABI and sources.
- [ ] Collect license texts, copyright/NOTICE and modification notices for actual
  shipped Rust/Python artifacts, bundled fonts, dictionaries and language data;
  review SPDX alternatives and license-file-only packages individually.
- [ ] Inspect exact model/config/voice revision notices and establish applicable
  voice provenance/redistribution terms; a preset name or hash proves no rights.
- [ ] Inventory native libraries in the selected Torch/NumPy/spaCy/eSpeak wheels,
  including phonemizer/loader relationships and any corresponding source duties.
- [ ] Inspect the exact FFmpeg binary/build configuration if it is bundled;
  a user-selected executable hash does not establish its license or provenance.
- [ ] Define the shipping boundary and review the assembled distribution's
  obligations. The current preview excludes weights, voice tensors, Python and
  its installed packages; consented local setup is a separate boundary.
- [ ] Preserve evidence and notices with each concrete release; obtain a human
  release review rather than treating this technical inventory as legal advice.

User-reported first Kokoro speech on an Apple M1 is acknowledged in
`master-progress.md`. Successful speech does not resolve these licensing gaps,
full offline acceptance, or physical-device synchronization measurements.
