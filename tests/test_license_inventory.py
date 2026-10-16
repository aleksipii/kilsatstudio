"""Evidence inventory tests, entirely offline and without installed Kokoro."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('license_inventory', ROOT / 'scripts/license_inventory.py')
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)


class InventoryTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        runtime = self.root / 'services/kokoro-runtime'
        runtime.mkdir(parents=True)
        self.manifest = json.loads((ROOT / 'services/kokoro-runtime/setup-manifest.json').read_text())
        (runtime / 'setup-manifest.json').write_text(json.dumps(self.manifest))
        (runtime / 'requirements.txt').write_text('# ignored\nkokoro==0.9.4\ntorch>=2.2,<3\n')
        (self.root / 'Cargo.lock').write_text('version = 4\n[[package]]\nname = "example"\nversion = "1.0.0"\nsource = "registry+example"\nchecksum = "abc"\n')

    def test_identity_is_not_license_or_approval(self):
        report = AUDIT.build_inventory(self.root)
        self.assertFalse(report['distribution_approved'])
        self.assertIsNone(report['rust'][0]['declared_license'])
        self.assertEqual(report['rust'][0]['notices_status'], 'not-collected')
        self.assertTrue(report['unresolved'])

    def test_metadata_requires_exact_source_identity(self):
        package = dict(name='example', version='1.0.0', source='registry+wrong', license='MIT')
        self.assertIsNone(AUDIT.build_inventory(self.root, {'packages': [package]})['rust'][0]['declared_license'])
        package['source'] = 'registry+example'
        self.assertEqual(AUDIT.build_inventory(self.root, {'packages': [package]})['rust'][0]['declared_license'], 'MIT')
        self.assertIsNone(AUDIT.build_inventory(self.root, {'packages': [package, package]})['rust'][0]['declared_license'])

    def test_pip_observations_do_not_establish_wheel_hashes(self):
        pip = {'installed': [{'metadata': {'name': 'Kokoro', 'version': '0.9.4', 'license_expression': 'Apache-2.0'}}]}
        report = AUDIT.build_inventory(self.root, pip_inspect=pip)
        self.assertEqual(report['python_direct'][0]['match_status'], 'matches')
        self.assertEqual(report['python_installed'][0]['artifact_hash_status'], 'not-established-by-pip-inspect')
        pip['installed'][0]['metadata']['version'] = '0.0.0'
        self.assertEqual(AUDIT.build_inventory(self.root, pip_inspect=pip)['python_direct'][0]['match_status'], 'mismatch')

    def test_unpinned_managed_package_fails(self):
        manifest = copy.deepcopy(self.manifest)
        manifest['packages'][0] = 'kokoro>=0.9'
        (self.root / 'services/kokoro-runtime/setup-manifest.json').write_text(json.dumps(manifest))
        with self.assertRaisesRegex(ValueError, 'exact version'):
            AUDIT.build_inventory(self.root)

    def test_immutable_assets_distinguished_from_local_verification(self):
        report = AUDIT.build_inventory(self.root)
        self.assertTrue(all(asset['pin_status'] == 'pinned' for asset in report['model_assets']))
        self.assertEqual(report['english_model']['pin_status'], 'pinned')
        self.assertTrue(all(asset['local_bytes_status'] == 'not-inspected' for asset in report['model_assets']))
        manifest = copy.deepcopy(self.manifest)
        manifest['files'][0]['url'] = 'https://example.org/resolve/main/config.json'
        (self.root / 'services/kokoro-runtime/setup-manifest.json').write_text(json.dumps(manifest))
        self.assertEqual(AUDIT.build_inventory(self.root)['model_assets'][0]['pin_status'], 'invalid-pin')

    def test_binary_is_hashed_without_execution(self):
        path = self.root / 'encoder'
        path.write_bytes(b'not executable')
        report = AUDIT.build_inventory(self.root, binaries=[('ffmpeg', path)])
        self.assertEqual(report['binaries'][0]['sha256'], AUDIT.digest(path))
        self.assertEqual(report['binaries'][0]['version_status'], 'not-executed')

    def test_deterministic_and_input_sensitive(self):
        first = AUDIT.build_inventory(self.root)
        self.assertEqual(first, AUDIT.build_inventory(self.root))
        (self.root / 'Cargo.lock').write_text((self.root / 'Cargo.lock').read_text() + '\n# change\n')
        self.assertNotEqual(first['inputs'], AUDIT.build_inventory(self.root)['inputs'])

    def test_strict_writes_reviewable_result_and_fails_gate(self):
        output = self.root / 'inventory.json'
        self.assertEqual(AUDIT.main(['--root', str(self.root), '--output', str(output), '--strict']), 1)
        self.assertFalse(json.loads(output.read_text())['distribution_approved'])

    def test_invalid_optional_document_shapes_rejected(self):
        for document in ([], {}, {'installed': [42]}):
            with self.assertRaises(ValueError):
                AUDIT.build_inventory(self.root, pip_inspect=document)
        with self.assertRaises(ValueError):
            AUDIT.build_inventory(self.root, metadata={'packages': 'wrong'})

    def test_invalid_pip_record_rejected(self):
        with self.assertRaisesRegex(ValueError, 'name or version'):
            AUDIT.build_inventory(self.root, pip_inspect={'installed': [{'metadata': {'name': 'unknown'}}]})


if __name__ == '__main__':
    unittest.main()
