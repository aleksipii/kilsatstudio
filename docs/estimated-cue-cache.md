# Saved estimated mouth timing

The Rhubarb panel can save and explicitly reload an estimated result. This is
an immutable result archive: it reuses a chosen past result without running a
recognizer. It does not predict the result of a new execution or automatically
select a result from the audio alone. Rhubarb's external recognizer resources
and shared libraries are not yet fully fingerprinted for engine memoization.

## Native workflow

1. Prepare the selected dialogue line from local Kokoro or an imported WAV.
2. Configure Rhubarb and click **Estimate prepared line**. After exact PCM/text
   validation and the existing 100 KB editor/map limits, the worker saves the
   result under `KokoroConfig.cache_directory/estimated-alignments`.
3. Copy the displayed **Cache key** using **Copy key**. The panel displays the
   original tool version and recognizer. A save failure is shown explicitly;
   it does not discard otherwise usable timing or imply successful persistence.
4. To reuse the saved result, prepare the same audio and exact dialogue text,
   enter the key in **Saved timing key**, and click **Load cached timing**.
   This action does not require or execute Rhubarb. Different decoded PCM or
   text is an error even when recording duration is unchanged.
5. Review the loaded timing and its original provenance, then explicitly click
   **Apply estimated timing**. Loading does not edit the document or apply cues.

The key remains in the panel for reuse, but is not automatically persisted in
`.kilsat` or restored as a project selection on restart. Retain the key and its
cache entry when keeping a revision. There is no automatic discovery, newest
result selection, eviction or project transfer. Cached estimated cues remain
separate from the existing phoneme alignment cache; downstream preparation
labels them as estimated imports and does not claim new recognizer execution.

## Identity and storage

`motion_lipsync::estimated_cache::EstimatedCache` provides `store` and `load`.
The lowercase 64-character key hashes a versioned domain and canonical complete
`EstimatedVisemeAlignment`: exact PCM/text hashes, executable hash, tool version,
recognizer, schema, sample rate/count and every shape interval. Changed cues or
provenance create a different key. Original metadata survives a load; it is not
replaced with the currently configured tool path or recognizer.

Entries are schema-versioned JSON, bounded to 4 MiB and validated as estimated
mouth shapes against the prepared audio and text. Invalid keys, wrong schema,
corruption, mismatched identities, oversized files and ordinary non-file/symlink
entries are rejected. The archive is ordinary local application storage.
Payload integrity does not attest to the tool/resources used to create it.

Writes reserve an exclusive staging file, finish it, then publish a hard link
without replacing an existing entry. Identical concurrent publishers are
idempotent. Corrupt existing files are reported and preserved. Cleanup removes
only staging files created by that operation; the archive does not repair or
delete unknown entries. Filesystems must support that publication operation;
otherwise the panel reports that saving failed.

Worker reads, hashing and publication stay off the UI thread. Queue bounds,
cancellation, stale prepared-line identities, scene-map merge checks and explicit
Apply are shared with the existing native alignment path. A cancelled operation
can leave an already completed immutable entry, but cannot apply it implicitly.

Synthetic tests establish identity, storage and UI behavior, not real Rhubarb
compatibility or mouth quality. Use the [quality evaluator](viseme-quality.md)
for comparisons with manual references. Automatic fresh-engine cache lookup
with verified resource/environment identity, project timing persistence, real
corpus quality, physical Mac synchronization and final licenses remain open.
