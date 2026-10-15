#!/usr/bin/env python3
"""Offline evidence checks; metrics and declarations never imply production acceptance."""
import argparse
import json
import math
from pathlib import Path
import sys
import wave

from smoke_macos import analyze_wav, publish_report, sha256


def read_json(path):
    if path.stat().st_size > 4 * 1024 * 1024:
        raise ValueError('JSON evidence exceeds 4 MiB')
    return json.loads(path.read_text(encoding='utf-8'))


def intervals(document, audio_hash, duration, role):
    if type(document.get('schema_version')) is not int or document['schema_version'] != 1 or document.get('audio_sha256') != audio_hash:
        raise ValueError(f'{role} must bind schema version 1 to the exact WAV hash')
    allowed = ('manual',) if role == 'reference' else ('manual', 'estimated', 'forced_aligned', 'synthetic')
    if document.get('provenance') not in allowed:
        raise ValueError(f'{role} has unsupported provenance')
    rows = document.get('intervals')
    if not isinstance(rows, list) or not rows:
        raise ValueError(f'{role} requires nonempty phoneme intervals')
    previous = 0.0
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get('phoneme'), str) or not row['phoneme'].strip():
            raise ValueError('each interval requires a phoneme')
        start, end = row.get('start'), row.get('end')
        if any(type(value) not in (int, float) or not math.isfinite(value) for value in (start, end)):
            raise ValueError('interval boundaries must be finite numbers')
        if not 0 <= previous <= start < end <= duration:
            raise ValueError('intervals must be ordered, disjoint and inside the audio')
        previous = end
    return rows


def evaluate(wav, smoke, reference, candidate, review):
    fingerprint = sha256(wav)
    if type(smoke.get('schema_version')) is not int or smoke['schema_version'] != 1 or smoke.get('output_sha256') != fingerprint:
        raise ValueError('smoke evidence does not bind the exact WAV hash')
    measured = analyze_wav(wav, smoke.get('audio', {}).get('sample_count'))
    for key, value in measured.items():
        reported = smoke.get('audio', {}).get(key)
        if type(reported) is not type(value) or reported != value:
            raise ValueError(f'smoke audio measurement mismatch: {key}')
    origin = smoke.get('provenance', {})
    kind = origin.get('kind')
    if kind not in ('recorded_local_model', 'synthetic', 'stub'):
        raise ValueError('smoke requires explicit recorded_local_model, synthetic or stub provenance')
    ref = intervals(reference, fingerprint, measured['duration_seconds'], 'reference')
    cand = intervals(candidate, fingerprint, measured['duration_seconds'], 'candidate')
    if len(ref) != len(cand) or [r['phoneme'] for r in ref] != [r['phoneme'] for r in cand]:
        raise ValueError('reference and candidate must contain the same ordered phonemes')
    errors = sorted(abs(a[key] - b[key]) for a, b in zip(ref, cand) for key in ('start', 'end'))
    metrics = dict(boundaries=len(errors), mean_ms=1000 * sum(errors) / len(errors),
                   max_ms=1000 * errors[-1], p95_ms=1000 * errors[math.ceil(.95 * len(errors)) - 1])
    if type(review.get('schema_version')) is not int or review['schema_version'] != 1 or review.get('audio_sha256') != fingerprint:
        raise ValueError('human review must bind schema version 1 to the exact WAV hash')
    for field in ('reviewer', 'reviewed_at', 'device', 'notes'):
        if not isinstance(review.get(field), str) or not review[field].strip():
            raise ValueError(f'human review requires nonempty {field}')
    for field in ('listening', 'device_playback', 'animation_sync'):
        if review.get(field) not in ('pass', 'fail', 'not_tested'):
            raise ValueError(f'human review requires explicit {field}: pass/fail/not_tested')
    return dict(schema_version=1, audio_sha256=fingerprint, audio=measured, provenance=origin,
                mac_arm64_recording=(kind == 'recorded_local_model' and origin.get('system') == 'Darwin'
                                     and origin.get('architecture') == 'arm64'),
                candidate_provenance=candidate['provenance'], boundary_error=metrics,
                human_review=review, production_acceptance='not_granted',
                limitations=['Provenance and human observations are declarations, not independently verified.',
                             'Boundary errors depend on the supplied manual reference.',
                             'No automatic quality threshold or production acceptance is applied.'])


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('wav', 'smoke', 'reference', 'candidate', 'review', 'output'):
        parser.add_argument('--' + name, required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        report = evaluate(args.wav, *(read_json(getattr(args, name)) for name in
                                      ('smoke', 'reference', 'candidate', 'review')))
        report['input_sha256'] = {name: sha256(getattr(args, name)) for name in
                                  ('smoke', 'reference', 'candidate', 'review')}
        publish_report(args.output, report)
        print(json.dumps(report, allow_nan=False))
        return 0
    except (OSError, ValueError, KeyError, TypeError, AttributeError, EOFError, wave.Error) as error:
        print(f'Acceptance evidence rejected: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
