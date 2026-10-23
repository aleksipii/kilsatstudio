# Choosing local reference voices

The reference helper keeps its historical defaults (`am_michael` for DAD,
`af_heart` for STAN). The latter is a female preset, not a reviewed boy voice.
No replacement is selected automatically. Preset files alone do not establish
age, performance or suitability; listen before choosing.

Use the Python interpreter in the existing Kokoro environment. Listing voices
reads only local English `.pt` files and does not import Kokoro or load a model:

```sh
python scripts/night_room_real_speech.py --kokoro /path/to/Kokoro --list-voices
```

To compare a locally installed candidate, select it explicitly and use a separate
output directory for each audition. Replace `INSTALLED_PRESET` with a name from
the listing. No voices or models are downloaded by this helper.

```sh
python scripts/night_room_real_speech.py --kokoro /path/to/Kokoro \
  --boy-voice INSTALLED_PRESET --out /path/to/audition-candidate
cargo run -p motion-export --example night_room_reference -- \
  export /path/to/audition-frames /path/to/audition-candidate
```

`--dad-voice` selects the other voice. `--language en-us` (default) accepts
American English presets (`a…`); `--language en-gb` requires British presets
(`b…`) for both characters. This changes pronunciation, not the script text;
no translation occurs. `--speed` must be finite and within 0.5–2. All selected
voice files, model and config must exist before synthesis starts. The listing
is an inventory, not an integrity or licensing audit; the runtime validates
voice tensors during inference.

Compare intelligibility, age impression, delivery and comic timing with the same
lines. Rendering with candidate WAVs recalculates shot durations. Mouth timing
still comes from text-estimated visemes: Kokoro phonemes are reported without
validated word/phoneme timestamps, and this command does not perform forced
alignment. Listening and audio-versus-mouth review remain required.

To render the same reference with real speech on a GitHub-hosted Apple Silicon
runner instead of locally, use the manual, consent-gated workflow described in
[real-speech reference CI](real-speech-reference-ci.md). It downloads the pinned
model and selected voices only when the owner dispatches it with
`consent_download=true`, and its `boy_voice` input selects STAN's preset.

Validation without model assets:

```sh
python3 -m unittest discover -s scripts/tests -p 'test_night_room_real_speech.py' -v
```
