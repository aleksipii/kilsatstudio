"""Offline final-package boundary checks; fixtures never execute their binaries."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import stat
import struct
import tempfile
import unittest
from unittest.mock import patch
import warnings
import zipfile

REPO = Path(__file__).resolve().parents[1]


def load_script(name):
    spec = importlib.util.spec_from_file_location(name, REPO / 'scripts' / (name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


package = load_script('package_macos')
audit = load_script('package_audit')
COMMIT = 'a' * 40
TARGET = 'aarch64-apple-darwin'
APP = package.APP_NAME
RESOURCES = APP + '/Contents/Resources/'


class PackageAuditTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.binary = self.root / 'fixture-binary'
        self.binary.write_bytes(b'offline fake executable\n')
        self.binary.chmod(0o755)
        self.distribution = self.root / 'distribution'
        self.archive = self.root / 'preview.zip'

    def stage(self, helper=False):
        package.stage_distribution(REPO, self.binary, self.distribution,
                                   COMMIT, 'arm64', 'debug',
                                   self.binary if helper else None)
        return self.distribution

    def write_zip(self, extras=(), replacements=None, reverse=False):
        replacements = replacements or {}
        paths = sorted(self.distribution.rglob('*'), reverse=reverse)
        with zipfile.ZipFile(self.archive, 'w', zipfile.ZIP_DEFLATED) as archive:
            for path in paths:
                if not path.is_file():
                    continue
                name = path.relative_to(self.distribution).as_posix()
                info = zipfile.ZipInfo(name)
                info.create_system = 3
                info.external_attr = (stat.S_IFREG | stat.S_IMODE(path.stat().st_mode)) << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                archive.writestr(info, replacements.get(name, path.read_bytes()))
            for info, payload in extras:
                archive.writestr(info, payload)
        return self.archive

    def collect(self, **kwargs):
        return audit.audit_package(self.archive, REPO, kwargs.get('commit', COMMIT),
                                   kwargs.get('target', TARGET), kwargs.get('profile', 'debug'))

    def assert_invalid(self, report):
        self.assertFalse(report['package_valid'])
        self.assertFalse(report['distribution_approved'])
        self.assertTrue(report['errors'])
        self.assertTrue(all(isinstance(error, str) for error in report['errors']))

    def test_valid_inventory_binds_exact_archive_and_every_file(self):
        self.stage()
        self.write_zip(reverse=True)
        report = self.collect()
        self.assertEqual(report['schema_version'], 1)
        self.assertTrue(report['package_valid'], report['errors'])
        self.assertFalse(report['distribution_approved'])
        self.assertEqual(report['errors'], [])
        self.assertTrue(report['unresolved'])
        artifact = report['artifact']
        self.assertEqual(artifact['sha256'], hashlib.sha256(self.archive.read_bytes()).hexdigest())
        self.assertEqual(artifact['size_bytes'], self.archive.stat().st_size)
        self.assertEqual((artifact['commit'], artifact['target'], artifact['profile']),
                         (COMMIT, TARGET, 'debug'))
        self.assertEqual(artifact['filename'], self.archive.name)
        records = report['files']
        self.assertEqual([record['path'] for record in records], sorted(record['path'] for record in records))
        with zipfile.ZipFile(self.archive) as archive:
            self.assertEqual(len(records), len(archive.infolist()))
            for record in records:
                data = archive.read(record['path'])
                self.assertEqual(record['size_bytes'], len(data))
                self.assertEqual(record['sha256'], hashlib.sha256(data).hexdigest())
                self.assertIn('mode', record)
                self.assertIn('classification', record)
        self.assertEqual(report, self.collect())

    def test_optional_gpu_helper_and_executable_permission_boundary(self):
        self.stage(helper=True)
        self.write_zip()
        report = self.collect()
        self.assertTrue(report['package_valid'], report['errors'])
        names = {record['path']: record for record in report['files']}
        self.assertIn(RESOURCES + 'bin/gpu_probe', names)
        helper = self.distribution / (RESOURCES + 'bin/gpu_probe')
        helper.chmod(0o644)
        self.write_zip()
        self.assert_invalid(self.collect())

    def test_optional_head_helper_must_be_executable(self):
        package.stage_distribution(REPO, self.binary, self.distribution,
                                   COMMIT, 'arm64', 'debug', head_pose=self.binary)
        self.write_zip()
        report = self.collect()
        self.assertTrue(report['package_valid'], report['errors'])
        row = next(r for r in report['files'] if r['path'] == RESOURCES + 'bin/head_pose')
        self.assertEqual(row['classification'], 'executable')
        (self.distribution / (RESOURCES + 'bin/head_pose')).chmod(0o644)
        self.write_zip()
        self.assert_invalid(self.collect())

    def test_missing_desktop_or_executable_bit_is_invalid(self):
        self.stage()
        executable = self.distribution / APP / 'Contents/MacOS/studio-desktop'
        executable.chmod(0o644)
        self.write_zip()
        self.assert_invalid(self.collect())
        executable.unlink()
        self.write_zip()
        self.assert_invalid(self.collect())

    def test_exact_resource_bytes_and_both_license_copies(self):
        self.stage()
        self.write_zip()
        before = self.collect()
        for name in ('LICENSE', RESOURCES + 'LICENSE', RESOURCES + 'services/kokoro-runtime/runtime.py',
                     RESOURCES + 'KilsatStudio.icns', APP + '/Contents/Info.plist'):
            with self.subTest(path=name):
                self.write_zip(replacements={name: b'tampered bytes'})
                after = self.collect()
                self.assert_invalid(after)
                self.assertNotEqual(before['artifact']['sha256'], after['artifact']['sha256'])
                original = next(record for record in before['files'] if record['path'] == name)
                altered = next(record for record in after['files'] if record['path'] == name)
                self.assertNotEqual(original['sha256'], altered['sha256'])
        (self.distribution / 'LICENSE').unlink()
        self.write_zip()
        self.assert_invalid(self.collect())

    def test_build_identity_is_exact_and_not_just_a_substring(self):
        self.stage()
        self.write_zip()
        for parameters in ({'commit': 'b' * 40}, {'target': 'x86_64-apple-darwin'}, {'profile': 'release'}):
            with self.subTest(parameters=parameters):
                self.assert_invalid(self.collect(**parameters))
        build = (self.distribution / (RESOURCES + 'BUILD-INFO.txt')).read_bytes()
        self.write_zip(replacements={RESOURCES + 'BUILD-INFO.txt': build + b'Commit: surprise\n'})
        self.assert_invalid(self.collect())

    def test_generated_plist_values_extra_keys_and_icon_bytes_are_exact(self):
        self.stage()
        name = APP + '/Contents/Info.plist'
        original = plistlib.loads((self.distribution / name).read_bytes())
        for change in ({'CFBundleVersion': '2'}, {'UnexpectedKey': 'extra'},
                       {'NSHighResolutionCapable': False},
                       {'CFBundleExecutable': 'different-executable'}):
            with self.subTest(change=change):
                altered = dict(original, **change)
                self.write_zip(replacements={name: plistlib.dumps(altered)})
                self.assert_invalid(self.collect())
        icon_name = RESOURCES + 'KilsatStudio.icns'
        icon = bytearray((self.distribution / icon_name).read_bytes())
        icon[-1] ^= 1
        self.write_zip(replacements={icon_name: bytes(icon)})
        self.assert_invalid(self.collect())

    def test_unknown_models_wheels_native_libraries_and_executables_are_not_allowed(self):
        self.stage()
        for name in ('model.safetensors', 'dependency.whl', 'libforeign.dylib', 'ffmpeg', 'new-resource.txt'):
            with self.subTest(name=name):
                self.write_zip(extras=[(RESOURCES + name, b'not part of approved package boundary')])
                report = self.collect()
                self.assert_invalid(report)
                self.assertIn(RESOURCES + name, [record['path'] for record in report['files']])

    def test_signature_is_inventoried_but_never_claimed_verified_or_approved(self):
        self.stage()
        name = APP + '/Contents/_CodeSignature/CodeResources'
        self.write_zip(extras=[(name, b'fake signature fixture; no cryptographic assertion')])
        report = self.collect()
        self.assertTrue(report['package_valid'], report['errors'])
        record = next(record for record in report['files'] if record['path'] == name)
        self.assertIn('signature', record['classification'])
        self.assertFalse(report['distribution_approved'])
        self.assertIn('sign', json.dumps(report['unresolved']).lower())

    def test_unsafe_and_ambiguous_names_are_rejected_without_extraction(self):
        self.stage()
        for name in ('/absolute', '../escape', 'nested/../../escape', 'nested\\escape',
                     './LICENSE', 'nested//file', 'nested/./file', 'C:/escape'):
            with self.subTest(name=name):
                self.write_zip(extras=[(name, b'unsafe')])
                self.assert_invalid(self.collect())
        self.assertFalse((self.root / 'escape').exists())

    def test_duplicate_paths_symlinks_and_special_members_are_rejected(self):
        self.stage()
        with warnings.catch_warnings():
            warnings.simplefilter('ignore', UserWarning)
            self.write_zip(extras=[('LICENSE', b'duplicate')])
        self.assert_invalid(self.collect())
        for kind in (stat.S_IFLNK, stat.S_IFIFO, stat.S_IFCHR):
            with self.subTest(kind=kind):
                info = zipfile.ZipInfo(RESOURCES + 'unsafe')
                info.create_system = 3
                info.external_attr = (kind | 0o777) << 16
                self.write_zip(extras=[(info, b'target')])
                self.assert_invalid(self.collect())

    def test_encrypted_member_is_rejected(self):
        self.stage()
        self.write_zip()
        # Set the encryption bit in both headers without creating/using a decryptor.
        raw = bytearray(self.archive.read_bytes())
        for marker, offset in ((b'PK\x03\x04', 6), (b'PK\x01\x02', 8)):
            index = raw.find(marker)
            self.assertGreaterEqual(index, 0)
            raw[index + offset] |= 1
        self.archive.write_bytes(raw)
        self.assert_invalid(self.collect())

    def test_raw_nul_filename_and_directory_type_disagreements_are_rejected(self):
        self.stage()
        self.write_zip(extras=[('nulXname', b'unsafe')])
        raw = self.archive.read_bytes()
        self.assertEqual(raw.count(b'nulXname'), 2)
        self.archive.write_bytes(raw.replace(b'nulXname', b'nul\x00name'))
        self.assert_invalid(self.collect())
        for name, kind in ((RESOURCES + 'examples/', stat.S_IFREG),
                           (RESOURCES + 'not-a-directory', stat.S_IFDIR)):
            with self.subTest(name=name):
                info = zipfile.ZipInfo(name)
                info.create_system = 3
                info.external_attr = (kind | 0o755) << 16
                self.write_zip(extras=[(info, b'')])
                self.assert_invalid(self.collect())

    def test_appledouble_metadata_requires_a_present_allowlisted_target(self):
        self.stage()
        metadata = '__MACOSX/' + APP + '/Contents/Resources/._LICENSE'
        directory_metadata = '__MACOSX/._' + APP
        self.write_zip(extras=[(metadata, b'opaque AppleDouble fixture'),
                               (directory_metadata, b'opaque directory metadata')])
        report = self.collect()
        self.assertTrue(report['package_valid'], report['errors'])
        for record in report['files']:
            if record['path'] in (metadata, directory_metadata):
                self.assertEqual(record['classification'], 'appledouble-unverified')
        for name in ('__MACOSX/' + RESOURCES + 'bin/._gpu_probe',
                     '__MACOSX/' + RESOURCES + '._model.safetensors',
                     '__MACOSX/arbitrary.txt'):
            with self.subTest(name=name):
                self.write_zip(extras=[(name, b'unknown or absent metadata target')])
                self.assert_invalid(self.collect())

    def test_archive_member_total_and_count_limits_are_enforced(self):
        self.stage()
        self.write_zip()
        for constant, limit in (('MAX_ARCHIVE_BYTES', 1), ('MAX_MEMBER_BYTES', 1),
                                ('MAX_TOTAL_BYTES', 1), ('MAX_ENTRIES', 1)):
            with self.subTest(constant=constant), patch.object(audit, constant, limit):
                self.assert_invalid(self.collect())

    def test_malformed_zip_yields_invalid_report(self):
        self.archive.write_bytes(b'not a zip')
        self.assert_invalid(self.collect())

    def test_malformed_xml_plist_returns_invalid_report_instead_of_raising(self):
        self.stage()
        self.write_zip(replacements={APP + '/Contents/Info.plist':
                                     b'<?xml version="1.0"?><plist><dict><key>broken</dict>'})
        self.assert_invalid(self.collect())

    def test_corrupt_deflate_payload_returns_invalid_report_instead_of_raising(self):
        self.stage()
        self.write_zip()
        with zipfile.ZipFile(self.archive) as archive:
            member = archive.getinfo('LICENSE')
            self.assertEqual(member.compress_type, zipfile.ZIP_DEFLATED)
            offset = member.header_offset
        raw = bytearray(self.archive.read_bytes())
        name_length, extra_length = struct.unpack_from('<HH', raw, offset + 26)
        payload_offset = offset + 30 + name_length + extra_length
        # BTYPE=3 is reserved by DEFLATE: fail decompression before CRC validation.
        raw[payload_offset] = (raw[payload_offset] & ~7) | 7
        self.archive.write_bytes(raw)
        self.assert_invalid(self.collect())

    def test_atomic_report_and_cli_collection_strict_and_invalid_exits(self):
        self.stage()
        self.write_zip()
        output = self.root / 'report.json'
        report = self.collect()
        audit.write_report(report, output)
        self.assertEqual(json.loads(output.read_text()), report)
        original = output.read_bytes()
        with patch.object(audit, 'MAX_REPORT_BYTES', 1):
            with self.assertRaises(ValueError):
                audit.write_report(report, output)
        self.assertEqual(output.read_bytes(), original)
        args = ['--archive', str(self.archive), '--repo', str(REPO), '--commit', COMMIT,
                '--target', TARGET, '--profile', 'debug', '--output', str(output)]
        self.assertEqual(audit.main(args), 0)
        self.assertEqual(audit.main(args + ['--strict']), 1)
        self.archive.write_bytes(b'invalid')
        self.assertEqual(audit.main(args), 2)
        self.assertFalse(json.loads(output.read_text())['package_valid'])

    def test_required_signature_metadata_is_presence_only_and_not_approval(self):
        self.stage()
        self.write_zip()
        output = self.root / 'signature-report.json'
        args = ['--archive', str(self.archive), '--repo', str(REPO), '--commit', COMMIT,
                '--target', TARGET, '--profile', 'debug', '--output', str(output),
                '--require-signature-metadata']
        self.assertEqual(audit.main(args), 2)
        self.assertFalse(json.loads(output.read_text())['package_valid'])
        self.write_zip(extras=[(APP + '/Contents/_CodeSignature/CodeResources', b'not verified')])
        self.assertEqual(audit.main(args), 0)
        report = json.loads(output.read_text())
        self.assertTrue(report['package_valid'])
        self.assertTrue(report['signature_metadata_present'])
        self.assertFalse(report['distribution_approved'])
        self.assertEqual(audit.main(args + ['--strict']), 1)

    def test_cli_refuses_input_output_same_path_hardlink_and_symlink(self):
        self.stage()
        self.write_zip()
        original = self.archive.read_bytes()
        hardlink = self.root / 'hardlink.zip'
        os.link(self.archive, hardlink)
        symlink = self.root / 'symlink.zip'
        symlink.symlink_to(self.archive)
        args = ['--archive', str(self.archive), '--repo', str(REPO), '--commit', COMMIT,
                '--target', TARGET, '--profile', 'debug']
        for output in (self.archive, hardlink, symlink):
            with self.subTest(output=output):
                self.assertEqual(audit.main(args + ['--output', str(output)]), 2)
                self.assertEqual(self.archive.read_bytes(), original)
        unrelated = self.root / 'unrelated.txt'
        unrelated.write_bytes(b'preserve existing target')
        other_symlink = self.root / 'other-symlink.json'
        other_symlink.symlink_to(unrelated)
        self.assertEqual(audit.main(args + ['--output', str(other_symlink)]), 2)
        self.assertEqual(unrelated.read_bytes(), b'preserve existing target')


if __name__ == '__main__':
    unittest.main()
