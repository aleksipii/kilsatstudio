# Lip-sync consistency and comic-timing measurements

This document compares the night-room mouth tracks with the real dialogue audio and
measures the sequence timing. It describes one hand run on 2026-10-21 using three
Kokoro WAVs from one local synthesis (24 kHz mono, kept outside Git in
`~/Library/Application Support/Kilsat Studio/night-room-audio/audio/`).

**What these numbers are.** All of them are consistency checks between estimates.
The energy envelope only marks where the signal is above a loudness threshold. It
does not detect speech or recognise words. Rhubarb Lip Sync is an audio-based
*estimate*, not ground truth, so agreement with Rhubarb is consistency, not quality.
Nothing here measures perceptual lip-sync quality, and nothing here says whether the
timing is funny. Both still need human review against picture and sound.

## Tools

| Tool | What it does |
| --- | --- |
| `motion_lipsync::timing_eval` | RMS energy envelope, speech span, common mouth set, track agreement, mouth versus envelope consistency |
| `crates/motion-lipsync/examples/lipsync_compare.rs` | For each WAV and its text: the envelope, the text estimate scaled to the whole clip (`text_full`, the old behaviour) and to the speech span (`text_span`, the new behaviour), and optionally Rhubarb PocketSphinx (given the transcript) and Phonetic cues. It writes a table and, with `--out`, one JSON file per line |
| `night_room_reference -- timing [wav-dir]` | Comic-timing table for the reference sequence (`support::comic_timing`) |

None of these tools use the network. Rhubarb runs only when you pass its absolute
path, through the existing `rhubarb::align` backend.

### Energy envelope (documented threshold)

`EnvelopeConfig::DEFAULT_24K` uses these settings:

- a 20 ms RMS window (480 samples), centred on 10 ms cells (240 samples)
- a cell counts as voiced when its level is at least
  **max(−50 dBFS, loudest cell − 35 dB)**
- silent gaps shorter than **100 ms** between voiced runs are bridged, so stop
  closures and word joins do not count as pauses
- voiced runs shorter than **30 ms** are dropped as clicks

The speech span runs from the first voiced sample to the last. Leading and trailing
silence are the parts of the clip outside that span. Because the window is centred,
the span can extend into silence by at most one 10 ms cell. The synthetic tests
check this.

### Metrics

- **Common mouth set.** These are the nine Rhubarb shapes with `X` (rest) and `A`
  (closed lips) merged into one closed class. Text estimates are converted with the
  existing lossy `EstimatedVisemeTrack::to_mouth_cues`. A shape is *open* when it is
  not in the closed class.
- **Agreement per 24 kHz sample.** `shape` and `active` use the same definitions as
  `services/kokoro-runtime/viseme_acceptance.py`, applied after the X/A merge.
  `open/closed` is the fraction of samples where both tracks are open or both are
  closed.
- **Mouth versus envelope.** Open intervals are bridged with the same 100 ms rule as
  the envelope, so a /m b p/ closure inside a word is not counted as a closed pause.
  - *Onset/offset* is the first open sample minus the first voiced sample, and the
    last open end minus the last voiced end, in milliseconds. A negative onset means
    the mouth opens before the sound.
  - *Segment errors* are the mean absolute errors per voiced segment, measured
    against the open interval that overlaps that segment most. When one open interval
    spans two voiced segments, this inflates the error for one of them.
  - *Open in silence* and *closed in voiced* are reported as maximal runs, then runs
    at least one 24 fps frame long (1000 samples), then total seconds.

The Python evaluator is not run as a whole because it requires a *manual* reference
annotation, a smoke report and a human review file. Labelling Rhubarb output as
`manual` would be false. Instead, its `compare()` function was run directly on the
exported cues as a cross-check (see below).

## Commands (exact)

```sh
export CARGO_TARGET_DIR=/private/tmp/claude-501/shared-target
A="$HOME/Library/Application Support/Kilsat Studio/night-room-audio/audio"
RHUBARB=/absolute/path/to/rhubarb   # local native arm64 Rhubarb Lip Sync 1.14.0 build, with res/
cargo run -p motion-lipsync --example lipsync_compare -- --rhubarb "$RHUBARB" --out /tmp/lipsync-compare \
  "$A/dad_doorway.wav" "Hey, kid. What are you doing? Are you playing a game?" \
  "$A/stan_phone.wav"  "No. I'm trying a face-editing app." \
  "$A/dad_closeup.wav" "Oh OK. Is that a game?"
# threshold sensitivity (no Rhubarb):
cargo run -p motion-lipsync --example lipsync_compare -- --floor-dbfs -40 --relative-db 25 <same pairs>
cargo run -p motion-lipsync --example lipsync_compare -- --floor-dbfs -60 --relative-db 45 <same pairs>
# raw 9-shape cross-check with the existing evaluator's compare():
python3 -I -c 'import sys,json; sys.path.insert(0,"services/kokoro-runtime"); import viseme_acceptance as v
for p in sys.argv[1:]:
    r=json.load(open(p)); t={x["name"]:x["cues"] for x in r["tracks"]}
    for n in ("text_full","text_span","rhubarb_phonetic"):
        c=v.compare(t["rhubarb_pocketsphinx"],t[n],r["frames"]); print(p,n,c["shape_agreement"],c["active_shape_agreement"])' /tmp/lipsync-compare/*.json
# comic timing and per-line report on the real WAVs:
cargo run -p motion-export --example night_room_reference -- timing "$HOME/Library/Application Support/Kilsat Studio/night-room-audio"
cargo run -p motion-export --example night_room_reference -- report "$HOME/Library/Application Support/Kilsat Studio/night-room-audio"
```

A later rerun of the Rhubarb step on the same machine failed with
`Rhubarb failed: exit status: 1` while the system volume was full (1.7 GiB free).
The numbers below come from the earlier successful run with identical code and
defaults. The envelope-only runs and the timing runs succeeded.

## Results: envelope

| Line | Length | Peak | Threshold | Leading silence | Trailing silence | Voiced segments (s) |
| --- | --- | --- | --- | --- | --- | --- |
| dad_doorway | 3.650 s | −17.7 dBFS | −50.0 dBFS | **0.390 s** | **0.630 s** | 0.390–1.750, 1.950–3.020 |
| stan_phone | 2.625 s | −19.1 dBFS | −50.0 dBFS | **0.320 s** | **0.455 s** | 0.320–2.170 |
| dad_closeup | 2.575 s | −18.6 dBFS | −50.0 dBFS | **0.400 s** | **0.625 s** | 0.400–1.020, 1.170–1.950 |

The close-up line seems long because its clip carries about 1.0 s of silence: 0.40 s
before the speech and 0.63 s after it. The detected speech lasts 1.55 s, inside a
2.575 s clip. Every line has 0.32–0.40 s of leading and 0.46–0.63 s of trailing
silence.

Sensitivity check (leading / trailing silence in seconds):

| Threshold | dad_doorway | stan_phone | dad_closeup |
| --- | --- | --- | --- |
| max(−40 dBFS, peak − 25 dB) | 0.420 / 0.740 | 0.320 / 0.575 | 0.410 / 0.685 |
| max(−50 dBFS, peak − 35 dB) (default) | 0.390 / 0.630 | 0.320 / 0.455 | 0.400 / 0.625 |
| max(−60 dBFS, peak − 45 dB) | 0.380 / 0.580 | 0.320 / 0.425 | 0.400 / 0.555 |

The conclusion does not depend on the threshold: every setting finds hundreds of
milliseconds of silence at both ends.

## Results: mouth tracks versus envelope (before and after)

Onset and offset are in ms (mouth minus audio). Silence and voiced columns show
runs / runs of at least one frame / seconds.

| Line | Track | Onset | Offset | Seg on (abs) | Seg off (abs) | Open in silence | Closed in voiced |
| --- | --- | --- | --- | --- | --- | --- | --- |
| dad_doorway | text_full (before) | −390 | +546 | 482 | 394 | 4/3/0.993 | 2/2/0.716 |
| | **text_span (after)** | +0 | −60 | 410 | 68 | 1/1/0.075 | 4/4/0.554 |
| | rhubarb_pocketsphinx | −10 | −100 | 790 | 635 | 2/1/0.210 | 1/1/0.100 |
| | rhubarb_phonetic | −40 | +30 | 820 | 665 | 3/1/0.270 | 0/0/0.000 |
| stan_phone | text_full (before) | −320 | +379 | 314 | 379 | 2/2/0.632 | 1/1/0.314 |
| | **text_span (after)** | +0 | −54 | 447 | 54 | 0/0/0.000 | 2/2/0.322 |
| | rhubarb_pocketsphinx | +10 | −190 | 10 | 190 | 0/0/0.000 | 2/1/0.200 |
| | rhubarb_phonetic | −20 | +50 | 20 | 140 | 2/1/0.070 | 1/1/0.110 |
| dad_closeup | text_full (before) | −400 | +503 | 267 | 370 | 2/2/0.903 | 2/2/0.372 |
| | **text_span (after)** | +0 | −73 | 8 | 111 | 0/0/0.000 | 3/2/0.238 |
| | rhubarb_pocketsphinx | −40 | −130 | 25 | 65 | 1/0/0.040 | 2/1/0.140 |
| | rhubarb_phonetic | −120 | −50 | 505 | 465 | 2/2/0.270 | 1/1/0.050 |

**Finding.** The text estimate was scaled across the whole clip, silence included,
which produced a systematic offset. The mouth opened 320–400 ms before the first
sound and kept moving 380–550 ms after the last. Per line, that meant 0.63–0.99 s of
open mouth over silence. Both Rhubarb recognisers keep onset and offset within
about ±200 ms.

Some limits on reading the table:

- After the fix, a negative offset of 50–75 ms remains. The estimate ends on a
  closed or narrow shape (for example the /m/ in "game"), which the binary
  open/closed view counts as closed.
- Within-line errors are still large for the text estimate (for example a 410 ms
  segment onset on dad_doorway). It knows nothing about internal pauses beyond its
  fixed punctuation model.
- The "closed in voiced" values include legitimate voiced lip closures longer than
  100 ms.

## Results: agreement with Rhubarb (common set)

| Line | Pair | Shape | Active | Open/closed |
| --- | --- | --- | --- | --- |
| dad_doorway | text_full vs PocketSphinx | 0.149 | 0.071 | 0.564 |
| | text_span vs PocketSphinx | 0.492 | 0.278 | 0.788 |
| | text_full vs Phonetic | 0.139 | 0.066 | 0.597 |
| | text_span vs Phonetic | 0.404 | 0.193 | 0.760 |
| | PocketSphinx vs Phonetic | 0.696 | 0.578 | 0.951 |
| stan_phone | text_full vs PocketSphinx | 0.138 | 0.085 | 0.537 |
| | text_span vs PocketSphinx | 0.365 | 0.073 | 0.818 |
| | text_full vs Phonetic | 0.134 | 0.092 | 0.575 |
| | text_span vs Phonetic | 0.297 | 0.027 | 0.803 |
| | PocketSphinx vs Phonetic | 0.741 | 0.624 | 0.939 |
| dad_closeup | text_full vs PocketSphinx | 0.301 | 0.215 | 0.474 |
| | text_span vs PocketSphinx | 0.630 | 0.298 | 0.903 |
| | text_full vs Phonetic | 0.161 | 0.120 | 0.474 |
| | text_span vs Phonetic | 0.533 | 0.257 | 0.822 |
| | PocketSphinx vs Phonetic | 0.685 | 0.500 | 0.876 |

The span fix raises open/closed agreement from 0.47–0.60 to 0.76–0.90. Agreement on
the specific shape while the mouth is open stays low (active 0.03–0.30). The two
Rhubarb recognisers agree with each other far more (0.50–0.62) than either agrees
with the text estimate. The shape choice of the text estimate is therefore unlikely
to be accurate, but that is not established, because no reference annotation exists.

Cross-check with `viseme_acceptance.compare()` on the raw nine shapes (A and X kept
separate; PocketSphinx is passed in the reference slot only because the function
takes two lists):

| Line | text_full | text_span | Phonetic |
| --- | --- | --- | --- |
| dad_doorway | 0.105 / 0.068 | 0.489 / 0.293 | 0.696 / 0.589 |
| stan_phone | 0.109 / 0.082 | 0.358 / 0.098 | 0.741 / 0.646 |
| dad_closeup | 0.254 / 0.204 | 0.630 / 0.334 | 0.674 / 0.506 |

Each cell is shape / active agreement. The direction matches the common-set
results.

## Fix: rest over detected leading and trailing silence

`DialogueClip::derive_text_estimate` (in motion-voice) now calls
`timeline::speech_span(&audio)`, which uses the default envelope above, and then
`english::estimate_viseme_track_in_span`:

- The same text estimate is scaled into `[speech_start, speech_end)`, and `Rest`
  covers the samples before and after. `time_scale` is now relative to the span.
- A silent or very quiet clip, where nothing passes the threshold, keeps the
  previous whole-clip estimate.
- The provenance is still `Estimated { tool: "kilsat-english-estimator", version: "1" }`.
- The result is deterministic in the PCM samples and the text.
- The cache identity is `estimate_identity_in_span`. It equals the old
  `estimate_identity` when the span covers the whole clip and otherwise also hashes
  the span. A re-synthesised clip with the same length but different silence is
  therefore re-estimated.
- Preview and export still read the same cues through `mouth_at` and the manifest.

With the real WAVs, `night_room_reference -- report <wav-dir>` now lists 29, 22 and
13 mouth intervals (previously 27, 20 and 11; the extra two per line are the rest
intervals at either end). Mouth-variant switches are unchanged at 28, 21 and 12.
The before and after numbers above are `text_full` and `text_span`, which come from
the same functions the pipeline uses.

The fix does not address internal pause placement, shape choice or the
anticipation that animators often add (opening a frame or two before the sound).
None of these was tuned.

## Comic timing of the reference sequence (real WAVs)

All values are seconds on the sequence clock. A *word* boundary is the envelope's
speech span, not word recognition. Captions run from the speak event to the end of
the clip, so they include the clip's silence.

| Shot | Cut in | Cut out | Shot | Cut → first word | Audio | Lead | Speech | Trail | Pause after line | Hold after audio | Caption | Chars | cps | Speech cps | Last word → next first word |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| bed_closeup | 0.000 | 2.400 | 2.400 | – | – | – | – | – | – | reaction hold 2.400 | – | – | – | – | first word at 2.940 |
| doorway_wide | 2.400 | 6.800 | 4.400 | 0.540 | 3.650 | 0.390 | 2.630 | 0.630 | 1.230 | 0.600 | 3.650 | 53 | 14.5 | 20.2 | 1.700 |
| boy_phone_medium | 6.800 | 10.175 | 3.375 | 0.470 | 2.625 | 0.320 | 1.850 | 0.455 | 1.055 | 0.600 | 2.625 | 34 | 13.0 | 18.4 | 1.605 |
| dad_closeup | 10.175 | 13.500 | 3.325 | 0.550 | 2.575 | 0.400 | 1.550 | 0.625 | 1.225 | 0.600 | 2.575 | 22 | 8.5 | 14.2 | – |

How the table relates to the shot list:

- The shot list's `lead_in` (0.15 s) and the clip's leading silence add up to about
  0.5 s from each cut to the first word.
- The `tail` (0.6 s) and the trailing silence add up to about 1.1–1.2 s from the last
  word to the next cut.
- The gap from one line's last word to the next line's first word is therefore
  1.6–1.7 s.

Whether those pauses work comedically is an editorial question for human review.
This report does not change the shot list.

### Subtitle guidance (guidance, not a verdict)

The values below come from the Netflix Timed Text Style Guide, retrieved
2026-10-21:

- English (USA) guide: reading speed *up to 20 characters per second* for adult
  programmes and *up to 17* for children's programmes.
- General Requirements: minimum event duration *5/6 second* and maximum *7 seconds*.
- Subtitle Timing Guidelines: at least 2 frames between events.

The guide does not define how characters are counted. This report counts Unicode
scalar values, including spaces and punctuation, of the line text.

| Shot | Caption | cps | ≤ 20 (adult) | ≤ 17 (children) | 0.833–7 s |
| --- | --- | --- | --- | --- | --- |
| doorway_wide | 3.650 s | 14.5 | within | within | within |
| boy_phone_medium | 2.625 s | 13.0 | within | within | within |
| dad_closeup | 2.575 s | 8.5 | within | within | within |

Measured against detected speech time only, doorway_wide would be 20.2 cps and
boy_phone_medium 18.4 cps. The captions stay readable under this guidance because
they remain up through the clip's silence. If clips are ever trimmed to the speech
span, re-check reading speed.

## Tests (no external WAVs)

These tests use synthetic signals generated at test time; `*.wav` is gitignored.

- `crates/motion-lipsync/src/timing_eval.rs`: envelope boundaries on the hop grid,
  gap bridging versus pauses, relative and absolute thresholds, click removal,
  non-finite samples, determinism, and the common-set merge. It also checks
  agreement with hand-computed values, open-interval bridging, and known
  onset/offset errors with mismatch run counts and durations, including a silent
  clip.
- `crates/motion-lipsync/src/english/tests.rs`: the span estimate rests outside the
  span and equals the shifted whole-span estimate; a full span equals the old track
  and identity; identity changes with the span; invalid spans are rejected.
- `crates/motion-voice/tests/text_visemes.rs`: a padded tone clip rests over its
  silence through `mouth_at`; the identity changes when the silence changes at
  equal length; a silent clip keeps the whole-clip estimate.
- `crates/motion-export/tests/night_room_reference.rs`: comic-timing arithmetic on
  the placeholder tones (cut gaps, `tail` hold, caption length equal to audio,
  cps, and that every row is present).

## Still open

- No manual annotation of these clips exists, so neither the text estimate nor
  Rhubarb has been measured against a reference. Perceptual lip-sync quality is
  unmeasured.
- Only one voice run with three lines was measured, which is not a corpus.
- Internal pause timing and shape choice of the text estimate were not changed.
- Comic timing, framing and voice still need human review. The cut and silence
  numbers above are inputs to that review, not a judgement.
