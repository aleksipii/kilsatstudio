#!/usr/bin/env python3
"""Build the speech bundle that ships inside Kilsat Studio.app.

The bundle holds a relocatable Python, the pinned Kokoro model and English voices,
the Piper runtime with the Finnish harri voice and the Rhubarb lip-sync tool, so
the app speaks and estimates mouth timing without any download or setup.

Downloads happen here, at build time, only with --allow-downloads. Every file is
checked against services/speech-bundle/manifest.json (HTTPS, size, SHA-256).

Usage:
  scripts/build_speech_bundle.py --output /absolute/new/speech --allow-downloads \
      [--download-cache /absolute/cache]
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import runpy
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile

REPO = Path(__file__).resolve().parents[1]
MANIFEST = REPO / 'services/speech-bundle/manifest.json'
DESCRIPTOR = 'speech-bundle.json'
PYTHON_BIN = 'python/bin/python3.12'
PIPER_WRAPPER = '''#!/bin/sh
# Runs the Piper speech engine with the Python bundled next to this script.
here=$(cd "$(dirname "$0")" && pwd)
exec "$here/../python/bin/python3.12" -I -B "$here/piper_launch.py" "$@"
'''
# eSpeak NG silently ignores data paths of 160 characters or more and falls back to
# its build machine's path, so the data lives near the bundle root instead of deep
# inside site-packages. Both engines are pointed at these folders explicitly.
ESPEAK_DATA = {'espeak/kokoro': 'python/lib/python3.12/site-packages/espeakng_loader/espeak-ng-data',
               'espeak/piper': 'python/lib/python3.12/site-packages/piper/espeak-ng-data'}
PIPER_LAUNCH = '''"""Run Piper with the bundle's short eSpeak NG data path."""
import os
import sys

DATA = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'espeak', 'piper')
from piper import phonemize_espeak  # noqa: E402

_initialize = phonemize_espeak.EspeakPhonemizer.__init__


def _bundled_data(self, espeak_data_dir=None):
    _initialize(self, DATA)


phonemize_espeak.EspeakPhonemizer.__init__ = _bundled_data
from piper.__main__ import main  # noqa: E402

sys.exit(main())
'''

setup = runpy.run_path(str(REPO / 'services/kokoro-runtime/setup_kokoro.py'))


def load_json(path):
    with Path(path).open('rb') as stream:
        raw = stream.read(1024 * 1024 + 1)
    if len(raw) > 1024 * 1024:
        raise ValueError(f'{path} exceeds 1 MiB')
    return json.loads(raw)


def safe_relative(name):
    path = PurePosixPath(name)
    if (not name or path.is_absolute() or '..' in path.parts or '\\' in name
            or any(part.startswith('.') for part in path.parts)):
        raise ValueError(f'unsafe bundle path: {name!r}')
    return path


def safe_member(name):
    """Archive entries may be dotfiles but never absolute or escaping."""
    path = PurePosixPath(name)
    if not name or path.is_absolute() or '..' in path.parts or '\\' in name:
        raise ValueError(f'unsafe archive entry: {name!r}')
    return path


def validate_manifest(data):
    """Check the bundle manifest; returns it unchanged."""
    if data.get('schema_version') != 1 or not re.fullmatch(r'[a-z0-9-]{1,64}', data.get('bundle_id', '')):
        raise ValueError('unsupported speech bundle manifest')
    python = data['python']
    setup['validate_download'](python['url'], python['sha256'], python['size_bytes'])
    if not re.fullmatch(r'3\.1[0-2]\.[0-9]+', python.get('version', '')):
        raise ValueError('bundled Python must be 3.10-3.12 for Kokoro')
    if not python['url'].endswith('aarch64-apple-darwin-install_only_stripped.tar.gz') and \
            not python['url'].endswith('aarch64-apple-darwin-install_only.tar.gz'):
        raise ValueError('bundled Python must be a relocatable arm64 macOS build')
    packages = data.get('packages')
    if not isinstance(packages, list) or any(
            not re.fullmatch(r'[A-Za-z0-9_.-]+(?:\[[A-Za-z0-9_,.-]+\])?==[A-Za-z0-9_.+!-]+', p) for p in packages):
        raise ValueError('every bundled package needs an exact name==version pin')
    piper = data['piper']
    if not re.fullmatch(r'piper_[a-z0-9_]{1,64}', piper.get('preset', '')) or \
            not re.fullmatch(r'[0-9a-f]{40}', piper.get('revision', '')):
        raise ValueError('Piper needs a piper_ preset and an immutable revision')
    names = set()
    for item in piper['files']:
        safe_relative(item['path'])
        setup['validate_download'](item['url'], item['sha256'], item['size_bytes'])
        if f"/resolve/{piper['revision']}/" not in item['url']:
            raise ValueError('Piper files must come from the pinned revision')
        names.add(item['path'])
    if not any(n.endswith('.onnx') for n in names) or not any(n.endswith('.onnx.json') for n in names):
        raise ValueError('Piper needs a model and its configuration')
    rhubarb = data['rhubarb']
    setup['validate_download'](rhubarb['url'], rhubarb['sha256'], rhubarb['size_bytes'])
    safe_relative(rhubarb['archive_root'])
    if 'rhubarb' not in rhubarb.get('keep', []):
        raise ValueError('Rhubarb bundle must keep the executable')
    for name in rhubarb['keep']:
        safe_relative(name)
    for key in ('kokoro_manifest', 'kokoro_voice_catalog'):
        safe_relative(data[key])
    return data


def kokoro_files(kokoro, catalog):
    """Model files plus every pinned English voice, deduplicated by path."""
    if catalog['model_revision'] != kokoro['model_revision']:
        raise ValueError('voice catalog and Kokoro manifest name different revisions')
    files = {item['path']: item for item in kokoro['files']}
    for voice in catalog['voices']:
        existing = files.get(voice['path'])
        if existing and (existing['sha256'], existing['size_bytes']) != (voice['sha256'], voice['size_bytes']):
            raise ValueError(f"conflicting pins for {voice['path']}")
        files.setdefault(voice['path'], voice)
    for item in files.values():
        safe_relative(item['path'])
        setup['validate_download'](item['url'], item['sha256'], item['size_bytes'])
    return [files[k] for k in sorted(files)]


def fetch(item, cache, name):
    """Return a verified local copy, downloading into the cache if needed."""
    target = cache / f"{item['sha256']}-{name}"
    if target.is_file() and not target.is_symlink():
        if target.stat().st_size == item['size_bytes'] and sha256_file(target) == item['sha256']:
            return target
        target.unlink()
    print(f'Downloading {name}', flush=True)
    partial = target.with_name(target.name + '.partial')
    if partial.exists():
        partial.unlink()
    setup['download'](item['url'], partial, item['sha256'], item['size_bytes'], None)
    partial.replace(target)
    return target


def sha256_file(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def extract_python(archive, root):
    with tarfile.open(archive, 'r:gz') as tar:
        for member in tar.getmembers():
            safe_member(member.name.rstrip('/'))
            if not member.name.startswith('python/'):
                raise ValueError('unexpected entry in Python archive: ' + member.name)
        tar.extractall(root, filter='data')
    if not (root / PYTHON_BIN).is_file():
        raise ValueError('Python archive lacks ' + PYTHON_BIN)


def extract_rhubarb(archive, destination, spec):
    prefix = spec['archive_root'] + '/'
    keep = tuple(spec['keep'])
    with zipfile.ZipFile(archive) as bundle:
        for info in bundle.infolist():
            if not info.filename.startswith(prefix):
                continue
            relative = info.filename[len(prefix):]
            if not relative or not (relative in keep or relative.split('/', 1)[0] in keep):
                continue
            path = destination / safe_member(relative.rstrip('/'))
            if info.is_dir():
                path.mkdir(parents=True, exist_ok=True)
                continue
            if (info.external_attr >> 16) & 0o170000 == 0o120000:
                raise ValueError('symlink in Rhubarb archive: ' + info.filename)
            path.parent.mkdir(parents=True, exist_ok=True)
            with bundle.open(info) as source, path.open('xb') as target:
                shutil.copyfileobj(source, target)
            mode = (info.external_attr >> 16) & 0o777
            path.chmod(0o755 if relative == 'rhubarb' or mode & 0o111 else 0o644)
    if not (destination / 'rhubarb').is_file():
        raise ValueError('Rhubarb archive lacks the executable')


def write_piper_wrapper(root):
    path = root / 'bin/piper'
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(PIPER_WRAPPER, encoding='utf-8')
    path.chmod(0o755)
    (root / 'bin/piper_launch.py').write_text(PIPER_LAUNCH, encoding='utf-8')
    return path


def relocate_espeak_data(root):
    """Move eSpeak NG data to short paths; the empty original keeps exists() checks working."""
    for short, original in ESPEAK_DATA.items():
        source, target = root / original, root / short
        if not (source / 'phontab').is_file():
            raise ValueError('eSpeak NG data not found at ' + original)
        target.parent.mkdir(parents=True, exist_ok=True)
        source.rename(target)
        source.mkdir()


def descriptor(manifest, kokoro, freeze_sha256, files):
    """The relative-path description the desktop reads at startup."""
    piper = manifest['piper']
    model = next(i['path'] for i in piper['files'] if i['path'].endswith('.onnx'))
    config = next(i['path'] for i in piper['files'] if i['path'].endswith('.onnx.json'))
    packages = ' '.join(manifest['packages'])
    environment = (f"{manifest['bundle_id']}; python {manifest['python']['version']}; {packages}; "
                   f'freeze sha256 {freeze_sha256}')
    return dict(
        schema_version=1,
        bundle_id=manifest['bundle_id'],
        kokoro=dict(python=PYTHON_BIN, config='kokoro/config.json', model='kokoro/kokoro-v1_0.pth',
                    voices='kokoro/voices', revision=f"{kokoro['model_revision']}:{freeze_sha256}"),
        piper=dict(executable='bin/piper', model=f'piper/{model}', config=f'piper/{config}',
                   preset=piper['preset'],
                   model_version=f"{piper['repository']}@{piper['revision']}/{model}",
                   environment_identity=environment),
        rhubarb=dict(executable='rhubarb/rhubarb', version=manifest['rhubarb']['version'],
                     architecture='x86_64'),
        files=files,
    )


MACHO_MAGIC = {b'\xcf\xfa\xed\xfe', b'\xce\xfa\xed\xfe', b'\xca\xfe\xba\xbe', b'\xbe\xba\xfe\xca'}


def is_macho(path):
    with Path(path).open('rb') as stream:
        return stream.read(4) in MACHO_MAGIC


def remove_symlinks(root):
    """The app package admits regular files only; Kokoro and Piper use python3.12 directly."""
    removed = []
    for path in sorted(Path(root).rglob('*'), reverse=True):
        if path.is_symlink():
            removed.append(path.relative_to(root).as_posix())
            path.unlink()
    return sorted(removed)


def sign_native_code(root):
    """Ad-hoc sign every Mach-O so the app signature seals these exact bytes."""
    signed = 0
    for path in sorted(Path(root).rglob('*')):
        if path.is_file() and not path.is_symlink() and is_macho(path):
            subprocess.run(['codesign', '--force', '--sign', '-', str(path)], check=True,
                           capture_output=True, timeout=120)
            signed += 1
    return signed


def run(command, timeout=3600, check=True):
    env = {k: v for k, v in os.environ.items() if not k.startswith('PYTHON')}
    env.update(PIP_DISABLE_PIP_VERSION_CHECK='1', PIP_NO_INPUT='1', PYTHONNOUSERSITE='1')
    result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=timeout)
    if check and result.returncode:
        raise RuntimeError(f'{command[0]} failed:\n{(result.stdout + result.stderr)[-4000:]}')
    return result.stdout


def build(output, cache):
    manifest = validate_manifest(load_json(MANIFEST))
    kokoro = setup['validate_manifest'](load_json(REPO / manifest['kokoro_manifest']))
    catalog = load_json(REPO / manifest['kokoro_voice_catalog'])
    model_files = kokoro_files(kokoro, catalog)
    if sys.platform != 'darwin' or platform.machine() != 'arm64':
        raise RuntimeError('the speech bundle is built on Apple Silicon macOS')
    if output.exists():
        raise ValueError('output must not exist; existing bundles are never overwritten')
    output.parent.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix='.speech-', dir=output.parent))
    try:
        extract_python(fetch(manifest['python'], cache, 'python.tar.gz'), work)
        python = work / PYTHON_BIN
        wheel = fetch(dict(url=kokoro['english_model_url'], sha256=kokoro['english_model_sha256'],
                           size_bytes=kokoro['english_model_size_bytes']), cache,
                      kokoro.get('english_model_filename', 'en_core_web_sm.whl'))
        named_wheel = work / kokoro.get('english_model_filename', 'en_core_web_sm-3.7.1-py3-none-any.whl')
        shutil.copy2(wheel, named_wheel)
        print('Installing pinned packages into the bundled Python', flush=True)
        run([str(python), '-I', '-m', 'pip', 'install', '--only-binary=:all:', '--no-warn-script-location',
             '--index-url', 'https://pypi.org/simple', *kokoro['packages'], *manifest['packages'],
             str(named_wheel)])
        named_wheel.unlink()
        # Precompiled, hash-checked bytecode: the signed app never needs to write .pyc files.
        # Some wheels ship unused Python 2 example files that cannot compile; skip them.
        run([str(python), '-I', '-m', 'compileall', '-q', '-j', '0',
             '--invalidation-mode', 'unchecked-hash', str(work / 'python/lib')], check=False)
        freeze = setup['sanitized_freeze'](run([str(python), '-I', '-m', 'pip', 'freeze', '--all']))
        evidence = work / 'evidence'
        evidence.mkdir()
        (evidence / 'freeze.txt').write_text(freeze, encoding='utf-8')
        code = ('import runpy; m=runpy.run_path(' + repr(str(REPO / 'services/kokoro-runtime/setup_kokoro.py'))
                + '); m["collect_installed_metadata"](' + repr(str(evidence / 'distributions.json')) + ')')
        run([str(python), '-I', '-c', code])
        shutil.copy2(MANIFEST, evidence / 'speech-bundle-manifest.json')

        recorded = []
        for item in model_files:
            target = work / 'kokoro' / item['path']
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(fetch(item, cache, PurePosixPath(item['path']).name), target)
            recorded.append(dict(path=f"kokoro/{item['path']}", sha256=item['sha256'], size_bytes=item['size_bytes']))
        for item in manifest['piper']['files']:
            target = work / 'piper' / item['path']
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(fetch(item, cache, item['path']), target)
            recorded.append(dict(path=f"piper/{item['path']}", sha256=item['sha256'], size_bytes=item['size_bytes']))
        write_piper_wrapper(work)
        relocate_espeak_data(work)
        extract_rhubarb(fetch(manifest['rhubarb'], cache, 'rhubarb.zip'), work / 'rhubarb', manifest['rhubarb'])

        print('Checking Kokoro prerequisites with the bundled Python', flush=True)
        doctor = [str(python), '-I', '-B', str(REPO / 'services/kokoro-runtime/doctor.py'),
                  '--config', str(work / 'kokoro/config.json'), '--model', str(work / 'kokoro/kokoro-v1_0.pth')]
        for voice in ('af_heart.pt', 'am_michael.pt'):
            doctor += ['--voice', str(work / 'kokoro/voices' / voice)]
        if json.loads(run(doctor, 300)).get('ok') is not True:
            raise RuntimeError('Kokoro prerequisite check failed for the bundled Python')

        removed = remove_symlinks(work)
        (evidence / 'removed-symlinks.txt').write_text(''.join(f'{n}\n' for n in removed), encoding='utf-8')
        print(f'Signed {sign_native_code(work)} native libraries and executables', flush=True)
        freeze_sha = hashlib.sha256(freeze.encode()).hexdigest()
        text = json.dumps(descriptor(manifest, kokoro, freeze_sha, recorded), indent=2, sort_keys=True) + '\n'
        (work / DESCRIPTOR).write_text(text, encoding='utf-8')
        work.rename(output)
    except BaseException:
        shutil.rmtree(work, ignore_errors=True)
        raise
    return output


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--download-cache', type=Path)
    parser.add_argument('--allow-downloads', action='store_true')
    args = parser.parse_args(argv)
    if not args.allow_downloads:
        parser.error('building the speech bundle downloads pinned assets; pass --allow-downloads')
    output = args.output.absolute()
    cache = (args.download_cache or output.parent / '.speech-download-cache').absolute()
    cache.mkdir(parents=True, exist_ok=True)
    print(build(output, cache))


if __name__ == '__main__':
    main()
