#!/usr/bin/env python3
"""Offline sample-clock mouth-shape evidence; never grants production acceptance."""
import argparse
import hashlib
import io
import json
import math
import os
from pathlib import Path
import stat
import struct
import sys
import wave

from acceptance import validate_review
from smoke_macos import publish_report

SHAPES = 'ABCDEFGHX'
JSON_LIMIT = 4 * 1024 * 1024
CANDIDATE_FIELDS = {'schema_version', 'source', 'tool_version', 'executable_sha256',
                    'pcm_sha256', 'transcript_sha256', 'recognizer', 'sample_rate',
                    'frame_count', 'cues'}
REFERENCE_FIELDS = {'schema_version', 'audio_sha256', 'provenance', 'sample_rate',
                    'frame_count', 'cues'}


def bounded_read(path, limit):
    # Nonblocking open prevents a hostile FIFO from hanging before fstat.
    descriptor = os.open(path, os.O_RDONLY | os.O_NONBLOCK)
    with os.fdopen(descriptor, 'rb') as source:
        if not stat.S_ISREG(os.fstat(source.fileno()).st_mode):
            raise ValueError('evidence input must be a regular file')
        data = source.read(limit + 1)
    if len(data) > limit:
        raise ValueError('evidence input exceeds size budget')
    return data


def digest(data):
    return hashlib.sha256(data).hexdigest()


def decode_json(data):
    def reject_constant(value):
        raise ValueError('nonfinite JSON number: ' + value)
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError('duplicate JSON key: ' + key)
            result[key] = value
        return result
    result = json.loads(data.decode('utf-8'), parse_constant=reject_constant,
                        object_pairs_hook=unique)
    if not isinstance(result, dict):
        raise ValueError('JSON evidence must be an object')
    return result


def wav_identity(data):
    if len(data) > 32 * 1024 * 1024:
        raise ValueError('WAV exceeds 32 MiB')
    # Reject inconsistent RIFF/chunk lengths before decoding.
    if len(data) < 12 or data[:4] != b'RIFF' or data[8:12] != b'WAVE' or struct.unpack_from('<I', data, 4)[0] + 8 != len(data):
        raise ValueError('invalid RIFF WAV length')
    position, seen = 12, set()
    while position < len(data):
        if position + 8 > len(data):
            raise ValueError('truncated WAV chunk')
        kind, size = data[position:position + 4], struct.unpack_from('<I', data, position + 4)[0]
        if kind == b'data' and (not size or size % 2 or size // 2 > 24000 * 300):
            raise ValueError('invalid PCM16 data length')
        position += 8 + size + size % 2
        if position > len(data) or (kind in (b'fmt ', b'data') and kind in seen):
            raise ValueError('invalid or duplicate WAV chunk')
        seen.add(kind)
    with wave.open(io.BytesIO(data), 'rb') as source:
        frames = source.getnframes()
        if (source.getnchannels(), source.getsampwidth(), source.getframerate(), source.getcomptype()) != (1, 2, 24000, 'NONE') or not 0 < frames <= 24000 * 300:
            raise ValueError('requires mono 24 kHz PCM16 WAV within five minutes')
        pcm = source.readframes(frames)
        if len(pcm) != frames * 2:
            raise ValueError('truncated WAV sample data')
    identity = hashlib.sha256(struct.pack('<IHQ', 24000, 1, frames))
    # Rust motion_audio reads PCM16 as (i16 / 32768.0) cast to f32.
    for (value,) in struct.iter_unpack('<h', pcm):
        identity.update(struct.pack('<f', value / 32768))
    return frames, identity.hexdigest()


def validate_cues(document, fields, frames, role):
    if set(document) != fields or type(document.get('schema_version')) is not int or document['schema_version'] != 1:
        raise ValueError(role + ' requires exact schema version 1 fields')
    if type(document['sample_rate']) is not int or document['sample_rate'] != 24000 or type(document['frame_count']) is not int or document['frame_count'] != frames:
        raise ValueError(role + ' sample clock differs from exact WAV')
    cues = document['cues']
    if not isinstance(cues, list) or len(cues) > 100000:
        raise ValueError(role + ' requires bounded cues')
    previous = 0
    for cue in cues:
        if not isinstance(cue, dict) or set(cue) != {'start_sample', 'end_sample', 'shape'}:
            raise ValueError('invalid mouth cue fields')
        start, end, shape = cue['start_sample'], cue['end_sample'], cue['shape']
        if type(start) is not int or type(end) is not int or not previous <= start < end <= frames or not isinstance(shape, str) or len(shape) != 1 or shape not in SHAPES:
            raise ValueError('invalid mouth cue interval or shape')
        previous = end
    return cues


def runs(cues, frames):
    result, previous = [], 0
    def append(start, end, shape):
        if start == end:
            return
        if result and result[-1][2] == shape:
            result[-1] = (result[-1][0], end, shape)
        else:
            result.append((start, end, shape))
    for cue in cues:
        append(previous, cue['start_sample'], 'X')
        append(cue['start_sample'], cue['end_sample'], cue['shape'])
        previous = cue['end_sample']
    append(previous, frames, 'X')
    return result


def compare(reference, candidate, frames):
    ref, cand = runs(reference, frames), runs(candidate, frames)
    confusion = {a: {b: 0 for b in SHAPES} for a in SHAPES}
    i = j = position = 0
    while position < frames:
        end = min(ref[i][1], cand[j][1])
        confusion[ref[i][2]][cand[j][2]] += end - position
        position = end
        if ref[i][1] == end:
            i += 1
        if cand[j][1] == end:
            j += 1
    matched = sum(confusion[a][a] for a in SHAPES)
    active = frames - confusion['X']['X']
    active_matched = matched - confusion['X']['X']
    boundary = dict(available=False, reason='collapsed run shape sequences differ')
    if [r[2] for r in ref] == [r[2] for r in cand]:
        errors = sorted(abs(a[1] - b[1]) for a, b in zip(ref[:-1], cand[:-1]))
        boundary = dict(available=True, boundaries=len(errors),
                        mean_ms=1000 * sum(errors) / (24000 * len(errors)) if errors else 0,
                        max_ms=1000 * max(errors, default=0) / 24000,
                        p95_ms=1000 * errors[math.ceil(.95 * len(errors)) - 1] / 24000 if errors else 0)
    return dict(total_samples=frames, matched_samples=matched, mismatched_samples=frames - matched,
                shape_agreement=matched / frames, active_union_samples=active,
                active_shape_agreement=active_matched / active if active else None,
                confusion_samples=confusion, boundary_error=boundary)


def evaluate(wav_bytes, smoke, reference, candidate, review, transcript):
    frames, pcm_hash = wav_identity(wav_bytes)
    audio_hash = digest(wav_bytes)
    if len(transcript) > 65536 or b'\0' in transcript:
        raise ValueError('transcript exceeds 64 KiB or contains NUL')
    transcript.decode('utf-8')
    if type(smoke.get('schema_version')) is not int or smoke['schema_version'] != 1 or smoke.get('output_sha256') != audio_hash:
        raise ValueError('smoke evidence must bind exact WAV hash')
    # Existing structural measurement semantics, operating on the captured bytes.
    peak, squares = 0, 0
    with wave.open(io.BytesIO(wav_bytes), 'rb') as source:
        while True:
            block = source.readframes(16384)
            if not block:
                break
            for (value,) in struct.iter_unpack('<h', block):
                peak = max(peak, abs(value))
                squares += value * value
    if not peak:
        raise ValueError('runtime output contains only silence')
    measured = dict(sample_rate=24000, channels=1, bits_per_sample=16,
                    sample_count=frames, duration_seconds=frames / 24000,
                    nonzero_signal=True, peak=peak / 32768,
                    rms=math.sqrt(squares / frames) / 32768)
    for key, value in measured.items():
        reported = smoke.get('audio', {}).get(key)
        if type(reported) is not type(value) or reported != value:
            raise ValueError('smoke audio measurement mismatch: ' + key)
    origin = smoke.get('provenance', {})
    if origin.get('kind') not in ('recorded_local_model', 'synthetic', 'stub'):
        raise ValueError('smoke requires explicit provenance')
    ref = validate_cues(reference, REFERENCE_FIELDS, frames, 'reference')
    if reference['audio_sha256'] != audio_hash or reference['provenance'] != 'manual':
        raise ValueError('reference must bind manual shapes to exact WAV')
    cand = validate_cues(candidate, CANDIDATE_FIELDS, frames, 'candidate')
    if candidate['source'] != 'estimated_rhubarb' or candidate['recognizer'] not in ('pocket_sphinx', 'phonetic') or not isinstance(candidate['tool_version'], str) or not candidate['tool_version'].strip() or len(candidate['tool_version'].encode('utf-8')) > 1024:
        raise ValueError('invalid estimated Rhubarb identity')
    for key in ('executable_sha256', 'pcm_sha256', 'transcript_sha256'):
        value = candidate[key]
        if not isinstance(value, str) or len(value) != 64 or any(c not in '0123456789abcdefABCDEF' for c in value):
            raise ValueError('invalid provenance SHA-256')
    if candidate['pcm_sha256'] != pcm_hash or candidate['transcript_sha256'] != digest(transcript):
        raise ValueError('candidate differs from exact decoded PCM or current transcript')
    validate_review(review, audio_hash)
    return dict(schema_version=1, audio_sha256=audio_hash, pcm_sha256=pcm_hash,
                transcript_sha256=digest(transcript), audio=measured, provenance=origin,
                candidate_source='estimated_rhubarb', candidate_provenance=candidate,
                metrics=compare(ref, cand, frames), human_review=review,
                production_acceptance='not_granted', provenance_verification='not_verified',
                limitations=['Manual reference and human observations are declarations, not independently verified.',
                             'Mouth shapes are estimated cues, not phonemes or forced alignment.',
                             'No automatic quality threshold or production acceptance is applied.'])


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('wav', 'smoke', 'reference', 'candidate', 'review', 'transcript', 'output'):
        parser.add_argument('--' + name, required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        inputs = {name: bounded_read(getattr(args, name),
                  32 * 1024 * 1024 if name == 'wav' else 65536 if name == 'transcript' else JSON_LIMIT)
                  for name in ('wav', 'smoke', 'reference', 'candidate', 'review', 'transcript')}
        report = evaluate(inputs['wav'], *(decode_json(inputs[name]) for name in
                         ('smoke', 'reference', 'candidate', 'review')), inputs['transcript'])
        report['input_sha256'] = {name: digest(data) for name, data in inputs.items()}
        publish_report(args.output, report)
        print(json.dumps(report, allow_nan=False))
        return 0
    except (OSError, ValueError, KeyError, TypeError, AttributeError, EOFError, wave.Error, RecursionError) as error:
        print(f'Viseme evidence rejected: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
