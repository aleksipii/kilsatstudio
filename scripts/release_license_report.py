#!/usr/bin/env python3
"""Bound supplied release/license evidence offline; collection never grants rights."""
import argparse
from datetime import date
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys
import tempfile

MAX_JSON = 16 * 1024 * 1024
MAX_TEXT = 1024 * 1024
MAX_TOTAL = 32 * 1024 * 1024
CATEGORIES = {'notice-copyright', 'modification', 'source', 'asset-provenance'}


def adjacent(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def identity(data):
    return dict(sha256=hashlib.sha256(data).hexdigest(), size_bytes=len(data))


def regular(path, limit):
    path = Path(path)
    for part in [path, *path.parents]:
        if part.is_symlink():
            raise ValueError('symlink input component')
    if not stat.S_ISREG(path.stat().st_mode):
        raise ValueError('input must be regular')
    with path.open('rb') as stream:
        data = stream.read(limit + 1)
    if len(data) > limit:
        raise ValueError('input byte budget exceeded')
    return data


def document(path):
    data = regular(path, MAX_JSON)
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError('duplicate JSON key')
            result[key] = value
        return result
    value = json.loads(data, object_pairs_hook=pairs)
    if not isinstance(value, dict) or type(value.get('schema_version')) is not int or value['schema_version'] != 1:
        raise ValueError('expected schema version 1 object')
    return value, identity(data)


def rows(value, label, limit=4096):
    if not isinstance(value, list) or len(value) > limit or not all(isinstance(x, dict) for x in value):
        raise ValueError(label + ' must be bounded object array')
    return value


def text(value, label):
    if not isinstance(value, str) or not value.strip() or len(value) > 4096:
        raise ValueError('missing or invalid ' + label)
    return value


def relative(value):
    text(value, 'relative path')
    p = PurePosixPath(value)
    if p.is_absolute() or '\\' in value or str(p) != value or any(x in ('', '.', '..') for x in value.split('/')):
        raise ValueError('noncanonical relative path')
    return value


def collect(archive, audit_path, build_path, index_path, corpus_root, repo):
    audit, audit_id = document(audit_path)
    build, build_id = document(build_path)
    index, index_id = document(index_path)
    artifact = audit.get('artifact')
    if not isinstance(artifact, dict):
        raise ValueError('missing artifact')
    checked = adjacent('package_audit').audit_package(
        Path(archive), Path(repo), artifact.get('commit'), artifact.get('target'), artifact.get('profile'))
    if not checked['package_valid'] or checked != audit:
        raise ValueError('audit differs from independently revalidated package')
    if not isinstance(build.get('inputs'), dict):
        raise ValueError('build inputs must be object')
    if (build.get('distribution_approved') is not False or build.get('physical_device_acceptance') != 'not_granted'
            or build.get('artifact') != artifact or build.get('inputs', {}).get('package_audit') != audit_id):
        raise ValueError('stale or unbound build evidence')
    files = {x['path']: x for x in audit['files']}
    native = rows(build.get('native_observations'), 'native observations')
    required = {adjacent('package_audit').DESKTOP, adjacent('package_audit').HELPER}
    if {x.get('path') for x in native} != required or len(native) != 2:
        raise ValueError('separate desktop/helper observations required')
    for row in native:
        if row.get('sha256') != files.get(row['path'], {}).get('sha256'):
            raise ValueError('stale executable observation')
    graphs = rows(build.get('graphs'), 'graphs', 2)
    if len(graphs) != 2 or {x.get('root') for x in graphs} != {'studio-desktop', 'motion-render'}:
        raise ValueError('separate nonempty graphs required')
    candidates = {}
    for graph in graphs:
        packages = rows(graph.get('packages'), 'graph packages', 2048)
        if not packages:
            raise ValueError('empty graph')
        seen = set()
        for package in packages:
            key = (graph['root'], text(package.get('id'), 'package id'))
            if key in seen:
                raise ValueError('duplicate graph candidate')
            seen.add(key)
            for field in ('name', 'version'):
                text(package.get(field), field)
            candidates[key] = package
    if index.get('artifact') != artifact or index.get('build_evidence') != build_id:
        raise ValueError('corpus index must bind exact artifact and build report')
    components = rows(index.get('components'), 'components')
    by_id = {}
    for component in components:
        key = text(component.get('id'), 'component id')
        if key in by_id:
            raise ValueError('duplicate component id')
        by_id[key] = component
    root = Path(corpus_root)
    if any(x.is_symlink() for x in [root, *root.parents]) or not root.is_dir():
        raise ValueError('corpus root must be directory')
    pending, file_count, corpus_bytes = [root], 0, 0
    while pending:
        directory = pending.pop()
        with os.scandir(directory) as entries:
            for entry in entries:
                if entry.is_symlink():
                    raise ValueError('symlink corpus component')
                if entry.is_dir(follow_symlinks=False):
                    pending.append(Path(entry.path))
                    file_count += 1
                elif entry.is_file(follow_symlinks=False):
                    file_count += 1
                    size = entry.stat(follow_symlinks=False).st_size
                    if size > MAX_TEXT:
                        raise ValueError('corpus text byte budget exceeded')
                    corpus_bytes += size
                else:
                    raise ValueError('special corpus file')
                if file_count > 4096 or corpus_bytes > MAX_TOTAL:
                    raise ValueError('corpus file/total budget exceeded')
    evidence, inodes, paths, total = {}, set(), set(), 0
    input_inodes = {(Path(p).stat().st_dev, Path(p).stat().st_ino) for p in (archive, audit_path, build_path, index_path)}
    for item in rows(index.get('evidence', []), 'evidence'):
        key = text(item.get('id'), 'evidence id')
        path = relative(item.get('path'))
        if key in evidence or path in paths:
            raise ValueError('duplicate evidence id/path')
        if item.get('kind') not in {'license', 'notice', 'copyright', 'source', 'modification', 'provenance'}:
            raise ValueError('unknown evidence kind')
        if item.get('component') not in by_id:
            raise ValueError('unknown evidence component')
        text(item.get('source'), 'evidence source association')
        if item['source'] != by_id[item['component']].get('source'):
            raise ValueError('evidence source differs from component identity')
        payload = regular(root / path, MAX_TEXT)
        info = (root / path).stat()
        inode = (info.st_dev, info.st_ino)
        if inode in inodes or inode in input_inodes:
            raise ValueError('hardlink/input evidence alias')
        inodes.add(inode)
        paths.add(path)
        total += len(payload)
        if total > MAX_TOTAL:
            raise ValueError('corpus total budget exceeded')
        if any(item.get(k) != v for k, v in identity(payload).items()):
            raise ValueError('corpus evidence hash/size mismatch')
        evidence[key] = item
    unresolved, mapped = [dict(code='raw-build-input-attestation-not-established', component='artifact'),
                          dict(code='release-authority-approval-required', component='artifact')], set()
    def gap(code, component):
        unresolved.append(dict(code=code, component=component))
    for key, component in sorted(by_id.items()):
        if component.get('boundary') not in {'bundled', 'local-install', 'external'}:
            raise ValueError('unknown component boundary')
        for field in ('name', 'version'):
            text(component.get(field), field)
        if not component.get('source') or not component.get('revision'):
            gap('upstream-identity-missing', key)
        if component.get('parent') is not None and (component['parent'] not in by_id or component['parent'] == key):
            raise ValueError('unknown/self parent component')
        ancestors, parent = {key}, component.get('parent')
        while parent is not None:
            if parent in ancestors or parent not in by_id:
                raise ValueError('cyclic/unknown parent component')
            ancestors.add(parent)
            parent = by_id[parent].get('parent')
        for member in rows(component.get('members', []), 'component members'):
            path = relative(member.get('path'))
            if files.get(path, {}).get('sha256') != member.get('sha256'):
                raise ValueError('component member differs from artifact')
            if component['boundary'] != 'bundled':
                raise ValueError('external component cannot claim bundled member')
        for ref in rows(component.get('graph_refs', []), 'graph references'):
            pair = (ref.get('graph'), ref.get('id'))
            package = candidates.get(pair)
            if package is None or pair in mapped:
                raise ValueError('unknown/duplicate graph mapping')
            if any(component.get(field) != package.get(field) for field in ('name', 'version', 'source')):
                raise ValueError('ambiguous graph component identity')
            if component['boundary'] != 'bundled':
                raise ValueError('graph candidates cannot be excluded without reviewed inclusion proof')
            mapped.add(pair)
        kinds = {x['kind'] for x in evidence.values() if x['component'] == key}
        if 'license' not in kinds:
            gap('license-text-missing', key)
        if 'provenance' not in kinds:
            gap('provenance-evidence-missing', key)
        if not component.get('embedded_inventory_complete'):
            gap('font-data-native-inventory-unreviewed', key)
        gap('human-component-approval-required', key)
    for graph, key in sorted(set(candidates) - mapped):
        gap('graph-candidate-unmapped', graph + ':' + key)
    reviews = rows(index.get('reviews', []), 'reviews')
    reviewed = set()
    for review in reviews:
        key = review.get('component')
        if key not in by_id or key in reviewed:
            raise ValueError('unknown/duplicate review component')
        reviewed.add(key)
        for field in ('reviewer', 'date', 'rationale', 'declared_expression'):
            text(review.get(field), field)
        if not re.fullmatch(r'\d{4}-\d{2}-\d{2}', review['date']):
            raise ValueError('invalid review date')
        date.fromisoformat(review['date'])
        component_digest = identity(json.dumps(by_id[key], sort_keys=True, separators=(',', ':')).encode())['sha256']
        if review.get('component_sha256') != component_digest:
            raise ValueError('stale component review digest')
        bindings = rows(review.get('evidence'), 'review evidence')
        referenced = set()
        for binding in bindings:
            item = evidence.get(binding.get('id'))
            if item is None or item['component'] != key or binding.get('sha256') != item['sha256'] or item['id'] in referenced:
                raise ValueError('stale/dangling review evidence')
            referenced.add(item['id'])
        if referenced != {x['id'] for x in evidence.values() if x['component'] == key}:
            raise ValueError('review must bind every component evidence text')
        if ' OR ' in review['declared_expression'] and not review.get('selected_alternative'):
            gap('license-alternative-unselected', key)
        obligations = rows(review.get('obligations'), 'obligations')
        categories = set()
        for obligation in obligations:
            category = obligation.get('category')
            if category not in CATEGORIES or category in categories:
                raise ValueError('unknown/duplicate obligation category')
            categories.add(category)
            text(obligation.get('rationale'), 'obligation rationale')
            if obligation.get('status') not in {'unreviewed', 'required', 'satisfied', 'reviewed-not-applicable'}:
                raise ValueError('unknown obligation status')
            refs = obligation.get('evidence', [])
            if not isinstance(refs, list) or any(x not in referenced for x in refs) or len(refs) != len(set(refs)):
                raise ValueError('dangling obligation evidence')
            if obligation['status'] in {'satisfied', 'reviewed-not-applicable'} and not refs:
                gap('obligation-support-missing', key + ':' + category)
            if obligation['status'] in {'unreviewed', 'required'}:
                gap('obligation-unresolved', key + ':' + category)
            if category == 'notice-copyright' and obligation['status'] != 'reviewed-not-applicable':
                destination = relative(obligation.get('destination'))
                digest = obligation.get('sha256')
                if not any(evidence[x]['sha256'] == digest for x in refs):
                    raise ValueError('notice digest lacks prepared evidence')
                if files.get(destination, {}).get('sha256') != digest:
                    gap('notice-not-packaged', key + ':' + destination)
        for category in sorted(CATEGORIES - categories):
            gap('obligation-category-missing', key + ':' + category)
    for key in sorted(set(by_id) - reviewed):
        gap('review-missing', key)
    return dict(schema_version=1, distribution_approved=False, physical_device_acceptance='not_granted',
                artifact=artifact, inputs=dict(audit=audit_id, build_evidence=build_id, corpus_index=index_id),
                components=sorted(components, key=lambda x: x['id']), evidence=sorted(evidence.values(), key=lambda x: x['id']),
                reviews=sorted(reviews, key=lambda x: x['component']), unresolved=sorted(unresolved, key=lambda x: (x['code'], x['component'])),
                trust='Local byte integrity and record completeness only; upstream claims and human judgments are not authenticated.')


def publish(report, output):
    output = Path(output)
    if output.exists() or output.is_symlink():
        raise ValueError('output must be new')
    for parent in output.parents:
        if parent.is_symlink():
            raise ValueError('output ancestor symlink')
    payload = (json.dumps(report, sort_keys=True, indent=2) + '\n').encode()
    if len(payload) > MAX_JSON:
        raise ValueError('output byte budget exceeded')
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=output.parent, prefix='.release-evidence-', delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        os.link(temporary, output)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ('archive', 'audit', 'build-evidence', 'corpus-index', 'corpus-root', 'output'):
        parser.add_argument('--' + flag, type=Path, required=True)
    parser.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--strict', action='store_true')
    args = parser.parse_args(argv)
    try:
        report = collect(args.archive, args.audit, args.build_evidence, args.corpus_index, args.corpus_root, args.repo)
        publish(report, args.output)
        return 1 if args.strict and report['unresolved'] else 0
    except (OSError, ValueError, KeyError, TypeError, RecursionError) as error:
        print('Release license report failed: ' + str(error), file=sys.stderr)
        return 2


if __name__ == '__main__':
    sys.exit(main())
