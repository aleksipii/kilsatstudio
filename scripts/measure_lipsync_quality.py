#!/usr/bin/env python3
"""Measure declared mouth annotations against exact local audio; never grant release acceptance."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import sys
import wave

# Reuse the existing sample-clock metric implementation rather than creating a second clock.
_RUNTIME = Path(__file__).resolve().parents[1] / 'services' / 'kokoro-runtime'
sys.path.insert(0, str(_RUNTIME))
from viseme_acceptance import bounded_read, compare, decode_json, wav_identity, SHAPES

JSON_LIMIT = 4 * 1024 * 1024
LANGUAGES = {'fi-fi', 'en-us', 'en-gb'}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def document_cues(document, *, frames, audio_hash, language, reference):
    fields = {'schema_version', 'audio_sha256', 'language', 'sample_rate', 'frame_count', 'cues'}
    fields |= {'provenance', 'annotator'} if reference else {'source', 'tool'}
    if not isinstance(document, dict) or set(document) != fields or type(document['schema_version']) is not int or document['schema_version'] != 1:
        raise ValueError('mouth document requires exact schema version 1 fields')
    if document['audio_sha256'] != audio_hash or document['language'] != language:
        raise ValueError('mouth document differs from exact WAV or selected language')
    if type(document['sample_rate']) is not int or document['sample_rate'] != 24000 or type(document['frame_count']) is not int or document['frame_count'] != frames:
        raise ValueError('mouth document sample clock differs from exact WAV')
    if reference:
        if document['provenance'] != 'manual' or not isinstance(document['annotator'], str) or not 1 <= len(document['annotator'].strip()) <= 256:
            raise ValueError('reference requires named, declared manual annotation')
    elif document['source'] not in ('estimated', 'text_estimated', 'estimated_rhubarb', 'forced', 'manual') or not isinstance(document['tool'], str) or not 1 <= len(document['tool'].strip()) <= 256:
        raise ValueError('candidate requires explicit source/tool provenance')
    cues = document['cues']
    if not isinstance(cues, list) or len(cues) > 100000:
        raise ValueError('mouth cues exceed bounded list')
    previous = 0
    for cue in cues:
        if not isinstance(cue, dict) or set(cue) != {'start_sample', 'end_sample', 'shape'}:
            raise ValueError('mouth cue requires exact fields')
        start, end, shape = cue['start_sample'], cue['end_sample'], cue['shape']
        if type(start) is not int or type(end) is not int or not previous <= start < end <= frames or not isinstance(shape, str) or len(shape) != 1 or shape not in SHAPES:
            raise ValueError('invalid mouth cue interval or shape')
        previous = end
    return cues


def measure(wav_bytes, language, candidate=None, reference=None, inference=None, review=None,
            min_active_agreement=.75, max_boundary_p95_ms=80.0):
    if language not in LANGUAGES:
        raise ValueError('language must be fi-fi, en-us or en-gb')
    if isinstance(min_active_agreement, bool) or not isinstance(min_active_agreement, (int, float)) or not math.isfinite(min_active_agreement) or not 0 <= min_active_agreement <= 1:
        raise ValueError('active agreement threshold must be finite within 0..1')
    if isinstance(max_boundary_p95_ms, bool) or not isinstance(max_boundary_p95_ms, (int, float)) or not math.isfinite(max_boundary_p95_ms) or not 0 <= max_boundary_p95_ms <= 1000:
        raise ValueError('boundary threshold must be finite within 0..1000 ms')
    frames, pcm_hash = wav_identity(wav_bytes)
    audio_hash = digest(wav_bytes)
    origin = {'kind': 'unknown'}
    if inference is not None:
        if not isinstance(inference, dict) or type(inference.get('schema_version')) is not int or inference['schema_version'] != 1 or inference.get('output_sha256') != audio_hash or inference.get('language') != language:
            raise ValueError('inference provenance must bind exact WAV and language')
        audio = inference.get('audio', {})
        if type(audio.get('sample_rate')) is not int or audio['sample_rate'] != 24000 or type(audio.get('sample_count')) is not int or audio['sample_count'] != frames:
            raise ValueError('inference sample clock differs from WAV')
        origin = inference.get('provenance', {})
        if not isinstance(origin, dict) or origin.get('kind') not in ('recorded_local_model', 'recorded_human', 'synthetic', 'stub', 'unknown'):
            raise ValueError('inference requires explicit bounded audio origin')
    if review is not None:
        if not isinstance(review, dict) or set(review) != {'audio_sha256', 'reviewer', 'note', 'accepted'} or review['audio_sha256'] != audio_hash or type(review['accepted']) is not bool or not all(isinstance(review[key], str) and 1 <= len(review[key].strip()) <= 4096 for key in ('reviewer', 'note')):
            raise ValueError('review requires exact audio hash, named reviewer, note and boolean decision')
    cand = None if candidate is None else document_cues(candidate, frames=frames, audio_hash=audio_hash, language=language, reference=False)
    ref = None if reference is None else document_cues(reference, frames=frames, audio_hash=audio_hash, language=language, reference=True)
    metrics = None if ref is None or cand is None else compare(ref, cand, frames)
    numeric_pass = False
    if metrics is not None:
        active = metrics['active_shape_agreement']
        boundary = metrics['boundary_error']
        numeric_pass = active is not None and active >= min_active_agreement and boundary['available'] and boundary['p95_ms'] <= max_boundary_p95_ms
    if origin['kind'] in ('synthetic', 'stub'):
        state = 'synthetic_only'
    elif metrics is None or metrics['active_shape_agreement'] is None or origin['kind'] == 'unknown':
        state = 'unmeasured'
    elif not numeric_pass or review is not None and not review['accepted']:
        state = 'fail'
    elif review is None:
        state = 'awaiting_review'
    else:
        state = 'reviewed_declaration'
    return dict(schema_version=1, state=state, audio_sha256=audio_hash, pcm_sha256=pcm_hash,
                language=language, sample_rate=24000, frame_count=frames,
                provenance=origin, metrics=metrics, metric_gate_passed=bool(numeric_pass),
                thresholds=dict(min_active_shape_agreement=min_active_agreement, max_boundary_p95_ms=max_boundary_p95_ms),
                human_review=review, production_acceptance='not_granted',
                limitations=['Audio origin, manual annotation and human review are declarations, not independently audited facts.',
                             'Mouth cue times are line-local 24 kHz samples, not video frames.',
                             'Orthographic/recognizer estimates are not forced alignment or phoneme accuracy.',
                             'Numeric thresholds never grant perceptual, rights, hardware or production acceptance.'])


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--wav', type=Path, required=True)
    parser.add_argument('--language', choices=sorted(LANGUAGES), required=True)
    for name in ('candidate', 'reference', 'inference-report', 'review'):
        parser.add_argument('--' + name, type=Path)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--min-active-agreement', type=float, default=.75)
    parser.add_argument('--max-boundary-p95-ms', type=float, default=80.0)
    args = parser.parse_args(argv)
    try:
        if args.output.exists():
            raise ValueError('output must be a new report; evidence is never overwritten')
        inputs = {}
        for name in ('wav', 'candidate', 'reference', 'inference_report', 'review'):
            path = getattr(args, name)
            if path is None:
                continue
            if path.is_symlink():
                raise ValueError('evidence input must not be a symlink')
            inputs[name] = bounded_read(path, 32 * 1024 * 1024 if name == 'wav' else JSON_LIMIT)
        report = measure(inputs['wav'], args.language,
                         *(decode_json(inputs[name]) if name in inputs else None for name in ('candidate', 'reference', 'inference_report', 'review')),
                         args.min_active_agreement, args.max_boundary_p95_ms)
        report['input_sha256'] = {name: digest(data) for name, data in inputs.items()}
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with args.output.open('x', encoding='utf-8') as output:
            json.dump(report, output, ensure_ascii=False, indent=2, allow_nan=False)
            output.write('\n')
        print(json.dumps(report, ensure_ascii=False, allow_nan=False))
        return 0
    except (OSError, ValueError, TypeError, KeyError, AttributeError, EOFError, wave.Error, RecursionError) as error:
        print(f'Lip-sync evidence rejected: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
