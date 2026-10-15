#!/usr/bin/env python3
"""Create an ad-hoc signed macOS preview; no models or Python environment bundled."""
import argparse
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile

APP_NAME = 'Kilsat Studio Preview.app'


def stage_bundle(repo, binary, destination, commit, arch, profile):
    repo, binary, destination = Path(repo).resolve(), Path(binary).resolve(), Path(destination)
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError('binary must be an existing executable file')
    if destination.exists():
        raise ValueError('bundle destination already exists')
    required = ['examples', 'services/kokoro-runtime/runtime.py',
                'services/kokoro-runtime/doctor.py', 'services/kokoro-runtime/smoke_macos.py',
                'services/kokoro-runtime/requirements.txt', 'services/kokoro-runtime/README.md',
                'docs/practical-testing.md']
    for relative in required:
        source = repo / relative
        if not source.exists() or source.is_symlink():
            raise ValueError(f'missing or unsafe resource: {relative}')
    resources = destination / 'Contents/Resources'
    executable = destination / 'Contents/MacOS/studio-desktop'
    executable.parent.mkdir(parents=True)
    resources.mkdir(parents=True)
    shutil.copy2(binary, executable)
    for relative in required:
        source, target = repo / relative, resources / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        if source.is_dir():
            # Only small screenplay files: never accidentally bundle downloaded models.
            target.mkdir()
            for screenplay in source.glob('*.yaml'):
                if screenplay.is_symlink():
                    raise ValueError('symlinked screenplay is not allowed')
                shutil.copy2(screenplay, target / screenplay.name)
        else:
            shutil.copy2(source, target)
    with (destination / 'Contents/Info.plist').open('wb') as handle:
        plistlib.dump({'CFBundleExecutable': 'studio-desktop',
                      'CFBundleIdentifier': 'studio.kilsat.preview',
                      'CFBundleName': 'Kilsat Studio Preview',
                      'CFBundlePackageType': 'APPL',
                      'CFBundleShortVersionString': '0.1.0',
                      'CFBundleVersion': '1', 'LSMinimumSystemVersion': '13.0',
                      'NSHighResolutionCapable': True}, handle)
    (resources / 'BUILD-INFO.txt').write_text(
        f'Commit: {commit}\nArchitecture: {arch}\nProfile: {profile}\n'
        'Preview build; ad-hoc signed, not notarized.\n'
        'No model weights, voice assets, Python runtime or Python dependencies included.\n',
        encoding='utf-8')
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--arch', choices=['arm64', 'x86_64'], required=True)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    if sys.platform != 'darwin':
        parser.error('signing and archive creation require macOS')
    output = args.output.absolute()
    if output.suffix != '.zip' or output.exists():
        parser.error('output must be a new .zip file; existing output is never overwritten')
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='kilsat-package-') as temporary:
        bundle = stage_bundle(args.repo, args.binary, Path(temporary) / APP_NAME,
                              args.commit, args.arch, args.profile)
        subprocess.run(['codesign', '--force', '--sign', '-', str(bundle)], check=True)
        subprocess.run(['codesign', '--verify', '--deep', '--strict', str(bundle)], check=True)
        # ditto preserves executable permissions and the enclosing app directory.
        subprocess.run(['ditto', '-c', '-k', '--sequesterRsrc', '--keepParent',
                        str(bundle), str(output)], check=True)
    print(output)


if __name__ == '__main__':
    main()
