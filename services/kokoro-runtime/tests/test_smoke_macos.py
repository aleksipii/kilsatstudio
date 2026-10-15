"""No real inference: validate evidence and failure handling with local PCM fixtures."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import struct
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock
import wave

spec = importlib.util.spec_from_file_location('smoke_macos', Path(__file__).resolve().parents[1] / 'smoke_macos.py')
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


def wav(path, samples=(0, 16384, -16384, 0)):
    with wave.open(str(path), 'wb') as target:
        target.setparams((1, 2, 24000, 0, 'NONE', 'not compressed'))
        target.writeframes(struct.pack('<' + 'h' * len(samples), *samples))


class SmokeTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.asset = self.root / 'af_heart.pt'
        self.asset.write_bytes(b'local fixture asset')
        self.output = self.root / 'speech.wav'
        self.report = self.root / 'report.json'
        self.args = ['--allow-local-inference', '--config', str(self.asset), '--model', str(self.asset),
                     '--voice', str(self.asset), '--output', str(self.output), '--report', str(self.report)]

    def runtime(self, command, **kwargs):
        request = json.loads(Path(command[command.index('--request') + 1]).read_text())
        wav(Path(request['output_path']))
        Path(command[command.index('--response') + 1]).write_text(json.dumps(dict(ok=True, sample_count=4)))
        return SimpleNamespace(returncode=0)

    def run_main(self, side_effect=None, system='Darwin', args=None):
        with mock.patch.object(smoke.platform, 'system', return_value=system), \
             mock.patch.object(smoke.platform, 'platform', return_value='macOS-fixture'), \
             mock.patch.object(smoke.platform, 'mac_ver', return_value=('14.0', ('', '', ''), 'arm64')), \
             mock.patch.object(smoke.platform, 'machine', return_value='arm64'), \
             mock.patch.object(smoke.platform, 'processor', return_value='arm'), \
             mock.patch.object(smoke.subprocess, 'run', side_effect=side_effect or self.runtime), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            return smoke.main(self.args if args is None else args)

    def test_report_is_structural_evidence_with_hashes_and_signal(self):
        self.assertEqual(self.run_main(), 0)
        report = json.loads(self.report.read_text())
        self.assertEqual(report['schema_version'], 1)
        self.assertEqual(report['output_sha256'], smoke.sha256(self.output))
        self.assertEqual(report['assets']['model']['sha256'], smoke.sha256(self.asset))
        self.assertEqual(report['audio']['peak'], 0.5)
        self.assertAlmostEqual(report['audio']['rms'], 0.3535533905932738)
        self.assertEqual(report['audio']['duration_seconds'], 4 / 24000)
        self.assertEqual(report['perceptual_validation'], 'requires listening')
        self.assertEqual(report['alignment'], 'not_aligned')
        self.assertIn('torch', report['packages'])

    def test_non_mac_missing_assets_and_existing_files_rejected_before_process(self):
        with self.assertRaises(SystemExit):
            self.run_main(system='Linux')
        self.asset.unlink()
        with self.assertRaises(SystemExit):
            self.run_main()
        self.asset.write_bytes(b'asset')
        for path in (self.output, self.report):
            path.write_bytes(b'preserve')
            with self.assertRaises(SystemExit):
                self.run_main()
            self.assertEqual(path.read_bytes(), b'preserve')
            path.unlink()

    def test_malformed_missing_and_timeout_responses_leave_no_evidence(self):
        def malformed(command, **kwargs):
            Path(command[command.index('--response') + 1]).write_text('{broken')
            return SimpleNamespace(returncode=0)
        for behavior in (malformed, lambda *a, **k: SimpleNamespace(returncode=1),
                         subprocess.TimeoutExpired('runtime', 1)):
            with self.subTest(behavior=behavior):
                self.assertEqual(self.run_main(side_effect=behavior), 1)
                self.assertFalse(self.output.exists())
                self.assertFalse(self.report.exists())

    def test_invalid_response_shape_and_sample_count(self):
        for result in ([], {'ok': 'yes'}, {'ok': True, 'sample_count': True}, {'ok': True, 'sample_count': 5}):
            def respond(command, **kwargs):
                request = json.loads(Path(command[command.index('--request') + 1]).read_text())
                wav(Path(request['output_path']))
                Path(command[command.index('--response') + 1]).write_text(json.dumps(result))
                return SimpleNamespace(returncode=0)
            self.assertEqual(self.run_main(side_effect=respond), 1)
            self.assertFalse(self.report.exists())
            self.assertFalse(self.output.exists())

    def test_wav_rejects_silence_and_truncation(self):
        wav(self.output, (0, 0))
        with self.assertRaisesRegex(ValueError, 'silence'):
            smoke.analyze_wav(self.output, 2)
        wav(self.output)
        self.output.write_bytes(self.output.read_bytes()[:-2])
        with self.assertRaisesRegex(ValueError, 'truncated'):
            smoke.analyze_wav(self.output, 4)

    def test_publish_race_preserves_existing_report(self):
        self.report.write_text('keep')
        with self.assertRaises(FileExistsError):
            smoke.publish_report(self.report, {'schema_version': 1})
        self.assertEqual(self.report.read_text(), 'keep')
        self.assertFalse(list(self.root.glob('.kokoro-evidence-*')))

    def test_output_created_during_inference_is_preserved(self):
        def race(command, **kwargs):
            self.output.write_bytes(b'other owner')
            return self.runtime(command, **kwargs)
        self.assertEqual(self.run_main(side_effect=race), 1)
        self.assertEqual(self.output.read_bytes(), b'other owner')
        self.assertFalse(self.report.exists())


if __name__ == '__main__':
    unittest.main()
