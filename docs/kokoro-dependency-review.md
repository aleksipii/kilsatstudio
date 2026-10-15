# Kokoro CPU dependency and asset review

Reviewed 2026-10-15 against the official sources below. This is a preliminary inventory, not release clearance. No model assets were downloaded and no actual Kokoro inference or macOS dependency environment was exercised for this review.

## Current distribution boundary

The macOS preview packages the Rust application and small Python sidecar scripts. `scripts/package_macos.py` deliberately excludes model weights, voice tensors, Python, and installed Python dependencies. Users supply a separate, consented local environment and assets. This boundary does not remove obligations if a later installer or release distributes those components.

`services/kokoro-runtime/requirements.txt` pins only `kokoro==0.9.4`; `torch>=2.2,<3` and `numpy>=1.26,<3` are development constraints. There is no complete Python dependency lock with wheel hashes, architecture tags, or bundled notices. Nothing here approves arbitrary future versions satisfying those ranges.

## Inventory

| Component | Verified upstream declaration and source | Unresolved release work |
| --- | --- | --- |
| Kokoro Python 0.9.4 | [Project metadata](https://github.com/hexgrad/kokoro/blob/main/pyproject.toml) identifies 0.9.4, Python `>=3.10,<3.14`, and dependencies including `misaki[en]>=0.9.4`, Torch, NumPy, Transformers, Hugging Face Hub and Loguru. [License](https://github.com/hexgrad/kokoro/blob/main/LICENSE): Apache-2.0. | Verify downloaded 0.9.4 wheel contents and hashes against these declarations; retain required license, attribution, NOTICE where applicable, and modification notices. Mutable upstream `main` is not an immutable release manifest. |
| Kokoro-82M v1.0 weights/config | [Official model card](https://huggingface.co/hexgrad/Kokoro-82M) labels weights Apache-2.0 and gives model SHA256 `496dba118d1a58f5f3db2efc88dbdc216e0483fc89fe6e47ee1f2c53f18ad1e4`. It separately lists CC BY training sources and attribution. | Record immutable repository revision and hashes of every actual file; preserve applicable upstream notices. The published checkpoint hash does not authenticate config or voice files. |
| `af_heart`, `am_michael` voice tensors | [Official VOICES.md](https://huggingface.co/hexgrad/Kokoro-82M/blob/main/VOICES.md) lists both presets and short hash prefixes; repository metadata declares Apache-2.0. | **Unresolved:** this review found no separate per-English-voice rights/provenance statement establishing all voice-asset and redistribution terms. Repository labeling alone is not a completed voice-rights audit. Verify exact tensors, full hashes, revision-specific notices and applicability of the license before bundling. Do not infer rights from preset names. |
| PyTorch | [Upstream license](https://github.com/pytorch/pytorch/blob/main/LICENSE) permits redistribution subject to preservation of copyright, conditions and disclaimers, and prohibits endorsement using named contributors without permission. | Select exact macOS arm64 wheel and Python ABI; inspect its included third-party notices and native libraries. Broad Torch version range is not a compatibility or redistribution guarantee. |
| NumPy | [Upstream license](https://github.com/numpy/numpy/blob/main/LICENSE.txt) contains BSD-style terms; binary distributions may include separately licensed components. | Pin wheel/hash and collect the selected wheel's complete licenses, including bundled numerical libraries. |
| Misaki English G2P | [Metadata](https://github.com/hexgrad/misaki/blob/main/pyproject.toml) currently identifies 0.9.4 and English extras including spaCy, spaCy Curated Transformers, num2words, phonemizer-fork, espeakng-loader, Torch and Transformers. [License](https://github.com/hexgrad/misaki/blob/main/LICENSE): Apache-2.0. | Kokoro's lower bound does not pin installed Misaki. Audit package resources/dictionaries and every resolved extra; do not assume all share Misaki's license. |
| spaCy and `en_core_web_sm` | [spaCy code license](https://github.com/explosion/spaCy/blob/master/LICENSE): MIT. Model releases have separate [release metadata](https://github.com/explosion/spacy-models/releases/tag/en_core_web_sm-3.8.0). The official [Hugging Face model metadata](https://huggingface.co/spacy/en_core_web_sm/blob/main/meta.json) snapshot inspected describes model 3.7.1, MIT, requiring spaCy `>=3.7.2,<3.8.0`, and lists additional source acknowledgements, including WordNet. | Neither model nor spaCy is pinned here. Choose a matching pair deliberately; do not treat the HF snapshot as the latest compatible model. Preserve model/source notices as well as spaCy's MIT notice. Preinstall the English pipeline: runtime automatic downloading is disabled. |
| eSpeak NG / phonemizer path | [eSpeak NG COPYING](https://github.com/espeak-ng/espeak-ng/blob/master/COPYING) supplies GPLv3 terms. Misaki's English dependency list includes loader/phonemizer packages that need separate inspection. | Determine precisely which native library, loader, dictionaries and data enter the resolved environment, their licenses, and their loading/linking relationship. Complete GPL/source-distribution analysis before distributing this path. A sidecar process alone does not establish compliance. |

Unpinned transitive packages also include Hugging Face Hub, Transformers and their dependencies. This inventory is intentionally not a full software bill of materials. Check every resolved artifact's actual licenses and notices rather than extrapolating from the top-level packages.

## macOS CPU compatibility

Use the project's documented Python 3.10–3.12 setup as a starting range, then lock one exact version. Kokoro's broader Python declaration does not establish availability of every native dependency wheel. [PyTorch's official installation guide](https://pytorch.org/get-started/locally/) documents macOS installation and CPU operation; its general platform guidance is not proof that an arbitrary selected Torch wheel supports the preview's macOS minimum. Check the wheel deployment target, arm64 architecture, Python ABI, NumPy ABI, and eSpeak library architecture on the actual test Mac. Use a native arm64 interpreter on Apple Silicon.

The sidecar explicitly selects CPU. No native Rust inference, Metal, MPS, Core ML or ONNX acceleration is established by this review. Offline readiness requires every model, voice, dictionary and English pipeline resource to be installed beforehand; Python network guards are not an OS network sandbox.

## Next verification gate

1. Resolve a dedicated arm64 macOS environment; commit an exact dependency/hash manifest and capture Python, macOS and wheel platform versions.
2. Inventory every installed distribution, native library and language resource; collect licenses/notices and resolve voice and phonemizer redistribution questions before bundling.
3. Record immutable model/config/voice revisions and full SHA256 values outside Git; retain source attribution with the manifest.
4. Run `doctor.py` and the opt-in `smoke_macos.py` with external network access denied. Save the report and WAV; listen for intelligibility and test two English voices plus speed changes.
5. Test playback, cancellation and regeneration in the desktop application on that same machine. Measure phoneme alignment separately; a successful WAV is not validated lip synchronization.
