"""Synthetic shape evidence verifies metrics and identity, never real mouth quality."""
import copy
import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import struct
import sys
import tempfile
import unittest
import wave

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import viseme_acceptance as acceptance
from smoke_macos import analyze_wav


def wav_bytes(values):
    output = io.BytesIO()
    with wave.open(output, 'wb') as target:
        target.setparams((1, 2, 24000, 0, 'NONE', 'not compressed'))
        target.writeframes(struct.pack('<' + str(len(values)) + 'h', *values))
    return output.getvalue()


def cue(start, end, shape):
    return dict(start_sample=start, end_sample=end, shape=shape)


class VisemeAcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.wav = wav_bytes([1000] * 2400)
        path = self.root / 'audio.wav'
        path.write_bytes(self.wav)
        fingerprint = acceptance.digest(self.wav)
        self.smoke = dict(schema_version=1, output_sha256=fingerprint,
                          audio=analyze_wav(path, 2400), provenance=dict(kind='synthetic'))
        self.reference = dict(schema_version=1, audio_sha256=fingerprint, provenance='manual',
                              sample_rate=24000, frame_count=2400,
                              cues=[cue(100, 1000, 'D'), cue(1000, 2000, 'F')])
        self.text = b'Hello fixture.'
        self.candidate = dict(schema_version=1, source='estimated_rhubarb', tool_version='synthetic stub',
                              executable_sha256='a' * 64, pcm_sha256=acceptance.wav_identity(self.wav)[1],
                              transcript_sha256=acceptance.digest(self.text), recognizer='pocket_sphinx',
                              sample_rate=24000, frame_count=2400,
                              cues=copy.deepcopy(self.reference['cues']))
        self.review = dict(schema_version=1, audio_sha256=fingerprint, reviewer='Fixture',
                           reviewed_at='2026-10-18', device='Synthetic', notes='Not listened',
                           listening='not_tested', device_playback='not_tested', animation_sync='not_tested')

    def evaluate(self):
        return acceptance.evaluate(self.wav, self.smoke, self.reference, self.candidate, self.review, self.text)

    def test_rust_pcm_identity_golden(self):
        self.assertEqual(acceptance.wav_identity(wav_bytes([-32768, -1, 0, 1, 32767]))[1],
                         '161dcb72c4cbb1b6ba7ce5a0520fdb05d1446c8f84a4246fc24f03504984a855')

    def test_exact_shapes_and_unverified_acceptance(self):
        report = self.evaluate()
        self.assertEqual(report['metrics']['shape_agreement'], 1)
        self.assertEqual(report['metrics']['active_shape_agreement'], 1)
        self.assertEqual(report['metrics']['mismatched_samples'], 0)
        self.assertEqual(report['production_acceptance'], 'not_granted')
        self.assertEqual(report['provenance_verification'], 'not_verified')
        self.assertEqual(report['metrics']['confusion_samples']['X']['X'], 500)

    def test_gaps_rest_and_shape_confusion(self):
        self.candidate['cues'] = [cue(200, 1000, 'D'), cue(1000, 1800, 'B')]
        metrics = self.evaluate()['metrics']
        self.assertEqual(metrics['mismatched_samples'], 1100)
        self.assertEqual(metrics['confusion_samples']['D']['X'], 100)
        self.assertEqual(metrics['confusion_samples']['F']['B'], 800)
        self.assertEqual(metrics['active_union_samples'], 1900)
        self.assertAlmostEqual(metrics['active_shape_agreement'], 800 / 1900)
        self.assertFalse(metrics['boundary_error']['available'])
        self.assertIn('sequences differ', metrics['boundary_error']['reason'])

    def test_collapsed_segmentation_and_shifted_boundaries(self):
        self.candidate['cues'] = [cue(100, 500, 'D'), cue(500, 1100, 'D'), cue(1100, 2000, 'F')]
        metrics = self.evaluate()['metrics']
        self.assertEqual(metrics['mismatched_samples'], 100)
        boundary = metrics['boundary_error']
        self.assertTrue(boundary['available'])
        self.assertEqual(boundary['boundaries'], 3)
        self.assertAlmostEqual(boundary['max_ms'], 100 / 24)
        self.assertAlmostEqual(boundary['mean_ms'], 100 / 72)

    def test_all_rest_has_no_active_metric(self):
        self.reference['cues'] = []
        self.candidate['cues'] = [cue(0, 2400, 'X')]
        metrics = self.evaluate()['metrics']
        self.assertEqual(metrics['shape_agreement'], 1)
        self.assertIsNone(metrics['active_shape_agreement'])
        self.assertEqual(metrics['boundary_error']['boundaries'], 0)

    def test_exact_pcm_and_transcript_bindings(self):
        for field in ('pcm_sha256', 'transcript_sha256'):
            saved = self.candidate[field]
            self.candidate[field] = '0' * 64
            with self.assertRaisesRegex(ValueError, 'exact decoded PCM'):
                self.evaluate()
            self.candidate[field] = saved
        self.text += b' changed'
        with self.assertRaises(ValueError):
            self.evaluate()

    def test_hostile_candidate_fields(self):
        original = copy.deepcopy(self.candidate)
        for patch in (dict(schema_version=True), dict(sample_rate=True), dict(frame_count=2400.0),
                      dict(recognizer='unknown'), dict(source='forced_aligned'), dict(extra=True),
                      dict(tool_version='x' * 1025), dict(executable_sha256='g' * 64)):
            self.candidate = dict(original, **patch)
            with self.assertRaises(ValueError):
                self.evaluate()
        for patch in (dict(start_sample=True), dict(end_sample=float('nan')),
                      dict(shape='Z'), dict(end_sample=2401), dict(extra=1)):
            self.candidate = copy.deepcopy(original)
            self.candidate['cues'][0].update(patch)
            with self.assertRaises(ValueError):
                self.evaluate()

    def test_human_smoke_and_reference_validation(self):
        for target, key, value in ((self.review, 'audio_sha256', '0' * 64),
                                   (self.smoke['audio'], 'sample_count', True),
                                   (self.reference, 'provenance', 'estimated')):
            saved = target[key]
            target[key] = value
            with self.assertRaises(ValueError):
                self.evaluate()
            target[key] = saved

    def test_wav_and_text_bounds(self):
        for data in (b'bad', self.wav[:-1], b'x' * (32 * 1024 * 1024 + 1)):
            with self.assertRaises(ValueError):
                acceptance.wav_identity(data)
        odd = bytearray(self.wav)
        data_offset = odd.index(b'data')
        struct.pack_into('<I', odd, data_offset + 4, 4799)
        with self.assertRaisesRegex(ValueError, 'PCM16 data length'):
            acceptance.wav_identity(bytes(odd))
        for text in (b'x' * 65537, b'bad\0text', b'\xff'):
            self.text = text
            with self.assertRaises(ValueError):
                self.evaluate()

    def test_json_and_file_limits(self):
        for data in (b'{"a":NaN}', b'{"a":1,"a":2}', b'[]'):
            with self.assertRaises(ValueError):
                acceptance.decode_json(data)
        path = self.root / 'large.json'
        path.write_bytes(b'x' * (acceptance.JSON_LIMIT + 1))
        with self.assertRaisesRegex(ValueError, 'budget'):
            acceptance.bounded_read(path, acceptance.JSON_LIMIT)
        if hasattr(os, 'mkfifo'):
            fifo = self.root / 'fifo'
            os.mkfifo(fifo)
            with self.assertRaisesRegex(ValueError, 'regular'):
                acceptance.bounded_read(fifo, 100)

    def test_cli_publishes_exclusively_and_hashes_captured_inputs(self):
        values = dict(wav=self.wav, transcript=self.text,
                      **{name: json.dumps(getattr(self, name)).encode() for name in
                         ('smoke', 'reference', 'candidate', 'review')})
        args = []
        for name, data in values.items():
            path = self.root / name
            path.write_bytes(data)
            args.extend(['--' + name, str(path)])
        output = self.root / 'report.json'
        args.extend(['--output', str(output)])
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(acceptance.main(args), 0)
            saved = output.read_bytes()
            self.assertEqual(acceptance.main(args), 1)
        self.assertEqual(output.read_bytes(), saved)
        report = json.loads(saved)
        self.assertEqual(report['input_sha256'], {name: hashlib.sha256(data).hexdigest() for name, data in values.items()})
        (self.root / 'transcript').write_bytes(b'changed')
        with contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(acceptance.main(args), 1)
        self.assertEqual(output.read_bytes(), saved)


if __name__ == '__main__':
    unittest.main()
