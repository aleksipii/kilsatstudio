# Exact-audio alignment cache

`motion_lipsync::cache` stores externally imported or manually authored phone
intervals without downloading a model or estimating timestamps. It does not
implement an automatic forced aligner or certify perceptual timing quality.

The SHA-256 key covers exact decoded interleaved f32 PCM bits, sample rate,
channels, frame count, transcript, provider name/version, configuration and manual
edit identity. Different audio samples (even one bit), text, model/dictionary
revision or manual edit gets a different file. Configuration is an opaque exact
string; callers must include every relevant setting and environment revision.
Manual edits must use `AlignmentSource::Manual` and a new nonempty edit identity
(prefer the SHA-256 of the complete edited timing document). Do not retain a
forced provenance claim after human correction.

API: create a `CacheKey::new`, then `AlignmentCache::new(directory).store` or
`.load`. Missing entries return `Ok(None)`; malformed JSON, checksum mismatch,
wrong identity, invalid phonemes, wrong duration or I/O failures return explicit
errors. Callers may offer deletion/reimport; corruption is never a silent hit.
Forced entries must match provider/version provenance. Paths use only a generated
64-digit hash; user text cannot create paths. Metadata fields are bounded to
64 KiB each and stored entries to 4 MiB, with at most 100,000 phonemes. Writes use
unique sibling files, sync, atomic rename and directory sync. Multiple writers
can safely replace the same key; readers see a complete previous or new entry.
This is a local trusted cache directory, not a security boundary for hostile
filesystem writers. SHA-256 detects corruption, not malicious tampering.

## Offline command example

```sh
cargo run -p motion-lipsync --example alignment_cache -- audio.wav timing.json cache-dir 'Hello there'
# Same command again reports a cache hit.
# Human-corrected timings must be explicitly marked:
cargo run -p motion-lipsync --example alignment_cache -- audio.wav edited.json cache-dir 'Hello there' --manual
```

`timing.json` uses the existing external-alignment import schema (see
`crates/motion-lipsync/tests/fixtures/external.json`). Its `audio.asset_id` must
be the exact decoded PCM hash exposed by `CacheKey::audio_sha256()`; WAV container
bytes are not this identity. The example prints the required hash before import.
The import document's SHA-256 is part of the edit identity, so changed timestamps
cannot reuse a previous entry. `--manual` changes provenance to manual; the
original tool/version still namespace the input revision. No audio is synthesized
or played, no aligner is executed and no network/download is performed.
