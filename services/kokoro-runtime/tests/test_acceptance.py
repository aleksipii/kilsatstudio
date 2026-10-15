"""Synthetic fixtures verify evidence handling, never speech quality."""
import copy
import importlib.util
from pathlib import Path
import struct
import sys
import tempfile
import unittest
import wave

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
spec = importlib.util.spec_from_file_location('acceptance', ROOT / 'acceptance.py')
acceptance = importlib.util.module_from_spec(spec)
spec.loader.exec_module(acceptance)


class AcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.wav = Path(self.directory.name) / 'fixture.wav'
        with wave.open(str(self.wav), 'wb') as out:
            out.setparams((1, 2, 24000, 0, 'NONE', 'not compressed'))
            out.writeframes(struct.pack('<24000h', *([1000] * 24000)))
        self.hash = acceptance.sha256(self.wav)
        self.smoke = dict(schema_version=1, output_sha256=self.hash,
                          audio=acceptance.analyze_wav(self.wav, 24000),
                          provenance=dict(kind='synthetic', system='Darwin', architecture='arm64'))
        self.reference = dict(schema_version=1, audio_sha256=self.hash, provenance='manual',
                              intervals=[dict(phoneme='a', start=.1, end=.4)])
        self.candidate = copy.deepcopy(self.reference)
        self.candidate['provenance'] = 'estimated'
        self.candidate['intervals'][0].update(start=.12, end=.44)
        self.review = dict(schema_version=1, audio_sha256=self.hash, reviewer='Fixture reviewer',
                           reviewed_at='2026-10-15', device='Synthetic fixture', notes='Not listened',
                           listening='not_tested', device_playback='not_tested', animation_sync='not_tested')

    def evaluate(self):
        return acceptance.evaluate(self.wav, self.smoke, self.reference, self.candidate, self.review)

    def test_measures_errors_without_accepting_synthetic_fixture(self):
        result = self.evaluate()
        self.assertAlmostEqual(result['boundary_error']['mean_ms'], 30)
        self.assertAlmostEqual(result['boundary_error']['max_ms'], 40)
        self.assertAlmostEqual(result['boundary_error']['p95_ms'], 40)
        self.assertFalse(result['mac_arm64_recording'])
        self.assertEqual(result['production_acceptance'], 'not_granted')

    def test_tampered_audio_and_measurements_rejected(self):
        self.smoke['audio']['peak'] = .99
        with self.assertRaisesRegex(ValueError, 'measurement mismatch'):
            self.evaluate()
        self.smoke['output_sha256'] = '0' * 64
        with self.assertRaisesRegex(ValueError, 'exact WAV'):
            self.evaluate()

    def test_alignment_identity_bounds_and_provenance_rejected(self):
        original = copy.deepcopy(self.candidate)
        for change in (dict(start=True), dict(end=float('nan')), dict(end=1.1), dict(phoneme='b')):
            self.candidate = copy.deepcopy(original)
            self.candidate['intervals'][0].update(change)
            with self.assertRaises(ValueError):
                self.evaluate()
        self.candidate = original
        self.reference['provenance'] = 'estimated'
        with self.assertRaisesRegex(ValueError, 'provenance'):
            self.evaluate()

    def test_human_review_is_required_and_bound(self):
        for field in ('reviewer', 'listening', 'device_playback', 'animation_sync', 'audio_sha256'):
            saved = self.review.pop(field)
            with self.assertRaises(ValueError):
                self.evaluate()
            self.review[field] = saved
        self.smoke['provenance']['kind'] = 'recorded_local_model'
        self.assertTrue(self.evaluate()['mac_arm64_recording'])
        self.assertEqual(self.evaluate()['production_acceptance'], 'not_granted')


if __name__ == '__main__':
    unittest.main()
