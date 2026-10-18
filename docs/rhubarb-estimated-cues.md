# Opt-in offline estimated mouth cues

`motion_lipsync::rhubarb::align` and the `rhubarb_align` example execute an
explicitly configured, existing local Rhubarb executable. Nothing is installed
or downloaded, and no real Rhubarb inference or quality evaluation was performed
while implementing this adapter. Run the blocking API on a bounded worker; the
generated JSON can be pasted into the existing selected-line timing input. For
scene or project preparation, place it under the stable global dialogue ID in
the existing JSON map: `{"0": { ...estimated document... }}`. All three paths
validate exact decoded PCM and current dialogue text before preparing a mouth
track. No estimated data is inserted into the phoneme alignment cache.

```sh
cargo run -p motion-lipsync --example rhubarb_align -- \
  /absolute/path/to/rhubarb line.wav transcript.txt estimated-cues.json
```

The CLI chooses English `pocketSphinx`; the library also permits the explicit
`phonetic` recognizer. Both always use JSON, `GHX` extended shapes and two threads.
The CLI refuses to replace an existing output. Cancellation is available to
library callers; the example relies on its process lifetime and the bounded
execution deadline.

## Data and provenance

Output has a separate `EstimatedVisemeAlignment` schema; it cannot be mistaken
for the existing phoneme `Alignment` or `Forced` source. It stores exact decoded
PCM identity (SHA-256 over little-endian sample rate, channels, frame count and
f32 sample bits), exact UTF-8 transcript identity, recognizer, executable SHA-256,
reported version, schema version, sample count and sample-based mouth cues.
Configuration for schema 1 fixes JSON/GHX/two threads and the mapping below.
Executable identity is checked again after execution; the input snapshot is also
checked for modification. This does not establish identity or licensing of
Rhubarb's separate recognizer resources or shared libraries. Managed installation,
resource manifests, estimated-cue caching and redistribution remain open.

| Rhubarb shape | Studio viseme |
| --- | --- |
| A | Closed |
| B | I |
| C | E |
| D | A |
| E | O |
| F | U |
| G | TeethLip |
| H | TongueUp |
| X | Rest |

This is a deliberately lossy mouth-shape mapping, not an inferred phoneme label.
The original shape is retained for future drawing-specific mapping. Sample
intervals are half-open. Gaps and samples after the recording return Rest.

Upstream truncates reported duration to centiseconds. The adapter checks that
value against the exact decoded PCM frame count, rounds cue boundaries to the
24 kHz sample clock and leaves the final sub-centisecond tail at Rest. It rejects
non-finite, unordered, overlapping, unknown or sample-collapsing cues.

## Resource and failure limits

- Explicit absolute executable path; maximum binary size 128 MiB. No PATH search.
- Input WAV maximum 32 MiB; RIFF sizes/chunks checked before decoding; mono
  24 kHz PCM16 or float32 only, nonempty and at most five minutes.
- Transcript maximum 64 KiB, without NUL; JSON maximum 4 MiB and 100,000 cues.
- Version capture maximum 4 KiB and five seconds; analysis deadline 120 seconds.
- Child stdin and stderr are disconnected. A bounded pipe reader captures at most
  the output cap plus one byte and closes the pipe on overflow; only successful
  bounded output reaches the staging file. Cancellation, timeout or overflow
  kills and reaps the direct child. This is an adapter for a user-approved local
  executable, not a sandbox for adversarial programs or their descendants.
- Private staging directory on Unix, exclusive file creation and owned cleanup
  on every returned success/error path. Abnormal termination of the whole host
  process can leave staging files, as with other local tools.

Synthetic process fixtures test execution, provenance changes, limits, cancellation
and serialization only. Real-engine compatibility across supported platforms,
licensed recognizer-resource identity, real-audio cue accuracy, user corrections,
Mac device synchronization, project timing persistence and estimated-cue caching
are still open. PR #31 added explicit native execution and Apply controls for an
already prepared line; see [native alignment](native-alignment.md).
Selected-line/scene/sequence
preparation now imports these cues through one typed input path; preview and
export share their exact sample intervals. UI labels imported cues as estimated,
uncached and quality not measured. Export manifests use an explicit
`estimated` provenance source, preserving existing manual/forced JSON support.
Do not mark automatic alignment or perceptual acceptance complete from these tests.

## Primary-source contract

The official upstream [README](https://github.com/DanielSWolf/rhubarb-lip-sync#command-line-options)
was checked on 2026-10-16 for `--version`, `--exportFormat json`, `--recognizer`,
`--dialogFile`, `--extendedShapes`, `--threads`, stdout output and JSON metadata.
Its [mouth-shape descriptions](https://github.com/DanielSWolf/rhubarb-lip-sync#mouth-shapes)
and [JSON format](https://github.com/DanielSWolf/rhubarb-lip-sync#json-format-json)
explain the mapping and centisecond duration truncation. These references establish
the documented protocol, not local installation, a pinned distribution, engine
execution or measured quality.
