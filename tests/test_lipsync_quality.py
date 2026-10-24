"""Synthetic fixtures test measurement semantics, not real Finnish mouth quality."""
import contextlib
import copy
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import struct
import tempfile
import unittest
import wave

SPEC = importlib.util.spec_from_file_location('measure_lipsync_quality', Path(__file__).resolve().parents[1] / 'scripts' / 'measure_lipsync_quality.py')
quality = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(quality)


def fixture(kind='recorded_local_model'):
    stream = io.BytesIO()
    with wave.open(stream, 'wb') as out:
        out.setnchannels(1); out.setsampwidth(2); out.setframerate(24000)
        out.writeframes(struct.pack('<24000h', *([1000] * 24000)))
    audio = stream.getvalue(); digest = hashlib.sha256(audio).hexdigest()
    base = dict(schema_version=1, audio_sha256=digest, language='fi-fi', sample_rate=24000,
                frame_count=24000, cues=[dict(start_sample=2400, end_sample=12000, shape='A'), dict(start_sample=12000, end_sample=21600, shape='D')])
    candidate = dict(base, source='text_estimated', tool='synthetic-test-estimator')
    reference = dict(base, provenance='manual', annotator='synthetic-test-annotation')
    inference = dict(schema_version=1, output_sha256=digest, language='fi-fi',
                     audio=dict(sample_rate=24000, sample_count=24000), provenance=dict(kind=kind, note='synthetic unit fixture testing a declared origin'))
    return audio, candidate, reference, inference


class LipSyncQualityTests(unittest.TestCase):
    def test_without_annotation_or_origin_quality_is_unmeasured(self):
        audio, candidate, reference, inference = fixture()
        report = quality.measure(audio, 'fi-fi', candidate, None, inference)
        self.assertEqual(report['state'], 'unmeasured')
        self.assertIsNone(report['metrics'])
        report = quality.measure(audio, 'fi-fi', candidate, reference)
        self.assertEqual(report['state'], 'unmeasured')
        self.assertEqual(report['production_acceptance'], 'not_granted')

    def test_numeric_agreement_is_awaiting_real_review_not_production(self):
        audio, candidate, reference, inference = fixture()
        report = quality.measure(audio, 'fi-fi', candidate, reference, inference)
        self.assertEqual(report['state'], 'awaiting_review')
        self.assertEqual(report['metrics']['active_shape_agreement'], 1)
        self.assertEqual(report['metrics']['boundary_error']['p95_ms'], 0)
        self.assertTrue(report['metric_gate_passed'])
        self.assertEqual(report['production_acceptance'], 'not_granted')

    def test_synthetic_metrics_never_qualify_as_real_quality(self):
        audio, candidate, reference, inference = fixture('synthetic')
        report = quality.measure(audio, 'fi-fi', candidate, reference, inference)
        self.assertEqual(report['state'], 'synthetic_only')
        self.assertTrue(report['metric_gate_passed'])
        self.assertEqual(report['production_acceptance'], 'not_granted')

    def test_language_wav_and_sample_identity_mismatches_are_rejected(self):
        audio, candidate, reference, inference = fixture()
        for key, value in [('audio_sha256', '0'*64), ('language', 'en-us'), ('frame_count', 23999), ('sample_rate', True)]:
            changed = copy.deepcopy(candidate); changed[key] = value
            with self.assertRaises(ValueError): quality.measure(audio, 'fi-fi', changed, reference, inference)
        changed = copy.deepcopy(inference); changed['output_sha256'] = '0'*64
        with self.assertRaises(ValueError): quality.measure(audio, 'fi-fi', candidate, reference, changed)

    def test_boundary_error_and_negative_human_review_fail(self):
        audio, candidate, reference, inference = fixture()
        changed = copy.deepcopy(candidate)
        changed['cues'][0]['end_sample'] += 2400
        changed['cues'][1]['start_sample'] += 2400
        report = quality.measure(audio, 'fi-fi', changed, reference, inference, max_boundary_p95_ms=80)
        self.assertEqual(report['state'], 'fail')
        self.assertEqual(report['metrics']['boundary_error']['p95_ms'], 100)
        review = dict(audio_sha256=reference['audio_sha256'], reviewer='fixture', note='reject', accepted=False)
        self.assertEqual(quality.measure(audio, 'fi-fi', candidate, reference, inference, review)['state'], 'fail')

    def test_invalid_json_cues_and_nonfinite_thresholds_are_rejected(self):
        audio, candidate, reference, inference = fixture()
        for cue in [dict(start_sample=True, end_sample=5, shape='A'), dict(start_sample=2, end_sample=1, shape='A'), dict(start_sample=0, end_sample=24001, shape='A'), dict(start_sample=0, end_sample=5, shape='?')]:
            changed = copy.deepcopy(candidate); changed['cues'] = [cue]
            with self.assertRaises(ValueError): quality.measure(audio, 'fi-fi', changed, reference, inference)
        with self.assertRaises(ValueError): quality.measure(audio, 'fi-fi', min_active_agreement=float('nan'))
        with self.assertRaises(ValueError): quality.decode_json(b'{"schema_version":1,"schema_version":1}')

    def test_silent_audio_cannot_satisfy_the_quality_gate(self):
        audio, candidate, reference, inference = fixture()
        silent = audio[:44] + bytes(len(audio)-44)
        fingerprint = hashlib.sha256(silent).hexdigest()
        candidate['audio_sha256'] = reference['audio_sha256'] = inference['output_sha256'] = fingerprint
        report = quality.measure(silent, 'fi-fi', candidate, reference, inference)
        self.assertEqual(report['state'], 'unmeasured')
        self.assertFalse(report['nonzero_signal'])
        self.assertFalse(report['metric_gate_passed'])

    def test_cli_writes_new_unmeasured_report_and_preserves_existing_evidence(self):
        audio, *_ = fixture()
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); wav = root/'input.wav'; wav.write_bytes(audio); output = root/'report.json'
            args = ['--wav', str(wav), '--language', 'fi-fi', '--output', str(output)]
            with contextlib.redirect_stdout(io.StringIO()): self.assertEqual(quality.main(args), 0)
            before = output.read_bytes()
            self.assertEqual(json.loads(before)['state'], 'unmeasured')
            with contextlib.redirect_stderr(io.StringIO()): self.assertEqual(quality.main(args), 1)
            self.assertEqual(output.read_bytes(), before)
