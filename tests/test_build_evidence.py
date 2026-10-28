"""Saved build graph identity checks; never compile or execute fixture binaries."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

REPO = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('build_evidence', REPO / 'scripts/build_evidence.py')
build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build)


class BuildEvidenceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.paths = {}
        self.put('archive', b'fixture final ZIP bytes')
        self.put('lock', b'''version = 4
[[package]]
name = "studio-desktop"
version = "0.1.0"
[[package]]
name = "motion-render"
version = "0.1.0"
[[package]]
name = "third-party"
version = "1.0.0"
source = "registry+example"
checksum = "abc"
''')
        self.put('rustc', b'rustc 1.90.0\nhost: aarch64-apple-darwin\n')
        self.put('cargo', b'cargo 1.90.0\n')
        self.members = ['Kilsat Studio.app/Contents/MacOS/studio-desktop',
                        'Kilsat Studio.app/Contents/Resources/bin/gpu_probe']
        with zipfile.ZipFile(self.paths['archive'], 'w') as archive:
            for path in self.members:
                archive.writestr(path, b'fixture final executable')
        member_hash = hashlib.sha256(b'fixture final executable').hexdigest()
        self.audit = dict(package_valid=True, distribution_approved=False,
                          artifact=dict(commit='a' * 40, target='aarch64-apple-darwin',
                                        profile='debug', **build.identity(self.paths['archive'].read_bytes())),
                          files=[dict(path=p, sha256=member_hash) for p in self.members])
        self.put_json('audit', self.audit)
        self.observations = {p: dict(sha256=member_hash, lipo_archs='arm64',
                                    otool_libraries='fixture:\n /usr/lib/libSystem.B.dylib')
                             for p in self.members}
        self.put_json('observations', self.observations)
        self.metadata = dict(packages=[
            dict(id=name, name=name, version=version, source=source, license='MIT')
            for name, version, source in [('studio-desktop', '0.1.0', None),
                                          ('motion-render', '0.1.0', None),
                                          ('third-party', '1.0.0', 'registry+example')]],
            workspace_members=['studio-desktop', 'motion-render'],
            resolve=dict(nodes=[
                dict(id='studio-desktop', dependencies=['third-party'], features=['default']),
                dict(id='motion-render', dependencies=['third-party'], features=['gpu']),
                dict(id='third-party', dependencies=[], features=['native'])]))
        self.put_json('desktop', self.metadata)
        self.put_json('helper', self.metadata)

    def put(self, key, data):
        self.paths[key] = self.root / key
        self.paths[key].write_bytes(data)

    def put_json(self, key, data):
        self.put(key, json.dumps(data).encode())

    def collect(self):
        return build.collect(*(self.paths[k] for k in (
            'archive', 'audit', 'lock', 'desktop', 'helper', 'rustc', 'cargo', 'observations')))

    def test_graphs_are_separate_hash_bound_and_never_approved(self):
        report = self.collect()
        self.assertFalse(report['distribution_approved'])
        self.assertEqual(report['physical_device_acceptance'], 'not_granted')
        desktop, helper = report['graphs']
        self.assertEqual([p['id'] for p in desktop['packages']], ['studio-desktop', 'third-party'])
        self.assertEqual([p['id'] for p in helper['packages']], ['motion-render', 'third-party'])
        self.assertEqual(helper['packages'][1]['lock_checksum'], 'abc')
        self.assertEqual(helper['packages'][1]['license_review'], 'unreviewed')
        self.assertEqual(report, self.collect())

    def test_changed_zip_or_invalid_audit_rejected(self):
        self.paths['archive'].write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'archive bytes'):
            self.collect()

    def test_native_observations_bind_final_member_bytes(self):
        self.observations[self.members[0]]['sha256'] = 'c' * 64
        self.put_json('observations', self.observations)
        with self.assertRaisesRegex(ValueError, 'hash mismatch'):
            self.collect()

    def test_head_helper_requires_separate_hash_bound_observations(self):
        path = 'Kilsat Studio.app/Contents/Resources/bin/head_pose'
        data = b'fixture head executable'
        with zipfile.ZipFile(self.paths['archive'], 'a') as archive:
            archive.writestr(path, data)
        digest = hashlib.sha256(data).hexdigest()
        self.audit['artifact'].update(build.identity(self.paths['archive'].read_bytes()))
        self.audit['files'].append(dict(path=path, sha256=digest))
        self.put_json('audit', self.audit)
        with self.assertRaisesRegex(ValueError, 'separate desktop/helper'):
            self.collect()
        self.observations[path] = dict(sha256=digest, lipo_archs='arm64',
            otool_libraries='fixture:\n /System/Library/Frameworks/Vision.framework/Vision')
        self.put_json('observations', self.observations)
        self.assertEqual(len(self.collect()['native_observations']), 3)
        self.observations[path]['sha256'] = 'c' * 64
        self.put_json('observations', self.observations)
        with self.assertRaisesRegex(ValueError, 'hash mismatch'):
            self.collect()

    def test_missing_or_extra_native_component_rejected(self):
        self.observations['other'] = self.observations[self.members[0]]
        self.put_json('observations', self.observations)
        with self.assertRaisesRegex(ValueError, 'separate desktop/helper'):
            self.collect()

    def test_unlocked_component_and_incomplete_graph_rejected(self):
        self.metadata['packages'][2]['version'] = '2.0.0'
        self.put_json('desktop', self.metadata)
        with self.assertRaisesRegex(ValueError, 'Cargo.lock'):
            self.collect()
        self.metadata['packages'][2]['version'] = '1.0.0'
        self.metadata['resolve']['nodes'].pop()
        self.put_json('desktop', self.metadata)
        with self.assertRaisesRegex(ValueError, 'incomplete'):
            self.collect()

    def test_audit_member_forgery_rejected_even_with_correct_archive_hash(self):
        self.audit['files'][0]['sha256'] = 'c' * 64
        self.observations[self.members[0]]['sha256'] = 'c' * 64
        self.put_json('audit', self.audit)
        self.put_json('observations', self.observations)
        with self.assertRaisesRegex(ValueError, 'archive bytes'):
            self.collect()

    def test_malformed_document_shapes_fail_with_actionable_errors(self):
        for key, value in [('audit', []), ('observations', []), ('desktop', []),
                           ('audit', dict(self.audit, artifact=[])),
                           ('audit', dict(self.audit, files=[None])),
                           ('desktop', dict(self.metadata, resolve=[])),
                           ('desktop', dict(self.metadata, packages=[dict(id=[])]))]:
            with self.subTest(key=key, value=value):
                before = self.paths[key].read_bytes()
                self.put_json(key, value)
                with self.assertRaises(ValueError):
                    self.collect()
                self.put(key, before)
        self.observations[self.members[0]] = []
        self.put_json('observations', self.observations)
        with self.assertRaisesRegex(ValueError, 'observation entry must be an object'):
            self.collect()

    def test_helper_must_actually_resolve_gpu(self):
        self.metadata['resolve']['nodes'][1]['features'] = []
        self.put_json('helper', self.metadata)
        with self.assertRaisesRegex(ValueError, 'gpu feature'):
            self.collect()

    def test_input_bounds_and_symlinks(self):
        with self.assertRaisesRegex(ValueError, 'byte budget'):
            build.read_bound(self.paths['archive'], 1)
        link = self.root / 'link'
        link.symlink_to(self.paths['archive'])
        with self.assertRaisesRegex(ValueError, 'non-symlink'):
            build.read_bound(link)

    def test_cli_preserves_existing_evidence(self):
        output = self.root / 'report.json'
        arguments = []
        for flag, key in [('archive', 'archive'), ('audit', 'audit'), ('lock', 'lock'),
                          ('desktop-metadata', 'desktop'), ('helper-metadata', 'helper'),
                          ('rustc', 'rustc'), ('cargo', 'cargo'),
                          ('native-observations', 'observations')]:
            arguments.extend(['--' + flag, str(self.paths[key])])
        arguments.extend(['--output', str(output)])
        self.assertEqual(build.main(arguments), 0)
        before = output.read_bytes()
        self.assertEqual(build.main(arguments), 2)
        self.assertEqual(output.read_bytes(), before)


if __name__ == '__main__':
    unittest.main()
