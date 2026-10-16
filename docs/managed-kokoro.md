# Managed local Kokoro setup

The desktop setup workflow installs a private Python environment and verified local
model assets after explicit download consent. It does not embed model weights in
the application or run installation on the editor event loop. Existing manually
configured Kokoro environments remain usable.

Requirements: macOS, a Python 3.10–3.12 interpreter with `venv` and internet access
for this installation. Inference subsequently uses the CPU and local files. No
speech or screenplay is uploaded. Setup downloads model assets from Hugging Face,
a pinned English language wheel, and Python package wheels from PyPI. Package
indexes can see network metadata; setup does not guarantee offline inference until
a real local speech test passes.

## Installer protocol

```sh
python3 services/kokoro-runtime/setup_kokoro.py --plan \
  --root /absolute/new-profile --manifest /absolute/manifest.json
python3 services/kokoro-runtime/setup_kokoro.py \
  --root /absolute/new-profile --manifest /absolute/manifest.json \
  --allow-downloads --cancel-file /absolute/cancel-marker
```

`--plan` performs no filesystem access, networking or subprocess execution. Actual
installation requires the explicit consent flag before reading the manifest or
creating directories. The root must not exist; missing parent directories are created after consent. Never move
the resulting virtual environment, because installed launchers contain absolute
paths. A failed installation removes only the root created by this attempt. An
existing root is never removed or overwritten; choose a fresh root for retry.

Manifest schema version 1 contains `model_repository` (`hexgrad/Kokoro-82M`), a
40-character immutable `model_revision`, `files` records with `path`, HTTPS `url`,
`sha256` and `size_bytes`, exact direct `packages` pins, plus `english_model_url`,
`english_model_sha256` and `english_model_size_bytes`. Required model assets are
`config.json`, `kokoro-v1_0.pth`, `voices/af_heart.pt`, and `voices/am_michael.pt`.
The manifest is a trusted application input: do not substitute an unreviewed
manifest. Hash verification establishes integrity relative to that manifest.

Progress is one JSON object per line with `phase`, `status`, and `message`.
Success uses `phase: "complete"`, `status: "ready"`, `profile` containing the
absolute profile filename and `settings` containing its object. Failure uses
`phase: "failed"`, `status: "error"`, and process exit status 1.

The installer creates `root/profile.json` atomically only after the local doctor
passes. Schema version 1 contains `python`, `config`, `model`, `voices`, `cache`
(absolute paths within the root), and `revision` (model commit plus a SHA256 of
`pip freeze --all`). This identifies the resolved environment as well as the
model. Transitive dependencies resolve at install time; this is not a fully locked
or reproducible dependency set, and redistribution licenses are not certified. Asset downloads verify their complete size and streamed SHA256; pip is
restricted to binary wheels and ignores user pip configuration.

Creating the cancellation marker cancels cooperatively. Downloads have a
30-second socket timeout and a 15-minute per-asset limit, and child commands have bounded time and diagnostic
output. Command cancellation terminates the entire child process group, escalating
to a kill after three seconds. Controllers should allow at least 35 seconds for
cooperative cleanup before escalation. SIGTERM is handled as interruption; an
unavoidable hard kill may leave an incomplete root requiring manual inspection.

The prerequisite doctor does not import the model, perform synthesis, establish
perceptual lip-sync quality or certify asset redistribution licenses. Use the
existing opt-in macOS speech smoke test and listen to the generated WAV before
claiming Kokoro works on a particular Mac. This implementation's unit tests stub
network and subprocess calls; they download no assets or dependencies.
