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
unique sibling files, sync, atomic no-replace hard-link publication and directory
sync. Identical competing writes succeed; conflicting timings under the same key
are rejected, including concurrent writers. Use a new edit identity for changes.
Filesystems must support sibling hard links; unsupported publication is an explicit
error rather than a replacing fallback.
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

## Desktop exact-audio envelopes

`BoundAlignment` contains `schema_version: 1`, a complete serialized `CacheKey`,
and optional `alignment`. `AlignmentInput` accepts this strict object or a legacy
bare `Alignment`. The resolver reconstructs the complete key from the actual
decoded dialogue and current screenplay text. A different PCM bit or text is an
error even when audio duration is identical. Provider/version/configuration/edit
identity remain exact cache inputs; forced provenance must match its provider.

With an alignment payload, resolve verifies and imports it or reports a matching
cache hit. Without the payload, it requests that exact cache entry; a miss leaves
the mouth at rest. Corruption and conflicting timings report errors. Legacy
inputs preserve the previous one-sample duration tolerance and are labeled
unbound; they are never stored in this exact-audio cache. Blank input has no
alignment and leaves the mouth at rest.

Create an envelope from an existing externally measured import:

```sh
cargo run -p motion-lipsync --example bound_alignment -- audio.wav measured-import.json 'Hello there' bound.json
```

The measured import must already name the exact PCM identity described above.
The command refuses to overwrite output. Paste the resulting JSON into the
selected-line field, or map dialogue IDs to envelopes in the scene field. Both
workers use `KokoroConfig.cache_directory/alignments`; status distinguishes
import, hit, miss, legacy-unbound and absent timing. Changing/clearing timing
invalidates its preview. Parsing (at most 100 KB per desktop field), decoding,
hashing and filesystem work remain off the event loop.

These checks establish data pairing and structural provenance, not accurate
phoneme boundaries. No automatic aligner is run and no real-model, perceptual or
physical Mac synchronization gate is completed by this integration.
