# Kokoro CPU sidecar (protocol v1)

This first backend uses upstream Python/PyTorch. It explicitly loads local
`config.json`, `kokoro-v1_0.pth` and a selected `voices/<preset>.pt` tensor.
It never intentionally downloads assets or invokes an installer. Network calls
inside the Python process are denied, HF offline flags are set, and spaCy's
automatic downloader is disabled. Missing English G2P assets produce an error.

Install dependencies into a dedicated Python 3.10–3.12 environment after consent,
including compatible `en_core_web_sm`, Misaki English assets and eSpeak NG for
out-of-vocabulary words. `requirements.txt` pins the Kokoro API; the broad torch
and numpy ranges are development constraints, **not a redistribution lockfile**.
A release must lock/test its entire environment for macOS arm64 and audit all
runtime and phonemizer licenses. Keep all model weights outside Git.

Configure `KokoroConfig` with absolute local paths and `model_version` containing
an immutable upstream revision plus the installed dependency lock digest. The
provider also hashes actual model/config/voice/runtime bytes, text, language,
speed and style to form the cache key. Syntheses run as bounded one-shot child
processes. The host must call `SpeechProvider` from a worker thread. Timeout kills
and reaps the process. PCM16 WAV headers and samples are validated before caching.
Malformed cache entries regenerate; corrupt model assets fail clearly.

Direct protocol invocation (the Rust host normally constructs these files):

```
python runtime.py --request /absolute/request.json --response /absolute/response.json
```

Request fields are `protocol: 1`, `request: {text, voice: {preset, language,
speed, style}}`, `config_path`, `model_path`, `voice_path`, `output_path`.
Response is either `{ok:false, protocol:1, error:...}` or `{ok:true, protocol:1,
sample_rate:24000, sample_count:..., phonemes:..., alignment:"not_aligned"}`.
The output is 24kHz mono PCM16 WAV. Style/emotion is retained as metadata but is
not implemented as acoustic conditioning. Raw phonemes have **no timestamps**;
use the separate alignment stage, never assume upstream timing is ground truth.

## Backend decision and sources

- [Upstream KPipeline](https://github.com/hexgrad/kokoro/blob/main/kokoro/pipeline.py)
  accepts an existing KModel and a float32 voice tensor, supporting explicit local
  assets with no voice-name download path.
- [Upstream KModel](https://github.com/hexgrad/kokoro/blob/main/kokoro/model.py)
  accepts explicit config and checkpoint paths.
- [Upstream ONNX export example](https://github.com/hexgrad/kokoro/blob/main/examples/export.py)
  demonstrates a potential later ONNX backend. Native Rust ONNX, Core ML, MPS and
  Metal execution have not been validated here. Python CPU minimizes initial
  conversion and phonemizer integration risk while keeping the Rust trait stable.
- [Kokoro model card](https://huggingface.co/hexgrad/Kokoro-82M) identifies Apache-2.0
  model weights and discusses training sources and attribution. Review the exact
  downloaded revision's model card and [voice documentation](https://huggingface.co/hexgrad/Kokoro-82M/blob/main/VOICES.md)
  and retain applicable notices before redistributing any assets. The model card
  alone is not a completed license audit of every dependency or voice asset.

## Verification status

`python3 -m unittest discover -s services/kokoro-runtime/tests -v` validates the
stdlib protocol, malformed requests, protected asset paths, missing G2P assets,
network guards and the synthesis-to-WAV path. Synthesis tests use real NumPy
with stub Torch/Kokoro modules; they verify PCM samples and CPU arguments,
not actual speech inference. These tests skip explicitly when NumPy is absent. No macOS/Apple Silicon machine, model assets
or audio listening/phoneme timing validation were available in this environment.
A real offline macOS CPU WAV smoke test and release dependency/license audit are
still required. This component therefore does not satisfy milestone 2 by itself.


## Opt-in macOS CPU smoke test

After consenting to and separately installing the dependencies and approved model,
voice and English G2P assets, run with a new output filename:

```sh
python services/kokoro-runtime/smoke_macos.py --allow-local-inference \
  --config /absolute/models/config.json \
  --model /absolute/models/kokoro-v1_0.pth \
  --voice /absolute/models/voices/af_heart.pt \
  --output /absolute/audio/kokoro-smoke.wav
```

The runner refuses non-macOS platforms and existing outputs, uses a bounded
runtime timeout, checks the mono PCM16 WAV, and reports the actual machine
architecture and CPU backend. It never installs dependencies or assets. Runtime
network guards are defense in depth inside Python, not an OS network sandbox;
release verification should additionally run with external network access denied.
Listen to the WAV and record the exact installed asset/dependency revisions before
reporting speech quality or Apple Silicon compatibility. This runner does not
validate lip synchronization, playback devices or video export.
