#!/usr/bin/env python3
"""Consent-gated, isolated local Kokoro installer. No assets are bundled."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import urllib.request

MAX_MANIFEST = 1024 * 1024
MAX_LOG = 65536


def emit(phase, status, message, **fields):
    print(json.dumps(dict(phase=phase, status=status, message=message, **fields)), flush=True)


def absolute(value):
    path = Path(value)
    if not path.is_absolute() or '..' in path.parts:
        raise ValueError('Paths must be absolute without parent traversal.')
    return path


def validate_manifest(data):
    if data.get('schema_version') != 1:
        raise ValueError('Unsupported setup manifest schema.')
    if data.get('model_repository') != 'hexgrad/Kokoro-82M':
        raise ValueError('Unsupported model repository.')
    if not re.fullmatch(r'[0-9a-f]{40}', data.get('model_revision', '')):
        raise ValueError('Model revision must be an immutable commit SHA.')
    packages = data.get('packages')
    if not isinstance(packages, list) or not packages or any(not isinstance(p, str) or not re.fullmatch(r'[A-Za-z0-9_.-]+(?:\[[A-Za-z0-9_,.-]+\])?==[A-Za-z0-9_.+!-]+', p) for p in packages):
        raise ValueError('Every package must use an exact name==version pin.')
    seen = set()
    files = data.get('files')
    if not isinstance(files, list) or len(files) > 100:
        raise ValueError('Invalid model files list.')
    for item in files:
        path = PurePosixPath(item.get('path', ''))
        name = str(path)
        if path.is_absolute() or '..' in path.parts or not path.parts or name in seen or '\\' in name or name.startswith('.'):
            raise ValueError('Unsafe or duplicate asset path.')
        if not (name in ('config.json', 'kokoro-v1_0.pth') or (len(path.parts) == 2 and path.parts[0] == 'voices' and name.endswith('.pt'))):
            raise ValueError('Unsupported model asset path.')
        seen.add(name)
        validate_download(item['url'], item['sha256'], item['size_bytes'])
        expected = 'https://huggingface.co/hexgrad/Kokoro-82M/resolve/' + data['model_revision'] + '/' + name
        if item['url'] != expected:
            raise ValueError('Model URL must identify the pinned repository asset.')
    if not {'config.json', 'kokoro-v1_0.pth', 'voices/af_heart.pt', 'voices/am_michael.pt'}.issubset(seen):
        raise ValueError('Manifest lacks required model or English voice assets.')
    validate_download(data['english_model_url'], data['english_model_sha256'], data['english_model_size_bytes'])
    if not data['english_model_url'].split('?')[0].endswith('.whl'):
        raise ValueError('English language asset must be a wheel.')
    filename = data.get('english_model_filename')
    if filename is not None and not re.fullmatch(r'en_core_web_sm-[0-9]+\.[0-9]+\.[0-9]+-py3-none-any\.whl', filename):
        raise ValueError('Invalid normalized English wheel filename.')
    return data


def validate_download(url, digest, size):
    if not isinstance(url, str) or not url.startswith('https://') or not re.fullmatch(r'[0-9a-f]{64}', digest or '') or type(size) is not int or not 0 < size <= 2_000_000_000:
        raise ValueError('Every download needs HTTPS, SHA256 and a bounded positive size.')


def cancelled(cancel_file):
    if cancel_file and cancel_file.exists():
        raise RuntimeError('Installation cancelled.')


def download(url, path, digest, size, cancel_file):
    actual = hashlib.sha256()
    total = 0
    deadline = time.monotonic() + 900
    request = urllib.request.Request(url, headers={'User-Agent': 'Kilsat-Studio-local-setup/1'})
    with urllib.request.urlopen(request, timeout=30) as response, path.open('xb') as output:
        if not response.geturl().startswith('https://'):
            raise ValueError('Download redirected to an insecure URL.')
        while True:
            cancelled(cancel_file)
            if time.monotonic() >= deadline:
                raise RuntimeError('Asset download exceeded its time limit.')
            chunk = response.read(1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            if total > size:
                raise ValueError('Downloaded asset exceeds its declared size.')
            actual.update(chunk)
            output.write(chunk)
    if total != size or actual.hexdigest() != digest:
        raise ValueError('Downloaded asset failed size or SHA256 verification.')


def run(command, cancel_file, timeout=1800):
    # A file avoids unbounded pipe buffering and pipe deadlocks.
    with tempfile.TemporaryFile() as log:
        process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT, start_new_session=True,
                                   env=dict(os.environ, PIP_DISABLE_PIP_VERSION_CHECK='1', PIP_NO_INPUT='1', PYTHONNOUSERSITE='1'))
        deadline = time.monotonic() + timeout
        try:
            while process.poll() is None:
                cancelled(cancel_file)
                if time.monotonic() >= deadline:
                    raise RuntimeError('Setup command timed out.')
                if log.tell() > 8 * 1024 * 1024:
                    raise RuntimeError('Setup command exceeded the diagnostic log limit.')
                time.sleep(0.1)
        except BaseException:
            try:
                os.killpg(process.pid, signal.SIGTERM)
                process.wait(timeout=3)
            except (ProcessLookupError, subprocess.TimeoutExpired):
                pass
            finally:
                # A reaped leader does not mean its descendants exited.
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait()
            raise
        # The command may have exited while a descendant remains in its session.
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        log.seek(0)
        result = log.read(MAX_LOG + 1)
        if len(result) > MAX_LOG:
            raise RuntimeError('Setup command output exceeded the bounded result limit.')
        text = result.decode('utf-8', errors='replace')
        if process.returncode:
            raise RuntimeError('Setup command failed: ' + text[-4000:])
        return text


def install(root, manifest, cancel_file):
    if platform.system() != 'Darwin' or not (3, 10) <= sys.version_info[:2] <= (3, 12):
        raise RuntimeError('Installation requires macOS and Python 3.10–3.12; select a compatible interpreter.')
    cancelled(cancel_file)
    root.parent.mkdir(parents=True, exist_ok=True)
    root.mkdir(parents=False, exist_ok=False)
    try:
        emit('environment', 'running', 'Creating an isolated Python environment.')
        run([sys.executable, '-m', 'venv', '--copies', str(root / 'venv')], cancel_file, 120)
        python = root / 'venv/bin/python'
        assets = root / 'assets'
        assets.mkdir()
        for item in manifest['files']:
            cancelled(cancel_file)
            destination = assets / item['path']
            destination.parent.mkdir(parents=True, exist_ok=True)
            emit('assets', 'running', 'Downloading and verifying ' + item['path'])
            download(item['url'], destination, item['sha256'], item['size_bytes'], cancel_file)
        wheel_name = manifest.get('english_model_filename', manifest['english_model_url'].split('?')[0].rsplit('/', 1)[-1])
        if not re.fullmatch(r'[A-Za-z0-9_.+-]+\.whl', wheel_name):
            raise ValueError('Unsafe English wheel name.')
        # The reviewed spaCy release is hosted under HF's generic 'any' name.
        if wheel_name == 'en_core_web_sm-any-py3-none-any.whl':
            wheel_name = 'en_core_web_sm-3.7.1-py3-none-any.whl'
        wheel = root / wheel_name
        emit('language', 'running', 'Downloading and verifying the English language wheel.')
        download(manifest['english_model_url'], wheel, manifest['english_model_sha256'], manifest['english_model_size_bytes'], cancel_file)
        emit('packages', 'running', 'Installing pinned packages and the verified English wheel.')
        run([str(python), '-m', 'pip', '--isolated', 'install', '--only-binary=:all:', '--index-url', 'https://pypi.org/simple', *manifest['packages'], str(wheel)], cancel_file)
        freeze = run([str(python), '-m', 'pip', '--isolated', 'freeze', '--all'], cancel_file, 60)
        voices = assets / 'voices'
        emit('doctor', 'running', 'Checking local prerequisites; this does not run speech inference.')
        command = [str(python), str(Path(__file__).with_name('doctor.py')), '--config', str(assets / 'config.json'), '--model', str(assets / 'kokoro-v1_0.pth')]
        for voice in sorted(voices.glob('*.pt')):
            command += ['--voice', str(voice)]
        report = json.loads(run(command, cancel_file, 120))
        if report.get('ok') is not True:
            raise RuntimeError('Local prerequisite check failed.')
        cache = root / 'cache'
        cache.mkdir()
        profile = dict(schema_version=1, python=str(python), config=str(assets / 'config.json'), model=str(assets / 'kokoro-v1_0.pth'), voices=str(voices), cache=str(cache), revision=manifest['model_revision'] + ':' + hashlib.sha256(freeze.encode()).hexdigest())
        cancelled(cancel_file)
        pending = root / 'profile.json.tmp'
        pending.write_text(json.dumps(profile, indent=2) + '\n')
        pending.replace(root / 'profile.json')
        emit('complete', 'ready', 'Local prerequisites installed. Run a speech test to verify inference.', profile=str(root / 'profile.json'), settings=profile)
        return profile
    except BaseException:
        shutil.rmtree(root)
        raise


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', required=True, type=absolute)
    parser.add_argument('--manifest', required=True, type=absolute)
    parser.add_argument('--cancel-file', type=absolute)
    parser.add_argument('--allow-downloads', action='store_true')
    parser.add_argument('--plan', action='store_true')
    args = parser.parse_args(argv)
    try:
        # Plan deliberately does not even read the manifest or inspect the filesystem.
        if args.plan:
            emit('plan', 'ready', 'Create an isolated environment, download verified assets and install pinned packages after consent.', root=str(args.root), manifest=str(args.manifest))
            return 0
        if not args.allow_downloads:
            raise ValueError('Explicit --allow-downloads consent is required before installation.')
        with args.manifest.open('rb') as stream:
            raw = stream.read(MAX_MANIFEST + 1)
        if len(raw) > MAX_MANIFEST:
            raise ValueError('Manifest exceeds 1 MiB.')
        manifest = validate_manifest(json.loads(raw))
        def interrupt(signum, frame):
            raise KeyboardInterrupt('Installation interrupted.')
        signal.signal(signal.SIGTERM, interrupt)
        install(args.root, manifest, args.cancel_file)
        return 0
    except (Exception, KeyboardInterrupt) as error:
        emit('failed', 'error', str(error) or 'Installation interrupted.')
        return 1


if __name__ == '__main__':
    sys.exit(main())
