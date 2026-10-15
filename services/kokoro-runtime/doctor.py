#!/usr/bin/env python3
"""Inspect local Kokoro prerequisites without importing models or downloading assets."""
import argparse
import hashlib
from importlib import metadata
import json
from pathlib import Path
import platform
import sys

REQUIRED = ('kokoro', 'torch', 'numpy', 'spacy', 'misaki', 'en-core-web-sm')
EXPECTED_KOKORO = '0.9.4'


def inspect(config, model, voices):
    errors = []
    dependencies = {}
    for package in REQUIRED:
        try:
            version = metadata.version(package)
        except metadata.PackageNotFoundError:
            version = None
            errors.append(f'Missing {package}; explicitly install a compatible local dependency environment.')
        except Exception as exc:
            version = None
            errors.append(f'Cannot read {package} distribution metadata: {type(exc).__name__}; repair the local environment.')
        dependencies[package] = version
    if dependencies['kokoro'] is not None and dependencies['kokoro'] != EXPECTED_KOKORO:
        errors.append(f'Kokoro {EXPECTED_KOKORO} is required by this runtime; install that version explicitly.')
    assets = []
    for kind, path in [('config', config), ('model', model)] + [('voice', voice) for voice in voices]:
        record = {'kind': kind, 'path': str(path), 'size_bytes': None, 'sha256': None}
        try:
            digest = hashlib.sha256()
            size = 0
            if not Path(path).is_file():
                raise OSError('not a regular file')
            with Path(path).open('rb') as source:
                while True:
                    chunk = source.read(1024 * 1024)
                    if not chunk:
                        break
                    size += len(chunk)
                    digest.update(chunk)
            record.update(size_bytes=size, sha256=digest.hexdigest())
            if size == 0:
                errors.append(f'Empty {kind} asset: {path}; provide a complete local asset.')
        except (OSError, ValueError) as exc:
            errors.append(f'Cannot read {kind} asset {path}: {type(exc).__name__}; provide a readable local regular file.')
        assets.append(record)
    return {'schema_version': 1, 'ok': not errors,
            'platform': platform.system(), 'architecture': platform.machine(),
            'python': platform.python_version(), 'dependencies': dependencies,
            'expected_kokoro': EXPECTED_KOKORO,
            'english_g2p_distribution_present': dependencies['en-core-web-sm'] is not None,
            'assets': assets, 'errors': errors,
            'verification': 'Prerequisite metadata only; inference, G2P operation and licenses are not validated.'}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config', type=Path, required=True)
    parser.add_argument('--model', type=Path, required=True)
    parser.add_argument('--voice', type=Path, action='append', required=True)
    args = parser.parse_args(argv)
    report = inspect(args.config, args.model, args.voice)
    print(json.dumps(report, ensure_ascii=True, allow_nan=False))
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())
