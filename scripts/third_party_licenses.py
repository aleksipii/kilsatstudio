#!/usr/bin/env python3
"""Collect licence texts of the Rust crates compiled into the shipped apps.

Walks the normal and build dependencies of the desktop app (`studio-desktop`)
and the iPad core (`motion-ffi`) for every Apple target, then copies each
third-party crate's LICENSE/COPYING/NOTICE files (and bundled font licences)
from the local Cargo sources into THIRD-PARTY-LICENSES.txt. Identical texts are
written once with every crate that uses them. A Markdown summary goes to
docs/third-party-license-report.md.

Runs `cargo metadata --locked --offline`; it never downloads anything (run
`cargo fetch --locked` first on a fresh machine). Collecting texts is not a
legal review: the summary lists what still needs a human decision.

  python3 scripts/third_party_licenses.py          # regenerate both files
  python3 scripts/third_party_licenses.py --check  # fail if they are stale
"""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

REPO = Path(__file__).resolve().parents[1]
TEXT_OUT = REPO / 'THIRD-PARTY-LICENSES.txt'
REPORT_OUT = REPO / 'docs' / 'third-party-license-report.md'
SUPPLIED = REPO / 'licenses' / 'supplied'
ROOTS = ('studio-desktop', 'motion-ffi')
TARGETS = ('aarch64-apple-darwin', 'x86_64-apple-darwin', 'aarch64-apple-ios', 'aarch64-apple-ios-sim')
LICENSE_FILE = re.compile(r'^(LICEN[CS]E|COPYING|NOTICE|COPYRIGHT|UNLICENSE)([-._].*)?$', re.I)
# Font licences shipped inside crates whose fonts end up in the binary.
EXTRA_FILES = {'epaint_default_fonts': ('fonts/OFL.txt', 'fonts/UFL.txt', 'fonts/Hack-Regular.txt',
                                        'fonts/emoji-icon-font-mit-license.txt')}
PERMISSIVE = {'MIT', 'Apache-2.0', 'BSD-2-Clause', 'BSD-3-Clause', 'Zlib', 'ISC', 'Unicode-3.0',
              'Unicode-DFS-2016', 'CC0-1.0', 'Unlicense', '0BSD', 'BSL-1.0', 'OFL-1.1',
              'Ubuntu-font-1.0', 'Apache-2.0 WITH LLVM-exception'}
MAX_TEXT = 256 * 1024


def metadata(target):
    output = subprocess.run(
        ['cargo', 'metadata', '--locked', '--offline', '--format-version', '1',
         '--filter-platform', target],
        cwd=REPO, check=True, capture_output=True, text=True).stdout
    return json.loads(output)


def shipped(meta):
    """Third-party packages reachable from ROOTS through normal/build edges."""
    packages = {p['id']: p for p in meta['packages']}
    nodes = {n['id']: n for n in meta['resolve']['nodes']}
    workspace = set(meta['workspace_members'])
    by_root = {}
    for root in ROOTS:
        start = [i for i in workspace if packages[i]['name'] == root]
        if len(start) != 1:
            raise SystemExit(f'workspace package {root!r} not found')
        seen, stack = set(), start
        while stack:
            current = stack.pop()
            if current in seen:
                continue
            seen.add(current)
            for dep in nodes[current]['deps']:
                if any(kind['kind'] in (None, 'build') for kind in dep['dep_kinds']):
                    stack.append(dep['pkg'])
        by_root[root] = {i for i in seen if i not in workspace}
    return packages, by_root


def normalise_expression(expression):
    # Old crates write "MIT/Apache-2.0"; treat "/" as OR.
    return re.sub(r'\s*/\s*', ' OR ', expression or '').strip()


def needs_review(expression):
    """True unless some OR-alternative consists only of permissive licences."""
    if not expression:
        return True
    flat = expression.replace('(', ' ').replace(')', ' ')
    for alternative in re.split(r'\s+OR\s+', flat):
        parts = [part.strip() for part in re.split(r'\s+AND\s+', alternative)]
        if parts and all(part in PERMISSIVE for part in parts):
            return False
    return True


def licence_texts(package):
    directory = Path(package['manifest_path']).parent
    names = sorted(path.name for path in directory.iterdir()
                   if path.is_file() and LICENSE_FILE.match(path.name))
    names += [name for name in EXTRA_FILES.get(package['name'], ()) if (directory / name).is_file()]
    if package.get('license_file'):
        names.append(package['license_file'])
    texts = []
    for name in dict.fromkeys(names):
        path = directory / name
        if not path.is_file() or path.stat().st_size > MAX_TEXT:
            continue
        text = path.read_text(encoding='utf-8', errors='replace').replace('\r\n', '\n').strip()
        if text:
            texts.append((name, text + '\n'))
    return texts


def supplied_texts():
    """Crate name -> (label, text) for texts copied from upstream by hand."""
    index = json.loads((SUPPLIED / 'index.json').read_text(encoding='utf-8'))
    found = {}
    for name, entry in index.items():
        if name.startswith('_'):
            continue
        text = (SUPPLIED / name).read_text(encoding='utf-8').replace('\r\n', '\n').strip() + '\n'
        for crate in entry['crates']:
            found[crate] = (f"supplied from {entry['source']}", text)
    return found


def canonical_apache(texts, users):
    """The most widely shared verbatim Apache-2.0 text among collected files."""
    candidates = [d for d, text in texts.items()
                  if 'Apache License' in text and 'Version 2.0, January 2004' in text
                  and 'END OF TERMS AND CONDITIONS' in text]
    return max(candidates, key=lambda d: (len(users[d]), d)) if candidates else None


def build():
    packages, membership = {}, collections.defaultdict(set)
    for target in TARGETS:
        found, by_root = shipped(metadata(target))
        packages.update(found)
        for root, ids in by_root.items():
            for package_id in ids:
                membership[package_id].add(root)
    ids = sorted(membership, key=lambda i: (packages[i]['name'], packages[i]['version']))
    texts, users, missing = {}, collections.defaultdict(list), []
    rows, without_file = [], []
    supplied = supplied_texts()
    for package_id in ids:
        package = packages[package_id]
        label = f"{package['name']} {package['version']}"
        found = licence_texts(package)
        if not found:
            without_file.append(package)
            if package['name'] in supplied:
                found = [supplied[package['name']]]
        for name, text in found:
            digest = hashlib.sha256(text.encode()).hexdigest()
            texts.setdefault(digest, text)
            users[digest].append(f'{label} ({name})')
        expression = normalise_expression(package['license'])
        rows.append((label, expression or 'UNDECLARED', sorted(membership[package_id]),
                     needs_review(expression), bool(found)))
    # No text in the package or supplied: elect the Apache-2.0 option when offered
    # (its text has no per-project copyright line); CC0 requires no notice.
    apache = canonical_apache(texts, users)
    for package in without_file:
        label = f"{package['name']} {package['version']}"
        expression = normalise_expression(package['license'])
        options = re.split(r'\s+OR\s+', expression.replace('(', ' ').replace(')', ' '))
        if package['name'] in supplied:
            continue
        if apache and any(option.strip() == 'Apache-2.0' for option in options):
            users[apache].append(f'{label} (Apache-2.0 elected; no licence file in crate package)')
        elif expression != 'CC0-1.0':
            missing.append(f"{label} (`{expression}`, {package.get('repository') or 'no repository'})")
    for digest in users:
        users[digest].sort()
    return rows, texts, users, missing


def render_text(rows, texts, users):
    lines = [
        'Koeta Studio: third-party licences of compiled Rust crates',
        '=' * 58,
        '',
        'Generated by scripts/third_party_licenses.py from Cargo.lock. Each text',
        'below is reproduced from the crate source with the crates it applies to.',
        'Material listed in NOTICE (fonts, models, voices) is covered there.',
        '',
        f'Crates: {len(rows)}. Distinct licence texts: {len(texts)}.',
        '',
    ]
    for digest in sorted(texts, key=lambda d: (users[d][0], d)):
        lines += ['-' * 78, 'Applies to:']
        lines += [f'  {user}' for user in users[digest]]
        lines += ['', texts[digest].rstrip('\n'), '']
    return '\n'.join(lines) + '\n'


def render_report(rows, missing):
    counts = collections.Counter(expression for _, expression, _, _, _ in rows)
    review = [row for row in rows if row[3]]
    desktop = sum('studio-desktop' in row[2] for row in rows)
    ipad = sum('motion-ffi' in row[2] for row in rows)
    lines = [
        '# Third-party licence report (Rust crates)',
        '',
        'Generated by `scripts/third_party_licenses.py`; regenerate after any',
        '`Cargo.lock` change (CI fails when this file is stale). Covers crates',
        'compiled into the desktop app (`studio-desktop`) and the iPad core',
        '(`motion-ffi`) for macOS arm64/x86_64 and iOS device/simulator targets.',
        'Licence texts are collected into `THIRD-PARTY-LICENSES.txt`, which the',
        'macOS package ships next to `NOTICE`.',
        '',
        'This is collected evidence, not legal approval. Python speech runtimes,',
        'models, voices, Rhubarb and FFmpeg are outside this report; see',
        '`docs/dependency-audit.md` and `NOTICE`.',
        '',
        f'- Third-party crates: **{len(rows)}** (desktop {desktop}, iPad core {ipad})',
        f'- Crates without a permissive-only alternative: **{len(review)}**',
        f'- Crates whose licence text is still missing: **{len(missing)}**',
        '',
        '## Licence expressions',
        '',
        '| Crates | Expression |',
        '| ---: | --- |',
    ]
    lines += [f'| {n} | `{expression}` |' for expression, n in
              sorted(counts.items(), key=lambda item: (-item[1], item[0]))]
    lines += ['', '## Needs a human decision', '']
    if review:
        lines += ['| Crate | Expression | Used by |', '| --- | --- | --- |']
        lines += [f'| {label} | `{expression}` | {", ".join(roots)} |'
                  for label, expression, roots, _, _ in review]
    else:
        lines.append('None: every crate offers a permissive-only licence alternative.')
    lines += ['', '## Licence text still missing', '',
              'Crates whose package lacks a licence file are covered by a text copied',
              'from upstream (`licenses/supplied/`), by the Apache-2.0 option when one',
              'is offered, or need no notice (CC0-1.0). These remain:', '']
    if missing:
        lines.append('Declared licence applies, but no text exists in the package or upstream repository:')
        lines.append('')
        lines += [f'- {label}' for label in missing]
    else:
        lines.append('None.')
    return '\n'.join(lines) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--check', action='store_true', help='fail if the outputs are stale')
    args = parser.parse_args()
    rows, texts, users, missing = build()
    outputs = {TEXT_OUT: render_text(rows, texts, users), REPORT_OUT: render_report(rows, missing)}
    stale = [path for path, content in outputs.items()
             if not path.is_file() or path.read_text(encoding='utf-8') != content]
    if args.check:
        for path in stale:
            print(f'{path.relative_to(REPO)} is stale; run python3 scripts/third_party_licenses.py',
                  file=sys.stderr)
        return 1 if stale else 0
    for path, content in outputs.items():
        path.write_text(content, encoding='utf-8')
    print(json.dumps({'crates': len(rows), 'texts': len(texts), 'missing_text': len(missing),
                      'needs_review': sum(row[3] for row in rows)}))
    return 0


if __name__ == '__main__':
    sys.exit(main())
