#!/usr/bin/env python3
"""Offline evidence inventory; declarations are never redistribution approval.

Python 3.11+. Reads supplied files only; never invokes Cargo, pip or a binary.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
import tomllib


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(chunk)
    return value.hexdigest()


def normalized(name):
    return re.sub(r'[-_.]+', '-', name).lower()


def build_inventory(root, metadata=None, pip_inspect=None, binaries=()):
    for label, document, field in [('Cargo metadata', metadata, 'packages'),
                                    ('pip inspect', pip_inspect, 'installed')]:
        if document is not None:
            if not isinstance(document, dict) or not isinstance(document.get(field), list):
                raise ValueError(f'{label} must contain a {field} list')
            if not all(isinstance(item, dict) for item in document[field]):
                raise ValueError(f'{label} entries must be objects')
    lock_path = root / 'Cargo.lock'
    setup_path = root / 'services/kokoro-runtime/setup-manifest.json'
    requirements_path = root / 'services/kokoro-runtime/requirements.txt'
    lock = tomllib.loads(lock_path.read_text())
    setup = json.loads(setup_path.read_text())
    metadata_packages = (metadata or {}).get('packages', [])
    rust = []
    for package in lock['package']:
        # Source is part of identity: same name/version from a different registry
        # must not supply a misleading license declaration.
        candidates = [p for p in metadata_packages if
                      (p.get('name'), p.get('version'), p.get('source')) ==
                      (package['name'], package['version'], package.get('source'))]
        matched = candidates[0] if len(candidates) == 1 else {}
        rust.append(dict(name=package['name'], version=package['version'],
                         source=package.get('source', 'workspace-or-path'),
                         checksum=package.get('checksum'),
                         declared_license=matched.get('license'),
                         declared_license_file=matched.get('license_file'),
                         evidence='matching-cargo-metadata' if matched else 'lock-identity-only',
                         notices_status='not-collected', review_status='unreviewed'))
    rust.sort(key=lambda p: (p['name'], p['version'], p['source']))
    installed = []
    for distribution in (pip_inspect or {}).get('installed', []):
        data = distribution.get('metadata', {})
        if not isinstance(data, dict) or not data.get('name') or not data.get('version'):
            raise ValueError('pip inspect distribution lacks name or version')
        installed.append(dict(name=data['name'], version=data['version'],
                              declared_license=data.get('license_expression') or data.get('license'),
                              classifiers=sorted(data.get('classifier', [])),
                              direct_url=distribution.get('direct_url'),
                              installer=distribution.get('installer'),
                              artifact_hash_status='not-established-by-pip-inspect',
                              notices_status='not-collected', review_status='unreviewed'))
    installed.sort(key=lambda p: (normalized(p['name']), p['version']))
    packages = []
    for spec in setup['packages']:
        match = re.fullmatch(r'([A-Za-z0-9_.-]+)(\[[A-Za-z0-9_,.-]+\])?==([^=<>!~\s]+)', spec)
        if not match:
            raise ValueError('managed package is not an exact version pin: ' + spec)
        name, extras, version = match.groups()
        observed = [p['version'] for p in installed if normalized(p['name']) == normalized(name)]
        packages.append(dict(requirement=spec, name=name, extras=extras or '', version=version,
                             observed_versions=observed,
                             match_status=('not-observed' if not observed else
                                           'matches' if observed == [version] else 'mismatch'),
                             artifact_hash_status='unpinned', review_status='unreviewed'))
    assets = []
    revision = setup['model_revision']
    for item in setup['files']:
        immutable = f'/resolve/{revision}/' in item['url'] and bool(re.fullmatch('[0-9a-f]{40}', revision))
        hash_valid = bool(re.fullmatch('[0-9a-f]{64}', item['sha256']))
        assets.append(dict(path=item['path'], repository=setup['model_repository'], revision=revision,
                           url=item['url'], sha256=item['sha256'], size_bytes=item['size_bytes'],
                           pin_status='pinned' if immutable and hash_valid else 'invalid-pin',
                           local_bytes_status='not-inspected', license_status='unverified-at-revision',
                           notices_status='not-collected', review_status='unreviewed'))
    language_url = setup['english_model_url']
    language_revision = re.search(r'/resolve/([0-9a-f]{40})/', language_url)
    language = dict(filename=setup['english_model_filename'], url=language_url,
                    revision=language_revision.group(1) if language_revision else None,
                    sha256=setup['english_model_sha256'], size_bytes=setup['english_model_size_bytes'],
                    pin_status='pinned' if language_revision and re.fullmatch('[0-9a-f]{64}', setup['english_model_sha256']) else 'invalid-pin',
                    local_bytes_status='not-inspected', license_status='unverified-at-revision',
                    notices_status='not-collected', review_status='unreviewed')
    binary_rows = []
    for label, path in binaries:
        binary_rows.append(dict(name=label, sha256=digest(path), size_bytes=path.stat().st_size,
                                version_status='not-executed', provenance_status='unknown',
                                license_status='unknown', review_status='unreviewed'))
    gaps = ['License texts, NOTICE files, bundled fonts/data and native-library notices are not collected.',
            'Declared license strings are evidence only; SPDX expressions and obligations require review.',
            'Cargo.lock includes target-dependent/build dependencies; this is not a shipped-binary SBOM.',
            'Python direct pins do not lock transitive wheels, platform tags or artifact hashes.',
            'Model and English-wheel pins are manifest declarations, not verification of local bytes.',
            'Voice provenance and redistribution applicability remain unresolved.',
            'Host Python, FFmpeg and eSpeak/native-library provenance require selected-artifact review.']
    if not metadata:
        gaps.append('Cargo metadata absent: Rust dependency license declarations unresolved.')
    if not pip_inspect:
        gaps.append('Installed Python inventory absent: no resolved-environment evidence.')
    return dict(schema_version=1, distribution_approved=False,
                inputs={'Cargo.lock': digest(lock_path), 'setup-manifest.json': digest(setup_path),
                        'requirements.txt': digest(requirements_path),
                        'cargo_metadata_sha256': hashlib.sha256(json.dumps(metadata, sort_keys=True).encode()).hexdigest() if metadata else None,
                        'pip_inspect_sha256': hashlib.sha256(json.dumps(pip_inspect, sort_keys=True).encode()).hexdigest() if pip_inspect else None},
                rust=rust, python_direct=packages, python_installed=installed,
                development_requirements=[line.strip() for line in requirements_path.read_text().splitlines()
                                          if line.strip() and not line.lstrip().startswith('#')],
                model_assets=assets, english_model=language,
                binaries=sorted(binary_rows, key=lambda p: (p['name'], p['sha256'])), unresolved=gaps)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--cargo-metadata', type=Path, help='Existing cargo metadata JSON, not a command')
    parser.add_argument('--pip-inspect', type=Path, help='Existing pip inspect JSON from the selected environment')
    parser.add_argument('--binary', action='append', default=[], metavar='NAME=PATH', help='Hash a selected binary without executing it')
    parser.add_argument('--output', type=Path, help='Write JSON here; default stdout')
    parser.add_argument('--strict', action='store_true', help='Exit 1 while unresolved review work remains')
    args = parser.parse_args(argv)
    try:
        binaries = []
        for item in args.binary:
            name, separator, path = item.partition('=')
            if not separator or not name or not path:
                raise ValueError('--binary requires NAME=PATH')
            binaries.append((name, Path(path)))
        report = build_inventory(args.root,
                                 json.loads(args.cargo_metadata.read_text()) if args.cargo_metadata else None,
                                 json.loads(args.pip_inspect.read_text()) if args.pip_inspect else None,
                                 binaries)
        result = json.dumps(report, indent=2, sort_keys=True) + '\n'
        if args.output:
            args.output.write_text(result)
        else:
            print(result, end='')
        return 1 if args.strict and report['unresolved'] else 0
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f'Inventory failed: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    sys.exit(main())
