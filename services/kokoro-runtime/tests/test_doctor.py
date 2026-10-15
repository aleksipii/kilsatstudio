import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location('doctor', Path(__file__).parents[1] / 'doctor.py')
doctor = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(doctor)


class DoctorTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.files = [self.root / name for name in ('config.json', 'model.pth', 'af_heart.pt', 'am_michael.pt')]
        for path in self.files:
            path.write_bytes(b'fixture bytes')

    def versions(self, package):
        return '0.9.4' if package == 'kokoro' else '1.0'

    def inspect(self):
        return doctor.inspect(self.files[0], self.files[1], self.files[2:])

    def test_ready_reports_each_voice_and_streamed_fingerprints(self):
        with mock.patch.object(doctor.metadata, 'version', side_effect=self.versions) as versions:
            report = self.inspect()
        self.assertTrue(report['ok'])
        self.assertEqual(versions.call_count, len(doctor.REQUIRED))
        self.assertEqual(len(report['assets']), 4)
        for asset in report['assets']:
            self.assertEqual(asset['size_bytes'], len(b'fixture bytes'))
            self.assertEqual(asset['sha256'], hashlib.sha256(b'fixture bytes').hexdigest())
        self.assertIn('not validated', report['verification'])

    def test_missing_distribution_is_actionable(self):
        def missing(package):
            if package == 'en-core-web-sm':
                raise doctor.metadata.PackageNotFoundError(package)
            return self.versions(package)
        with mock.patch.object(doctor.metadata, 'version', side_effect=missing):
            report = self.inspect()
        self.assertFalse(report['ok'])
        self.assertFalse(report['english_g2p_distribution_present'])
        self.assertIn('Missing en-core-web-sm', report['errors'][0])

    def test_wrong_kokoro_and_missing_empty_directory_assets(self):
        self.files[0].unlink()
        self.files[1].write_bytes(b'')
        self.files[2].unlink()
        self.files[2].mkdir()
        with mock.patch.object(doctor.metadata, 'version', return_value='99'):
            report = self.inspect()
        self.assertFalse(report['ok'])
        self.assertEqual(len(report['errors']), 4)
        self.assertIsNone(report['assets'][0]['sha256'])

    def test_cli_json_and_exit_status(self):
        arguments = ['--config', str(self.files[0]), '--model', str(self.files[1]),
                     '--voice', str(self.files[2]), '--voice', str(self.files[3])]
        for ready in (True, False):
            output = io.StringIO()
            effect = self.versions if ready else doctor.metadata.PackageNotFoundError('missing')
            with mock.patch.object(doctor.metadata, 'version', side_effect=effect), contextlib.redirect_stdout(output):
                code = doctor.main(arguments)
            self.assertEqual(code, 0 if ready else 1)
            self.assertEqual(json.loads(output.getvalue())['ok'], ready)

    def test_cli_requires_voice_and_rejects_unknown_options(self):
        with contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as error:
                doctor.main(['--config', 'x', '--model', 'y'])
        self.assertEqual(error.exception.code, 2)

    def test_read_and_metadata_errors_do_not_crash(self):
        with mock.patch.object(Path, 'open', side_effect=PermissionError('denied')), mock.patch.object(doctor.metadata, 'version', side_effect=OSError('broken metadata')):
            report = self.inspect()
        self.assertFalse(report['ok'])
        self.assertEqual(len(report['errors']), len(doctor.REQUIRED) + 4)


if __name__ == '__main__':
    unittest.main()
