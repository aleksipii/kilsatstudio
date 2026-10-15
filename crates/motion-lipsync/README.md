# External alignment import

`import_alignment_json(json, &AlignmentAudio)` imports measured phone intervals
from a separately operated offline forced aligner or its explicit JSON converter.
It never downloads assets, runs inference, derives timings from Kokoro tokens, or
stretches timings to fit a clip. Existing manual IPA `Alignment` remains supported.

See `tests/fixtures/external.json` for schema version 1. The fixture is synthetic
parser test data, not a quality measurement. Export phone intervals, not words.
The input must include the exact tool name/version, `alphabet` (`ipa` or
`arpabet`), and audio `asset_id`, `sample_rate`, and `frames`. Supply independently
known metadata for the actual audio to the import function; use a content digest
for `asset_id` in production. Metadata equality cannot authenticate an aligner or
prove that its producer used the claimed audio.

Duration is derived from frames / sample rate. Finite ordered non-overlapping
intervals must fit that duration; gaps evaluate to a closed/resting mouth. Unknown
JSON fields, unsupported schema versions, unsupported phones, empty tiers, and
mismatched audio fail with errors. Correct the export rather than guessing times.
English ARPAbet conversion accepts the standard inventory in `arpabet_to_ipa`,
vowel stress 0/1/2, and silence labels `sil`, `sp`, or empty string. Unknown labels
(such as `spn` or dictionary-specific allophones) require an explicit conversion.

An external aligner may emit IPA, ARPAbet, or dictionary-specific labels. Export
into this schema using the actual label inventory; this crate does not parse
TextGrid or assume every MFA model emits ARPAbet. Tool/version fields record
provenance and are not a quality certification. Review imported intervals against
the actual audio (including consonant closures, boundaries, pauses and seeking)
before accepting lip-sync quality. No real-audio quality validation is included.
