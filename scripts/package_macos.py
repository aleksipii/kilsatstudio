#!/usr/bin/env python3
"""Create an ad-hoc signed macOS preview; no models or Python environment bundled."""
import argparse
import os
from pathlib import Path
import plistlib
import shutil
import struct
import zlib
import subprocess
import sys
import tempfile

APP_NAME = 'Kilsat Studio Preview.app'



def app_icon():
    """Original geometric K/film icon, encoded without external image tools."""
    size = 256
    pixels = bytearray()
    for y in range(size):
        pixels.append(0)  # PNG row filter
        for x in range(size):
            border = x < 16 or x >= 240 or y < 16 or y >= 240
            stem = 62 <= x < 82 and 56 <= y < 200
            diagonal = 80 <= x < 188 and abs(abs(y - 128) - (x - 80) * .65) < 12
            sprocket = (28 <= x < 43 or 213 <= x < 228) and y % 48 < 20
            color = (26, 31, 48, 255) if border else (38, 48, 69, 255)
            if stem or diagonal:
                color = (91, 224, 193, 255)
            if sprocket:
                color = (149, 161, 181, 255)
            pixels.extend(color)
    def chunk(kind, payload):
        return (struct.pack('>I', len(payload)) + kind + payload +
                struct.pack('>I', zlib.crc32(kind + payload) & 0xffffffff))
    png = (b'\x89PNG\r\n\x1a\n' +
           chunk(b'IHDR', struct.pack('>IIBBBBB', size, size, 8, 6, 0, 0, 0)) +
           chunk(b'IDAT', zlib.compress(bytes(pixels))) + chunk(b'IEND', b''))
    # ic08 is the standard 256-by-256 PNG representation in an ICNS container.
    entry = b'ic08' + struct.pack('>I', len(png) + 8) + png
    return b'icns' + struct.pack('>I', len(entry) + 8) + entry

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
    (resources / 'KilsatStudio.icns').write_bytes(app_icon())
    with (destination / 'Contents/Info.plist').open('wb') as handle:
        plistlib.dump({'CFBundleExecutable': 'studio-desktop',
                      'CFBundleIdentifier': 'studio.kilsat.preview',
                      'CFBundleName': 'Kilsat Studio Preview',
                      'CFBundlePackageType': 'APPL',
                      'CFBundleShortVersionString': '0.1.0',
                      'CFBundleVersion': '1', 'LSMinimumSystemVersion': '13.0',
                      'NSHighResolutionCapable': True,
                      'CFBundleIconFile': 'KilsatStudio.icns',
                      'CFBundleDocumentTypes': [
                          {'CFBundleTypeName': 'Kilsat Studio Project',
                           'CFBundleTypeRole': 'Editor', 'LSHandlerRank': 'Alternate',
                           'LSItemContentTypes': ['studio.kilsat.project']},
                          {'CFBundleTypeName': 'MotionScript YAML',
                           'CFBundleTypeRole': 'Editor', 'LSHandlerRank': 'Alternate',
                           'CFBundleTypeExtensions': ['yaml', 'yml']}],
                      'UTExportedTypeDeclarations': [
                          {'UTTypeIdentifier': 'studio.kilsat.project',
                           'UTTypeDescription': 'Kilsat Studio JSON Project',
                           'UTTypeConformsTo': ['public.json'],
                           'UTTypeTagSpecification':
                               {'public.filename-extension': ['kilsat']}}]}, handle)
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
