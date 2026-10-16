# Managed Kokoro setup sources

Metadata checked 2026-10-16. The setup manifest pins exact immutable model and
English pipeline objects. Development fetched repository/package metadata and
the small JSON configuration for its digest; no model weights, voice tensors or
language wheel were downloaded and no installation/inference was exercised.

- Kokoro repository: [official model API](https://huggingface.co/api/models/hexgrad/Kokoro-82M?blobs=true), revision `f3ff3571791e39611d31c381e3a41a3af07b4987`. LFS metadata supplies full SHA-256 and sizes of weights and both voices. Config is2351 bytes; its downloaded text SHA-256 is pinned in setup-manifest.json.
- English pipeline: [official spaCy model API](https://huggingface.co/api/models/spacy/en_core_web_sm?blobs=true), revision `8fb6f8155286360bf5486fe399e2738d1df7ded2`. Its12803381-byte wheel has SHA-256 `86cc141f63942d4b2c5fcee06630fd6f904788d2f0ab005cce45aadb8fb73889`. The revision-specific meta.json identifies model3.7.1, spaCy>=3.7.2,<3.8.0 and MIT. The generic upstream filename is renamed locally to the valid versioned wheel filename.
- Official PyPI JSON metadata for [Kokoro0.9.4](https://pypi.org/pypi/kokoro/0.9.4/json), [Misaki0.9.4](https://pypi.org/pypi/misaki/0.9.4/json), [spaCy3.7.5](https://pypi.org/pypi/spacy/3.7.5/json), [NumPy1.26.4](https://pypi.org/pypi/numpy/1.26.4/json), [Torch2.6.0](https://pypi.org/pypi/torch/2.6.0/json). CPython3.12 macOS11 arm64 wheels are listed for spaCy, NumPy and Torch; this is availability metadata, not an installed-environment compatibility result.

The installer deliberately pairs spaCy3.7.5 with the hash-verified English3.7.1
model. Direct requirements are pinned. Transitive dependencies are resolved by
pip using wheels only; the installed freeze is captured and hashed into profile
identity. This is not a complete hash-locked environment, a redistribution audit
or proof of CPU inference on Apple Silicon. Model/data/package download remains
explicitly opt-in in the UI; missing host Python requires a compatible local
installation. Setup never installs a system interpreter.

The preview packages only small scripts and manifest metadata. No model, voice,
English wheel or Python environment is redistributed. Exact asset/license and
transitive native-library review remains a separate release gate.
