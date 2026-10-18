#!/usr/bin/env python3
"""Collect saved build observations offline; never execute or approve components.

Cargo metadata describes resolved workspace features, not rustc compile units.
Tool output is an unauthenticated observation supplied by the caller. Its hash
binds those bytes, not the machine or process that originally produced them.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import stat
import sys
import tomllib
import zipfile

MAX_INPUT_BYTES = 16 * 1024 * 1024
MAX_ARCHIVE_BYTES = 128 * 1024 * 1024
MAX_PACKAGES = 2048
MAX_OUTPUT_BYTES = 16 * 1024 * 1024


def read_bound(path, limit=MAX_INPUT_BYTES):
    path = Path(path)
    if not stat.S_ISREG(path.lstat().st_mode):
        raise ValueError('input must be a regular non-symlink file')
    with path.open('rb') as stream:
        data = stream.read(limit + 1)
    if len(data) > limit:
        raise ValueError('input byte budget exceeded')
    return data


def identity(data):
    return dict(sha256=hashlib.sha256(data).hexdigest(), size_bytes=len(data))


def object_value(value, label):
    if not isinstance(value, dict):
        raise ValueError(label + ' must be an object')
    return value


def object_rows(value, label, limit=MAX_PACKAGES):
    if not isinstance(value, list) or len(value) > limit:
        raise ValueError(label + ' must be a bounded array')
    for row in value:
        object_value(row, label + ' entry')
    return value


def strings(value, label):
    if not isinstance(value, list) or len(value) > MAX_PACKAGES or not all(
            isinstance(item, str) for item in value):
        raise ValueError(label + ' must be a bounded string array')
    return value


def graph(metadata, lock, root_name):
    object_value(metadata, 'metadata')
    object_value(lock, 'lock')
    packages = object_rows(metadata.get('packages'), 'packages')
    resolution = object_value(metadata.get('resolve'), 'resolve')
    nodes = object_rows(resolution.get('nodes'), 'nodes')
    strings(metadata.get('workspace_members'), 'workspace members')
    for package in packages:
        if not all(isinstance(package.get(key), str) for key in ('id', 'name', 'version')):
            raise ValueError('package identity fields must be strings')
        if package.get('source') is not None and not isinstance(package['source'], str):
            raise ValueError('package source must be a string or null')
    for node in nodes:
        if not isinstance(node.get('id'), str):
            raise ValueError('node identity must be a string')
        strings(node.get('dependencies', []), 'dependencies')
        strings(node.get('features', []), 'features')
    lock_rows = object_rows(lock.get('package'), 'lock packages')
    for package in lock_rows:
        if not all(isinstance(package.get(key), str) for key in ('name', 'version')):
            raise ValueError('lock identity fields must be strings')
        if package.get('source') is not None and not isinstance(package['source'], str):
            raise ValueError('lock source must be a string or null')
    if len(packages) > MAX_PACKAGES or len(nodes) > MAX_PACKAGES:
        raise ValueError('resolved package budget exceeded')
    by_id = {p['id']: p for p in packages}
    resolved = {n['id']: n for n in nodes}
    if len(by_id) != len(packages) or len(resolved) != len(nodes):
        raise ValueError('duplicate package/resolve identity')
    roots = [p['id'] for p in packages if p['name'] == root_name
             and p['id'] in metadata.get('workspace_members', [])]
    if len(roots) != 1:
        raise ValueError('expected exactly one workspace root: ' + root_name)
    if root_name == 'motion-render' and 'gpu' not in resolved.get(roots[0], {}).get('features', []):
        raise ValueError('helper metadata does not enable gpu feature')
    pending, seen, result = [roots[0]], set(), []
    locked = {(p['name'], p['version'], p.get('source')): p
              for p in lock.get('package', [])}
    while pending:
        package_id = pending.pop()
        if package_id in seen:
            continue
        seen.add(package_id)
        if package_id not in by_id or package_id not in resolved:
            raise ValueError('incomplete resolved graph')
        p, node = by_id[package_id], resolved[package_id]
        source = p.get('source')
        entry = locked.get((p['name'], p['version'], source))
        if entry is None:
            raise ValueError('resolved package does not match Cargo.lock: ' + p['name'])
        dependencies = sorted(node.get('dependencies', []))
        pending.extend(dependencies)
        result.append(dict(id=package_id, name=p['name'], version=p['version'],
                           source=source, lock_checksum=entry.get('checksum'),
                           declared_license=p.get('license'),
                           declared_license_file=p.get('license_file'),
                           features=sorted(node.get('features', [])),
                           dependencies=dependencies,
                           dependency_edges=node.get('deps', []),
                           license_review='unreviewed'))
    return dict(root=root_name, root_id=roots[0],
                scope='target-filtered Cargo metadata dependency closure; includes resolved build/dev edges; not compile-unit proof',
                packages=sorted(result, key=lambda p: p['id']))


def collect(archive, audit_path, lock_path, desktop_path, helper_path,
            rustc_path, cargo_path, observations_path):
    audit_bytes = read_bound(audit_path)
    audit = object_value(json.loads(audit_bytes), 'audit')
    archive_bytes = read_bound(archive, MAX_ARCHIVE_BYTES)
    archive_id = identity(archive_bytes)
    artifact = object_value(audit.get('artifact'), 'artifact')
    if (not audit.get('package_valid') or audit.get('distribution_approved') is not False
            or any(artifact.get(k) != v for k, v in archive_id.items())):
        raise ValueError('package audit is invalid or does not bind archive bytes')
    if not isinstance(artifact.get('commit'), str) or not re.fullmatch('[0-9a-f]{40}', artifact['commit']):
        raise ValueError('missing source commit')
    lock_bytes = read_bound(lock_path)
    lock = tomllib.loads(lock_bytes.decode('utf-8'))
    observations_bytes = read_bound(observations_path)
    observations = object_value(json.loads(observations_bytes), 'native observations')
    # The package inventory uses a fixed executable boundary. Do not infer
    # additional executables from caller-controlled tool text.
    required = ['Kilsat Studio Preview.app/Contents/MacOS/studio-desktop',
                'Kilsat Studio Preview.app/Contents/Resources/bin/gpu_probe']
    rows = object_rows(audit.get('files'), 'audit files', 4096)
    if not all(isinstance(row.get('path'), str) for row in rows):
        raise ValueError('audit file paths must be strings')
    files = {f['path']: f for f in rows}
    if len(files) != len(rows):
        raise ValueError('duplicate audit file path')
    if set(observations) != set(required) or not all(p in files for p in required):
        raise ValueError('expected separate desktop/helper native observations')
    with zipfile.ZipFile(io.BytesIO(archive_bytes)) as zipped:
        names = zipped.namelist()
        for path in required:
            if names.count(path) != 1:
                raise ValueError('missing or duplicate executable member')
            info = zipped.getinfo(path)
            if info.file_size > MAX_ARCHIVE_BYTES:
                raise ValueError('executable member byte budget exceeded')
            with zipped.open(info) as stream:
                data = stream.read(MAX_ARCHIVE_BYTES + 1)
            if len(data) > MAX_ARCHIVE_BYTES or identity(data)['sha256'] != files[path].get('sha256'):
                raise ValueError('audit executable identity differs from archive bytes')
    native = []
    for path in required:
        obs = object_value(observations[path], 'native observation entry')
        if obs.get('sha256') != files[path].get('sha256'):
            raise ValueError('native observation executable hash mismatch')
        if not all(isinstance(obs.get(k), str) and obs[k].strip()
                   for k in ('lipo_archs', 'otool_libraries')):
            raise ValueError('missing native tool observation')
        native.append(dict(path=path, sha256=obs['sha256'],
                           lipo_archs=obs['lipo_archs'],
                           otool_libraries=obs['otool_libraries'],
                           trust='caller-supplied tool output; not hardware attestation'))
    inputs = dict(package_audit=identity(audit_bytes), cargo_lock=identity(lock_bytes),
                  native_observations=identity(observations_bytes))
    graphs = []
    for name, path in [('studio-desktop', desktop_path), ('motion-render', helper_path)]:
        data = read_bound(path)
        inputs[name] = identity(data)
        graphs.append(graph(json.loads(data), lock, name))
    toolchain = {}
    for name, path in [('rustc', rustc_path), ('cargo', cargo_path)]:
        data = read_bound(path, 64 * 1024)
        value = data.decode('utf-8').strip()
        if not value or not value.startswith(name + ' '):
            raise ValueError('missing compiler/Cargo identity')
        toolchain[name] = dict(text=value, **identity(data))
    return dict(schema_version=1, distribution_approved=False,
                artifact=dict(artifact), inputs=inputs, graphs=graphs,
                toolchain=toolchain, native_observations=native,
                physical_device_acceptance='not_granted',
                unresolved=['Third-party license texts, SPDX choices, NOTICE and source obligations require human review.',
                            'Resolved Cargo graph is not proof of exact compiled or embedded components.',
                            'Local source contents, upstream origin and observation authenticity are not attested.',
                            'Physical Mac, offline speech, listening, GUI, Metal and synchronization acceptance remain separate.'])


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('archive', 'audit', 'lock', 'desktop-metadata', 'helper-metadata',
                 'rustc', 'cargo', 'native-observations', 'output'):
        parser.add_argument('--' + name, required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        report = collect(args.archive, args.audit, args.lock,
                         args.desktop_metadata, args.helper_metadata,
                         args.rustc, args.cargo, args.native_observations)
        data = (json.dumps(report, indent=2, sort_keys=True) + '\n').encode()
        if len(data) > MAX_OUTPUT_BYTES:
            raise ValueError('report byte budget exceeded')
        # New evidence only; never replace a previous device or audit record.
        with args.output.open('xb') as stream:
            stream.write(data)
    except (OSError, ValueError, KeyError, TypeError, RecursionError, zipfile.BadZipFile) as error:
        print('Build evidence failed: ' + str(error), file=sys.stderr)
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
