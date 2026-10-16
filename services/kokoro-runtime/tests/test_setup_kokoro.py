import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location('setup_kokoro', Path(__file__).parents[1] / 'setup_kokoro.py')
setup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(setup)


def manifest():
    revision = 'a' * 40
    return dict(schema_version=1, model_repository='hexgrad/Kokoro-82M', model_revision=revision,
                files=[dict(path=p, url='https://huggingface.co/hexgrad/Kokoro-82M/resolve/' + revision + '/' + p,
                            sha256='a' * 64, size_bytes=10) for p in ('config.json', 'kokoro-v1_0.pth', 'voices/af_heart.pt', 'voices/am_michael.pt')],
                packages=['kokoro==0.9.4'], english_model_url='https://example.org/en_core_web_sm-3.8.0-py3-none-any.whl',
                english_model_sha256='b' * 64, english_model_size_bytes=20)


class SetupTests(unittest.TestCase):
    def test_plan_has_no_filesystem_or_network_or_subprocess_side_effects(self):
        with mock.patch.object(Path, 'open', side_effect=AssertionError('filesystem')), mock.patch.object(Path, 'mkdir', side_effect=AssertionError('filesystem')), mock.patch.object(setup.subprocess, 'Popen', side_effect=AssertionError('process')), mock.patch.object(setup.urllib.request, 'urlopen', side_effect=AssertionError('network')), contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertEqual(setup.main(['--plan', '--root', '/not-present', '--manifest', '/absent']), 0)
        self.assertEqual(json.loads(output.getvalue())['phase'], 'plan')

    def test_consent_precedes_manifest_access(self):
        with mock.patch.object(Path, 'open', side_effect=AssertionError('filesystem')), contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertEqual(setup.main(['--root', '/new', '--manifest', '/manifest']), 1)
        self.assertIn('consent', output.getvalue())

    def test_manifest_rejects_unpinned_or_unsafe_assets(self):
        self.assertEqual(setup.validate_manifest(manifest())['schema_version'], 1)
        for key, value in [('model_revision', 'main'), ('packages', ['kokoro>=0.9']), ('english_model_sha256', '')]:
            data = manifest()
            data[key] = value
            with self.assertRaises(ValueError):
                setup.validate_manifest(data)
        data = manifest()
        data['files'][0]['path'] = '../escape'
        with self.assertRaises(ValueError):
            setup.validate_manifest(data)

    def test_hash_and_size_failure_do_not_pass(self):
        payload = b'hello'
        for digest, size in [('0' * 64, 5), (hashlib.sha256(payload).hexdigest(), 4), (hashlib.sha256(payload).hexdigest(), 6)]:
            response = io.BytesIO(payload)
            response.geturl = lambda: 'https://example.org/asset'
            with tempfile.TemporaryDirectory() as directory, mock.patch.object(setup.urllib.request, 'urlopen', return_value=response):
                with self.assertRaises(ValueError):
                    setup.download('https://example.org/asset', Path(directory) / 'asset', digest, size, None)

    def test_existing_root_is_never_removed(self):
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(setup.platform, 'system', return_value='Darwin'), mock.patch.object(setup.sys, 'version_info', (3, 11)):
            root = Path(directory)
            sentinel = root / 'keep'
            sentinel.write_text('user data')
            with self.assertRaises(FileExistsError):
                setup.install(root, manifest(), None)
            self.assertEqual(sentinel.read_text(), 'user data')

    def test_failure_removes_only_owned_root(self):
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(setup.platform, 'system', return_value='Darwin'), mock.patch.object(setup.sys, 'version_info', (3, 11)), mock.patch.object(setup, 'run', side_effect=RuntimeError('stub failure')), contextlib.redirect_stdout(io.StringIO()):
            root = Path(directory) / 'new'
            with self.assertRaisesRegex(RuntimeError, 'stub failure'):
                setup.install(root, manifest(), None)
            self.assertFalse(root.exists())
            self.assertTrue(Path(directory).exists())

    def test_cancel_before_creation(self):
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(setup.platform, 'system', return_value='Darwin'), mock.patch.object(setup.sys, 'version_info', (3, 11)):
            marker = Path(directory) / 'cancel'
            marker.touch()
            root = Path(directory) / 'new'
            with self.assertRaisesRegex(RuntimeError, 'cancelled'):
                setup.install(root, manifest(), marker)
            self.assertFalse(root.exists())

    def test_cancel_kills_descendants_even_when_leader_exits_on_term(self):
        process = mock.Mock(pid=123, returncode=None)
        process.poll.return_value = None
        process.wait.return_value = 0
        with mock.patch.object(setup.subprocess, 'Popen', return_value=process), mock.patch.object(setup, 'cancelled', side_effect=RuntimeError('cancelled')), mock.patch.object(setup.os, 'killpg') as kill:
            with self.assertRaisesRegex(RuntimeError, 'cancelled'):
                setup.run(['stub-command'], None)
        self.assertEqual(kill.call_args_list, [mock.call(123, setup.signal.SIGTERM), mock.call(123, setup.signal.SIGKILL)])

    def test_complete_profile_only_after_successful_doctor(self):
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(setup.platform, 'system', return_value='Darwin'), mock.patch.object(setup.sys, 'version_info', (3, 11)), contextlib.redirect_stdout(io.StringIO()):
            root = Path(directory) / 'new'
            def fake_run(command, cancel_file, timeout=1800, **kwargs):
                self.assertFalse((root / 'profile.json').exists())
                if '-c' in command:
                    output = root / 'evidence/distributions.json'
                    output.write_text(json.dumps({'schema_version': 1, 'installed': []}))
                    return ''
                if 'doctor.py' in ' '.join(command):
                    return '{"ok":true}'
                return 'kokoro==0.9.4\n' if 'freeze' in command else ''
            def fake_download(url, path, digest, size, cancel_file):
                path.write_bytes(b'test')
            with mock.patch.object(setup, 'run', side_effect=fake_run), mock.patch.object(setup, 'download', side_effect=fake_download):
                profile = setup.install(root, manifest(), None)
            self.assertEqual(json.loads((root / 'profile.json').read_text()), profile)
            for field in ('python', 'config', 'model', 'voices', 'cache'):
                self.assertTrue(Path(profile[field]).is_relative_to(root))
            self.assertTrue(profile['revision'].startswith('a' * 40 + ':'))
            evidence = setup.validate_profile_evidence(profile, root)
            self.assertEqual(evidence['freeze'], 'kokoro==0.9.4\n')
            (root / 'evidence/freeze.txt').write_text('tampered')
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                setup.validate_profile_evidence(profile, root)

    def test_old_profiles_and_redaction(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertIsNone(setup.validate_profile_evidence({'revision': 'old'}, directory))
        self.assertEqual(setup.sanitized_freeze('pkg @ https://user:secret@example.org/pkg.whl?token=hidden#sha256=abc\n'),
                         'pkg @ https://example.org/pkg.whl\n')

    def test_oversized_license_fails_before_read(self):
        from email.message import Message
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / 'LICENSE'
            with path.open('wb') as stream:
                stream.truncate(256 * 1024 + 1)
            metadata = Message()
            metadata['Name'] = 'example'
            metadata['Version'] = '1.0'
            distribution = mock.Mock(metadata=metadata, files=['LICENSE'])
            distribution.locate_file.return_value = path
            with mock.patch.object(Path, 'read_bytes', side_effect=AssertionError('oversized file read')):
                setup.collect_installed_metadata(root / 'metadata.json', [distribution], root)
            entry = json.loads((root / 'metadata.json').read_text())['installed'][0]['licenses'][0]
            self.assertEqual(entry['content_status'], 'not-collected-size-budget')
            self.assertNotIn('sha256', entry)
            self.assertNotIn('content_base64', entry)

    def test_total_text_budget_records_remaining_license_without_content(self):
        from email.message import Message
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            paths = []
            for index in range(17):
                name = 'LICENSE-' + str(index)
                (root / name).write_bytes(b'x' * (256 * 1024))
                paths.append(name)
            metadata = Message()
            metadata['Name'] = 'example'
            metadata['Version'] = '1.0'
            distribution = mock.Mock(metadata=metadata, files=paths)
            distribution.locate_file.side_effect = lambda name: root / name
            setup.collect_installed_metadata(root / 'metadata.json', [distribution], root)
            entries = json.loads((root / 'metadata.json').read_text())['installed'][0]['licenses']
            skipped = [entry for entry in entries if entry['content_status'] != 'collected']
            self.assertEqual(len(skipped), 1)
            self.assertEqual(skipped[0]['content_status'], 'not-collected-total-budget')
            self.assertNotIn('sha256', skipped[0])
            self.assertNotIn('content_base64', skipped[0])

    def test_metadata_collects_real_bytes_and_rejects_escape(self):
        from email.message import Message
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            license_path = root / 'x.dist-info/LICENSE'
            license_path.parent.mkdir()
            license_path.write_bytes(b'copyright\nMIT\n')
            metadata = Message()
            metadata['Name'] = 'example'
            metadata['Version'] = '1.2'
            metadata['License-Expression'] = 'MIT'
            distribution = mock.Mock(metadata=metadata, files=['x.dist-info/LICENSE'])
            distribution.locate_file.side_effect = lambda path: root / path
            output = root / 'metadata.json'
            setup.collect_installed_metadata(output, [distribution], root)
            row = json.loads(output.read_text())['installed'][0]
            self.assertEqual(row['declared_license'], 'MIT')
            self.assertEqual(row['licenses'][0]['sha256'], hashlib.sha256(license_path.read_bytes()).hexdigest())
            self.assertEqual(setup.base64.b64decode(row['licenses'][0]['content_base64']), license_path.read_bytes())
            with tempfile.TemporaryDirectory() as outside:
                escaped = Path(outside) / 'LICENSE'
                escaped.write_text('outside')
                license_path.unlink()
                license_path.symlink_to(escaped)
                with self.assertRaisesRegex(ValueError, 'escapes'):
                    setup.collect_installed_metadata(output, [distribution], root)



if __name__ == '__main__':
    unittest.main()
