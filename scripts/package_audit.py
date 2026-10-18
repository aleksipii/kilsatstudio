#!/usr/bin/env python3
"""Inventory a final preview ZIP offline. Collection never approves distribution.

No extraction, package imports, subprocesses or downloads. SHA256 binds observed
bytes, not their origin, executable architecture, signature validity or rights.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import re
import stat
import struct
import sys
import tempfile
import zipfile

MAX_ARCHIVE_BYTES = 128 * 1024 * 1024
MAX_MEMBER_BYTES = 128 * 1024 * 1024
MAX_TOTAL_BYTES = 512 * 1024 * 1024
MAX_ENTRIES = 4096
MAX_REPORT_BYTES = 8 * 1024 * 1024
MAX_DIRECTORY_BYTES = 4 * 1024 * 1024
CHUNK = 64 * 1024
APP = 'Kilsat Studio Preview.app'
CONTENTS = APP + '/Contents/'
RESOURCES = CONTENTS + 'Resources/'
DESKTOP = CONTENTS + 'MacOS/studio-desktop'
HELPER = RESOURCES + 'bin/gpu_probe'
SIGNATURE = CONTENTS + '_CodeSignature/CodeResources'
SOURCE_FILES = (
    'LICENSE', 'services/kokoro-runtime/runtime.py',
    'services/kokoro-runtime/doctor.py', 'services/kokoro-runtime/smoke_macos.py',
    'services/kokoro-runtime/acceptance.py', 'services/kokoro-runtime/setup_kokoro.py',
    'services/kokoro-runtime/setup-manifest.json',
    'services/kokoro-runtime/requirements.txt', 'services/kokoro-runtime/README.md',
    'docs/practical-testing.md',
)


def _hash_stream(stream, limit):
    digest, length = hashlib.sha256(), 0
    while True:
        chunk = stream.read(min(CHUNK, limit - length + 1))
        if not chunk:
            return digest.hexdigest(), length
        length += len(chunk)
        if length > limit:
            raise ValueError('byte budget exceeded')
        digest.update(chunk)


def _regular(path):
    if not stat.S_ISREG(path.lstat().st_mode):
        raise ValueError('not a regular non-symlink file: ' + str(path))


def _source_identity(repo, relative):
    path = repo / relative
    for parent in path.relative_to(repo).parents:
        if (repo / parent).is_symlink():
            raise ValueError('symlinked source directory: ' + str(parent))
    _regular(path)
    with path.open('rb') as stream:
        return _hash_stream(stream, MAX_MEMBER_BYTES)


def _path(name, directory=False):
    if len(name.encode('utf-8')) > 4096 or not name or '\\' in name:
        raise ValueError('invalid ZIP path: ' + repr(name[:120]))
    value = name[:-1] if directory and name.endswith('/') else name
    parts = value.split('/')
    if any(p in ('', '.', '..') for p in parts) or any(
            ord(c) < 32 or ord(c) == 127 or c == ':' for c in value):
        raise ValueError('noncanonical ZIP path: ' + repr(name[:120]))
    return value


def _ancestors(paths):
    return {str(p) for name in paths for p in Path(name).parents if str(p) != '.'}


def _directory_budget(stream):
    # Bound central-directory allocation before ZipFile reads it. Preview ZIPs
    # are small, single-volume, non-ZIP64 archives; larger formats fail closed.
    stream.seek(0, 2)
    size = stream.tell()
    if size > MAX_ARCHIVE_BYTES:
        raise ValueError('archive byte budget exceeded')
    stream.seek(max(0, size - 65557))
    tail = stream.read(65557)
    offset = tail.rfind(b'PK\x05\x06')
    if offset < 0 or len(tail) - offset < 22:
        raise ValueError('missing ZIP end record')
    fields = struct.unpack('<4s4H2LH', tail[offset:offset + 22])
    _, disk, start_disk, local_count, count, directory_size, directory_at, comment = fields
    if disk or start_disk or local_count != count or count == 65535:
        raise ValueError('multi-volume/ZIP64 archive unsupported')
    if count > MAX_ENTRIES or directory_size > MAX_DIRECTORY_BYTES:
        raise ValueError('ZIP entry/central-directory budget exceeded')
    end_at = size - len(tail) + offset
    if directory_at + directory_size != end_at or offset + 22 + comment != len(tail):
        raise ValueError('noncanonical ZIP directory/end record')


def audit_package(archive, repo, commit, target, profile):
    """Return a bounded fact report; invalid input remains visible as errors."""
    archive, repo = Path(archive), Path(repo)
    report = dict(schema_version=1, distribution_approved=False, package_valid=False,
                  artifact=dict(filename=archive.name, sha256=None, size_bytes=None,
                                commit=commit, target=target, profile=profile),
                  files=[], components=[], build_evidence=[], errors=[],
                  external_requirements=[
                      dict(component=name, boundary='external-not-bundled', review_status='unreviewed')
                      for name in ('Host Python', 'Kokoro Python dependencies', 'Model/config/voices',
                                   'Language data', 'FFmpeg', 'Rhubarb', 'eSpeak')],
                  unresolved=[
                      'Exact target/feature graphs and compiler identities not collected.',
                      'Executable architecture, native linkage and code signatures not verified.',
                      'Third-party copyright/NOTICE texts and license alternatives not reviewed.',
                      'Source/redistribution obligations and external asset rights unresolved.'])
    errors = report['errors']
    try:
        # Load only our adjacent checkout code, never modules from the ZIP or
        # supplied resource root. Share generated metadata with the packager.
        spec = importlib.util.spec_from_file_location(
            'audit_package_layout', Path(__file__).with_name('package_macos.py'))
        layout = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(layout)
        if not re.fullmatch(r'[0-9a-f]{40}', commit):
            raise ValueError('expected commit must be a full lowercase Git SHA')
        arch = {'aarch64-apple-darwin': 'arm64', 'x86_64-apple-darwin': 'x86_64'}.get(target)
        if arch is None or profile not in ('debug', 'release'):
            raise ValueError('unsupported expected target/profile')
        _regular(archive)
        expected = {RESOURCES + relative: _source_identity(repo, relative)
                    for relative in SOURCE_FILES}
        expected['LICENSE'] = expected[RESOURCES + 'LICENSE']
        if expected['LICENSE'][1] == 0:
            raise ValueError('source LICENSE is empty')
        examples = repo / 'examples'
        if examples.is_symlink() or not examples.is_dir():
            raise ValueError('missing or symlinked examples directory')
        for source in sorted(examples.glob('*.yaml')):
            relative = 'examples/' + source.name
            expected[RESOURCES + relative] = _source_identity(repo, relative)
        required = set(expected) | {DESKTOP, CONTENTS + 'Info.plist',
                                    RESOURCES + 'KilsatStudio.icns', RESOURCES + 'BUILD-INFO.txt'}
        admitted = required | {HELPER, SIGNATURE}
        parents = _ancestors(admitted)
        observed, total = {}, 0
        small = {}
        with archive.open('rb') as raw:
            before = os.fstat(raw.fileno())
            report['artifact']['sha256'], report['artifact']['size_bytes'] = _hash_stream(
                raw, MAX_ARCHIVE_BYTES)
            _directory_budget(raw)
            raw.seek(0)
            with zipfile.ZipFile(raw) as package:
                members = package.infolist()
                if len(members) > MAX_ENTRIES:
                    raise ValueError('ZIP entry count budget exceeded')
                names = set()
                for member in members:
                    name = _path(member.filename, member.is_dir())
                    if member.orig_filename != member.filename or name in names:
                        raise ValueError('duplicate or truncated ZIP path: ' + name)
                    names.add(name)
                    mode = member.external_attr >> 16
                    kind = stat.S_IFMT(mode)
                    if kind not in (0, stat.S_IFDIR if member.is_dir() else stat.S_IFREG):
                        raise ValueError('symlink or special ZIP member: ' + name)
                    if mode & 0o7000:
                        raise ValueError('privileged ZIP member mode: ' + name)
                    if member.flag_bits & 1 or member.compress_type not in (
                            zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED):
                        raise ValueError('encrypted/unsupported ZIP member: ' + name)
                    if member.file_size > MAX_MEMBER_BYTES or total + member.file_size > MAX_TOTAL_BYTES:
                        raise ValueError('member/aggregate byte budget exceeded: ' + name)
                    total += member.file_size
                    if member.is_dir():
                        metadata_dir = name == '__MACOSX' or (
                            name.startswith('__MACOSX/') and name[len('__MACOSX/'):] in parents | admitted)
                        if name not in parents and not metadata_dir:
                            errors.append('unexpected directory: ' + name)
                        if member.file_size:
                            errors.append('directory has data: ' + name)
                        continue
                    with package.open(member) as stream:
                        digest, size = _hash_stream(stream, min(MAX_MEMBER_BYTES, member.file_size))
                    if size != member.file_size:
                        raise ValueError('ZIP member size mismatch: ' + name)
                    classification = 'unknown'
                    if name in expected:
                        classification = 'notice' if name.endswith('/LICENSE') or name == 'LICENSE' else 'resource'
                        if (digest, size) != expected[name]:
                            errors.append('source bytes differ: ' + name)
                    elif name in (DESKTOP, HELPER):
                        classification = 'executable'
                        if not mode & 0o111 or size == 0:
                            errors.append('missing executable permission or empty binary: ' + name)
                    elif name == SIGNATURE:
                        classification = 'signature-unverified'
                    elif name in required:
                        classification = 'generated-resource'
                    elif name.startswith('__MACOSX/'):
                        path = Path(name[len('__MACOSX/'):])
                        target_name = str(path.with_name(path.name[2:])) if path.name.startswith('._') else ''
                        if target_name in admitted | parents:
                            classification = 'appledouble-unverified'
                    if classification == 'unknown':
                        errors.append('unexpected file: ' + name)
                    observed[name] = (digest, size)
                    report['files'].append(dict(path=name, size_bytes=size, sha256=digest,
                                                mode=mode, classification=classification))
                    if name in (CONTENTS + 'Info.plist', RESOURCES + 'BUILD-INFO.txt',
                                RESOURCES + 'KilsatStudio.icns'):
                        if size > 1024 * 1024:
                            raise ValueError('generated metadata byte budget exceeded: ' + name)
                        small[name] = package.read(member)
                for name in sorted(required - observed.keys()):
                    errors.append('missing required file: ' + name)
                # Metadata may name an optional known path only if actually present.
                for row in report['files']:
                    if row['classification'] == 'appledouble-unverified':
                        path = Path(row['path'][len('__MACOSX/'):])
                        real = str(path.with_name(path.name[2:]))
                        actual_dirs = _ancestors(observed)
                        if real not in observed and real not in actual_dirs:
                            errors.append('orphan AppleDouble metadata: ' + row['path'])
            after = os.fstat(raw.fileno())
            raw.seek(0)
            final_digest, final_size = _hash_stream(raw, MAX_ARCHIVE_BYTES)
            if (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (
                    after.st_size, after.st_mtime_ns, after.st_ctime_ns) or (
                    final_digest, final_size) != (report['artifact']['sha256'], report['artifact']['size_bytes']):
                raise ValueError('archive changed during inspection')
        info = small.get(RESOURCES + 'BUILD-INFO.txt')
        expected_info = (f'Commit: {commit}\nArchitecture: {arch}\nProfile: {profile}\n'
                         'Preview build; ad-hoc signed, not notarized.\n'
                         'No model weights, voice assets, Python runtime or Python dependencies included.\n').encode()
        if info is not None and info != expected_info:
            errors.append('BUILD-INFO fields differ from expected commit/target/profile')
        plist = small.get(CONTENTS + 'Info.plist')
        if plist is not None:
            document = plistlib.loads(plist)
            if document != layout.bundle_info():
                errors.append('Info.plist application metadata mismatch')
        icon = small.get(RESOURCES + 'KilsatStudio.icns')
        if icon is not None and icon != layout.app_icon():
            errors.append('generated ICNS bytes differ from source')
        report['components'] = [dict(path=name, sha256=observed[name][0],
                                     target=target, profile=profile, graph_status='not-collected',
                                     native_linkage_status='not-verified', review_status='unreviewed')
                                for name in (DESKTOP, HELPER) if name in observed]
        report['gpu_helper_present'] = HELPER in observed
        report['signature_metadata_present'] = SIGNATURE in observed
    except (OSError, ValueError, RuntimeError, zipfile.BadZipFile, EOFError,
            NotImplementedError, plistlib.InvalidFileException) as exc:
        errors.append(str(exc))
    report['files'].sort(key=lambda row: row['path'])
    report['errors'] = sorted(set(errors))
    report['package_valid'] = not report['errors']
    return report


def write_report(report, output):
    """Replace a regular sidecar atomically; never follow an output symlink."""
    output = Path(output)
    payload = (json.dumps(report, sort_keys=True, indent=2, ensure_ascii=True) + '\n').encode()
    if len(payload) > MAX_REPORT_BYTES:
        raise ValueError('report byte budget exceeded')
    if output.exists() or output.is_symlink():
        _regular(output)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=output.parent, prefix='.package-audit-', delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, output)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--commit', required=True)
    parser.add_argument('--target', required=True)
    parser.add_argument('--profile', choices=('debug', 'release'), required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--strict', action='store_true', help='Exit 1 while legal review remains unresolved')
    parser.add_argument('--require-signature-metadata', action='store_true',
                        help='Require CodeResources presence; does not verify a signature')
    args = parser.parse_args(argv)
    try:
        if args.archive.resolve() == args.output.resolve() or (
                args.output.exists() and args.archive.exists() and args.output.samefile(args.archive)):
            raise ValueError('audit output must differ from the input archive')
        report = audit_package(args.archive, args.repo, args.commit, args.target, args.profile)
        if args.require_signature_metadata and not report.get('signature_metadata_present'):
            report['errors'].append('required CodeResources signature metadata is missing')
            report['package_valid'] = False
        write_report(report, args.output)
        print(json.dumps(dict(package_valid=report['package_valid'],
                              distribution_approved=report['distribution_approved'],
                              archive_sha256=report['artifact']['sha256'],
                              file_count=len(report['files']), errors=report['errors']),
                         sort_keys=True))
    except (OSError, ValueError) as exc:
        print(str(exc), file=sys.stderr)
        return 2
    return 2 if not report['package_valid'] else 1 if args.strict else 0


if __name__ == '__main__':
    raise SystemExit(main())
