import importlib.util
from pathlib import Path
import plistlib
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location('package_macos', Path(__file__).resolve().parents[1] / 'scripts/package_macos.py')
package = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(package)


class PackageTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.repo = self.root / 'source with spaces'
        for relative in ['examples/dialogue.yaml', 'examples/model.pt',
                         'services/kokoro-runtime/runtime.py',
                         'services/kokoro-runtime/doctor.py',
                         'services/kokoro-runtime/smoke_macos.py',
                         'services/kokoro-runtime/requirements.txt',
                         'services/kokoro-runtime/README.md',
                         'services/kokoro-runtime/weights.pt', 'docs/practical-testing.md']:
            target = self.repo / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text('fixture')
        self.binary = self.root / 'studio-desktop'
        self.binary.write_text('executable fixture')
        self.binary.chmod(0o755)
        self.bundle = self.root / package.APP_NAME

    def stage(self):
        return package.stage_bundle(self.repo, self.binary, self.bundle, 'abc123', 'arm64', 'debug')

    def test_layout_permissions_and_allowlisted_resources(self):
        self.stage()
        contents = self.bundle / 'Contents'
        with (contents / 'Info.plist').open('rb') as handle:
            self.assertEqual(plistlib.load(handle)['CFBundleExecutable'], 'studio-desktop')
        self.assertTrue((contents / 'MacOS/studio-desktop').stat().st_mode & 0o111)
        resources = contents / 'Resources'
        self.assertTrue((resources / 'examples/dialogue.yaml').is_file())
        self.assertTrue((resources / 'services/kokoro-runtime/runtime.py').is_file())
        for helper in ['doctor.py', 'smoke_macos.py']:
            self.assertEqual((resources / 'services/kokoro-runtime' / helper).read_text(), 'fixture')
        self.assertEqual(list(resources.rglob('*.pt')), [])
        self.assertIn('abc123', (resources / 'BUILD-INFO.txt').read_text())

    def test_refuses_existing_bundle(self):
        self.stage()
        with self.assertRaisesRegex(ValueError, 'already exists'):
            self.stage()

    def test_requires_diagnostic_helpers_before_writing(self):
        for helper in ['doctor.py', 'smoke_macos.py']:
            path = self.repo / 'services/kokoro-runtime' / helper
            path.unlink()
            with self.assertRaisesRegex(ValueError, helper):
                self.stage()
            self.assertFalse(self.bundle.exists())
            path.write_text('fixture')

    def test_requires_executable(self):
        self.binary.chmod(0o644)
        with self.assertRaisesRegex(ValueError, 'executable'):
            self.stage()

    def test_requires_all_resources_before_writing(self):
        (self.repo / 'docs/practical-testing.md').unlink()
        with self.assertRaisesRegex(ValueError, 'missing or unsafe'):
            self.stage()
        self.assertFalse(self.bundle.exists())

    def test_rejects_symlinked_resource(self):
        path = self.repo / 'services/kokoro-runtime/runtime.py'
        path.unlink()
        path.symlink_to(self.binary)
        with self.assertRaisesRegex(ValueError, 'unsafe'):
            self.stage()


if __name__ == '__main__':
    unittest.main()
