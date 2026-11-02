"""Speech bundle builder checks; no network, model, Python download or inference."""
import copy
import importlib.util
import json
from pathlib import Path
import stat
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('build_speech_bundle', ROOT / 'scripts/build_speech_bundle.py')
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)
MANIFEST = json.loads((ROOT / 'services/speech-bundle/manifest.json').read_text())
KOKORO = json.loads((ROOT / 'services/kokoro-runtime/setup-manifest.json').read_text())
CATALOG = json.loads((ROOT / 'services/kokoro-runtime/english-voices.json').read_text())


class ManifestTests(unittest.TestCase):
    def test_checked_in_manifest_is_valid(self):
        builder.validate_manifest(copy.deepcopy(MANIFEST))

    def test_insecure_unpinned_or_unsafe_entries_are_rejected(self):
        cases = [
            lambda m: m['python'].update(url=m['python']['url'].replace('https://', 'http://')),
            lambda m: m['python'].update(sha256='0' * 63),
            lambda m: m['python'].update(version='3.13.1'),
            lambda m: m.update(packages=['piper-tts>=1.8']),
            lambda m: m['piper'].update(revision='main'),
            lambda m: m['piper']['files'][0].update(path='../escape.onnx'),
            lambda m: m['piper']['files'][0].update(url=m['piper']['files'][0]['url'].replace(
                m['piper']['revision'], 'main')),
            lambda m: m['rhubarb'].update(keep=['res']),
            lambda m: m.update(kokoro_manifest='/etc/passwd'),
        ]
        for index, mutate in enumerate(cases):
            with self.subTest(case=index):
                manifest = copy.deepcopy(MANIFEST)
                mutate(manifest)
                with self.assertRaises((ValueError, KeyError)):
                    builder.validate_manifest(manifest)

    def test_every_pinned_english_voice_and_the_model_are_bundled_once(self):
        files = builder.kokoro_files(KOKORO, CATALOG)
        paths = [item['path'] for item in files]
        self.assertEqual(len(paths), len(set(paths)))
        self.assertIn('config.json', paths)
        self.assertIn('kokoro-v1_0.pth', paths)
        self.assertEqual(sum(p.startswith('voices/') for p in paths), len(CATALOG['voices']))

    def test_conflicting_voice_pins_or_revisions_are_rejected(self):
        catalog = copy.deepcopy(CATALOG)
        voice = next(v for v in catalog['voices'] if v['name'] == 'af_heart')
        voice['sha256'] = 'f' * 64
        with self.assertRaises(ValueError):
            builder.kokoro_files(KOKORO, catalog)
        catalog = copy.deepcopy(CATALOG)
        catalog['model_revision'] = '0' * 40
        with self.assertRaises(ValueError):
            builder.kokoro_files(KOKORO, catalog)


class LayoutTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def test_descriptor_uses_relative_paths_and_binds_revisions(self):
        document = builder.descriptor(MANIFEST, KOKORO, 'a' * 64, [])
        self.assertEqual(document['schema_version'], 1)
        self.assertEqual(document['kokoro']['revision'], KOKORO['model_revision'] + ':' + 'a' * 64)
        self.assertEqual(document['piper']['preset'], 'piper_harri')
        self.assertIn(MANIFEST['piper']['revision'], document['piper']['model_version'])
        self.assertIn('a' * 64, document['piper']['environment_identity'])
        for section in ('kokoro', 'piper', 'rhubarb'):
            for key, value in document[section].items():
                if key in ('python', 'config', 'model', 'voices', 'executable'):
                    builder.safe_relative(value)

    def test_piper_wrapper_runs_the_bundled_python_isolated(self):
        path = builder.write_piper_wrapper(self.root)
        self.assertTrue(path.stat().st_mode & stat.S_IXUSR)
        text = path.read_text()
        self.assertIn('../python/bin/python3.12" -I -B "$here/piper_launch.py"', text)
        launch = (self.root / 'bin/piper_launch.py').read_text()
        self.assertIn("'espeak', 'piper'", launch)
        compile(launch, 'piper_launch.py', 'exec')

    def test_espeak_data_moves_to_short_paths_and_leaves_empty_originals(self):
        for original in builder.ESPEAK_DATA.values():
            (self.root / original).mkdir(parents=True)
            (self.root / original / 'phontab').write_bytes(b'data')
        builder.relocate_espeak_data(self.root)
        for short, original in builder.ESPEAK_DATA.items():
            self.assertTrue((self.root / short / 'phontab').is_file())
            self.assertEqual(list((self.root / original).iterdir()), [])
            # 59 characters is the bundle prefix inside /Applications/Kilsat Studio.app.
            self.assertLess(59 + len(short), 80)

    def test_missing_espeak_data_fails_the_build(self):
        with self.assertRaises(ValueError):
            builder.relocate_espeak_data(self.root)

    def test_rhubarb_keeps_only_the_tool_resources_and_license(self):
        archive = self.root / 'rhubarb.zip'
        prefix = MANIFEST['rhubarb']['archive_root'] + '/'
        with zipfile.ZipFile(archive, 'w') as bundle:
            info = zipfile.ZipInfo(prefix + 'rhubarb')
            info.external_attr = (stat.S_IFREG | 0o755) << 16
            bundle.writestr(info, b'binary')
            bundle.writestr(prefix + 'res/sphinx/cmudict-en-us.dict', b'dict')
            bundle.writestr(prefix + 'LICENSE.md', b'license')
            bundle.writestr(prefix + 'extras/plugin.jar', b'not shipped')
            bundle.writestr(prefix + 'tests/x.wav', b'not shipped')
        destination = self.root / 'out'
        builder.extract_rhubarb(archive, destination, MANIFEST['rhubarb'])
        names = sorted(p.relative_to(destination).as_posix() for p in destination.rglob('*') if p.is_file())
        self.assertEqual(names, ['LICENSE.md', 'res/sphinx/cmudict-en-us.dict', 'rhubarb'])
        self.assertTrue((destination / 'rhubarb').stat().st_mode & stat.S_IXUSR)

    def test_rhubarb_symlinks_are_rejected(self):
        archive = self.root / 'rhubarb.zip'
        prefix = MANIFEST['rhubarb']['archive_root'] + '/'
        with zipfile.ZipFile(archive, 'w') as bundle:
            info = zipfile.ZipInfo(prefix + 'rhubarb')
            info.external_attr = (stat.S_IFLNK | 0o777) << 16
            bundle.writestr(info, b'/bin/sh')
        with self.assertRaises(ValueError):
            builder.extract_rhubarb(archive, self.root / 'out', MANIFEST['rhubarb'])

    def test_symlinks_are_removed_and_reported(self):
        (self.root / 'python/bin').mkdir(parents=True)
        (self.root / 'python/bin/python3.12').write_bytes(b'x')
        (self.root / 'python/bin/python3').symlink_to('python3.12')
        self.assertEqual(builder.remove_symlinks(self.root), ['python/bin/python3'])
        self.assertTrue((self.root / 'python/bin/python3.12').is_file())

    def test_macho_detection_reads_only_the_magic(self):
        (self.root / 'lib.dylib').write_bytes(b'\xcf\xfa\xed\xfe' + b'\0' * 12)
        (self.root / 'script.py').write_text('print(1)\n')
        self.assertTrue(builder.is_macho(self.root / 'lib.dylib'))
        self.assertFalse(builder.is_macho(self.root / 'script.py'))

    def test_downloads_require_explicit_consent(self):
        with self.assertRaises(SystemExit):
            builder.main(['--output', str(self.root / 'speech')])
        self.assertFalse((self.root / 'speech').exists())


if __name__ == '__main__':
    unittest.main()
