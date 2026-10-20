# English text to estimated mouth shapes

Module: `motion_lipsync::english`. Offline and deterministic. It turns any English
text into a viseme track **estimated from text alone**. It does not hear audio. Its
perceptual quality has not been measured (no annotated audio is available), and
structural tests (ordering, bounds, determinism) do not establish it.

## Pipeline

1. `normalize` writes text as lowercase spoken words plus pauses. Covers integers,
   thousands separators, decimals, years ("1999", "2024", "1990s"), ordinals,
   `$ £ €` amounts, percent, clock times, units after numbers, `& + = @ # /`,
   common abbreviations (Mr, Dr, etc, e.g., i.e.), dotted and all-caps acronyms
   (spelled by letter name unless CMUdict knows the word), accents, hyphens,
   apostrophes, repeated letters ("sooo" becomes "soo") and repeated punctuation
   (one pause). Commas, colons and dashes give a 120 ms rest; sentence ends and
   newlines give 300 ms; leading and trailing pauses are dropped. At most 20 000
   tokens.
2. Pronunciation per word, first match wins: project **override**, **lexicon**
   (CMUdict), **derived** (CMUdict base plus `'s`/plural), **fallback** rule-based
   letter-to-sound (`letter_to_sound`, hand-written, deterministic). Fallback words
   are flagged `low_confidence`.
3. `estimate_viseme_track(text, &overrides, audio_frames)` maps phonemes to visemes
   with the fixed duration model below.

## Report API

`pronunciation_report(text, &overrides) -> PronunciationReport` lists each spoken
word with `source` (`override | lexicon | derived | fallback`), ARPAbet phonemes and
`low_confidence`. `report.low_confidence_words()` gives the words to override.
Run it without a GUI:

```sh
cargo run -p motion-lipsync --example english_report -- "Yo, looksmaxxing costs 5 dollars" 48000
```

## MotionScript `pronunciations:`

```yaml
pronunciations:
  looksmaxxing: "L UH K S M AE K S IH NG"
```

Top-level map. Keys: 1..=64 lowercase letters or inner apostrophes. Values: 1..=32
ARPAbet phonemes separated by spaces (case-insensitive; stress digits accepted and
ignored). At most 1024 entries. Violations are `Validation` errors from
`motion_script::parse`. Convert with `Pronunciations::from_map`.

## Track format (for the acting layer)

`EstimatedVisemeTrack { source, sample_rate: 24000, frame_count, cues, fallback_words, time_scale }`
with `cues: Vec<VisemeCue { start_sample, end_sample, viseme }>` on the 24 kHz
dialogue clock, using the existing `motion_lipsync::Viseme`.

- Half-open intervals, contiguous from 0 to `frame_count`; pauses are `Rest`;
  adjacent equal visemes are merged; every cue has at least one sample.
- `source` is always `AlignmentSource::Estimated { tool: "kilsat-english-estimator", version: "1" }`.
  Never forced-aligned; only a real aligner may produce `Forced`.
- With `audio_frames` (1..=7 200 000, the existing 300 s bound), the model timeline
  is scaled so the cues cover exactly the audio. `time_scale` is audio length over
  model length; far from 1.0 means the estimate is a poor fit (lead-in silence,
  slow or fast reading). Without it the unscaled model length is used.
- `viseme_at_sample` evaluates a sample. `to_mouth_cues()` converts to the Rhubarb
  `MouthCue` letters used by `EstimatedVisemeAlignment` but is lossy: Sibilant, Sh
  and R become `C`, TongueTeeth becomes `H`. Prefer `VisemeCue`, which is the same
  shape of record with the full `Viseme` set instead of a Rhubarb letter.

## Phoneme to viseme table

| ARPAbet | Viseme | ms |
| --- | --- | --- |
| P B M | Closed | 60 / 60 / 70 |
| F V | TeethLip | 80 |
| TH DH | TongueTeeth | 80 |
| T D N L | TongueUp | 60 / 60 / 70 / 70 |
| S Z | Sibilant | 80 |
| SH ZH CH JH | Sh | 80 / 80 / 100 / 100 |
| R ER | R | 70 / 110 |
| AA AE AH AW AY | A | 110 / 110 / 70 / 140 / 140 |
| EH EY | E | 90 / 130 |
| IH IY Y | I | 70 / 100 / 70 |
| AO OW OY | O | 110 / 130 / 150 |
| UH UW W | U | 80 / 110 / 70 |
| K G NG HH | borrow neighbour | 60 / 60 / 70 / 60 |

K, G, NG and HH have no distinct shape here: they take the next shaped phoneme in
the word, else the previous one, else Rest. The table follows the existing IPA table
(`phoneme_to_viseme`) except that velars borrow a vowel instead of showing Rest, and
unstressed schwa is mapped with AH (stress is discarded). Durations are fixed
guesses, not fitted to speech data.

## Data and licence

`crates/motion-lipsync/data/cmudict.txt.gz` is derived from CMUdict
(cmusphinx/cmudict, `cmudict.dict`): first pronunciation per lowercase `[a-z']`
word, stress digits removed, sorted, compressed (about 0.8 MB, 124 911 words).
Regenerate with `scripts/build_cmudict_data.py`. CMUdict is Copyright (C) 1993-2015
Carnegie Mellon University, distributed under a BSD-2-Clause style licence; the full
text is in `crates/motion-lipsync/data/CMUDICT-LICENSE` and **must accompany any
distributed build** (source or binary, since the data is embedded). The licence text
was read from the upstream repository; formal legal sign-off is still a human task.
The decompressed text is held in memory once (about 3 MB) on first use.

## Limits

Single pronunciation per word (no heteronym choice), no stress or prosody, no
compound splitting, English only. Fallback guesses for names and slang are rough;
use overrides for important words. Lexicon coverage and fallback accuracy against
real speech are unmeasured.
