#!/usr/bin/env python3
"""Create an ad-hoc signed macOS app; optionally with the prebuilt speech bundle.

Without --speech-bundle no models or Python environment are bundled. With it, the
directory made by scripts/build_speech_bundle.py is copied unchanged to
Contents/Resources/speech so Kokoro, Piper and Rhubarb work without any download.
"""
import hashlib
import argparse
import json
import os
from pathlib import Path
import plistlib
import shutil
import struct
import subprocess
import sys
import tempfile

APP_NAME = 'Koeta Studio.app'
VERSION = '1.0.0'


LOGO_PNG = Path(__file__).resolve().parent.parent / 'apps/studio-desktop/assets/koeta-logo-512.png'


def app_icon():
    """Koeta logo (512-by-512 RGBA PNG) wrapped in an ICNS container."""
    png = LOGO_PNG.read_bytes()
    if png[:8] != b'\x89PNG\r\n\x1a\n' or struct.unpack('>II', png[16:24]) != (512, 512):
        raise ValueError(f'{LOGO_PNG} must be a 512-by-512 PNG')
    # ic09 is the standard 512-by-512 PNG representation in an ICNS container.
    entry = b'ic09' + struct.pack('>I', len(png) + 8) + png
    return b'icns' + struct.pack('>I', len(entry) + 8) + entry


def bundle_info():
    """Expected generated application metadata shared with the offline audit."""
    return {'CFBundleExecutable': 'studio-desktop',
                      'CFBundleIdentifier': 'studio.kilsat.studio',
                      'CFBundleName': 'Koeta Studio',
                      'CFBundlePackageType': 'APPL',
                      'CFBundleShortVersionString': VERSION,
                      'CFBundleVersion': VERSION, 'LSMinimumSystemVersion': '13.0',
                      'NSHighResolutionCapable': True,
                      'NSCameraUsageDescription': 'Kameraa käytetään pään kallistuksen paikalliseen seurantaan ja liikkeen tallentamiseen animaatioksi. Kuvia tai ääntä ei tallenneta eikä lähetetä.',
                      'CFBundleIconFile': 'KoetaStudio.icns',
                      'CFBundleDocumentTypes': [
                          {'CFBundleTypeName': 'Koeta Studio Project',
                           'CFBundleTypeRole': 'Editor', 'LSHandlerRank': 'Alternate',
                           'LSItemContentTypes': ['studio.kilsat.project']},
                          {'CFBundleTypeName': 'MotionScript YAML',
                           'CFBundleTypeRole': 'Editor', 'LSHandlerRank': 'Alternate',
                           'CFBundleTypeExtensions': ['yaml', 'yml']}],
                      'UTExportedTypeDeclarations': [
                          {'UTTypeIdentifier': 'studio.kilsat.project',
                           'UTTypeDescription': 'Koeta Studio JSON Project',
                           'UTTypeConformsTo': ['public.json'],
                           'UTTypeTagSpecification':
                               {'public.filename-extension': ['kilsat']}}]}


SPEECH_DESCRIPTOR = 'speech-bundle.json'


def speech_identity(bundle):
    """(bundle_id, descriptor SHA-256) of a built speech bundle directory."""
    descriptor = Path(bundle) / SPEECH_DESCRIPTOR
    if descriptor.is_symlink() or not descriptor.is_file():
        raise ValueError('speech bundle lacks ' + SPEECH_DESCRIPTOR)
    raw = descriptor.read_bytes()
    if len(raw) > 1024 * 1024:
        raise ValueError('speech bundle descriptor exceeds 1 MiB')
    document = json.loads(raw)
    if document.get('schema_version') != 1 or not isinstance(document.get('bundle_id'), str):
        raise ValueError('unsupported speech bundle descriptor')
    return document['bundle_id'], hashlib.sha256(raw).hexdigest()


def build_info(commit, arch, profile, speech=None):
    """BUILD-INFO.txt contents, shared with the offline audit.

    speech is None or the (bundle_id, descriptor sha256) pair from speech_identity.
    """
    contents = ('No model weights, voice assets, Python runtime or Python dependencies included.\n'
                if speech is None else
                f'Bundled speech {speech[0]}: Python runtime, Kokoro model and English voices, '
                f'Piper with the Finnish harri voice and Rhubarb; descriptor sha256 {speech[1]}.\n')
    return (f'Commit: {commit}\nArchitecture: {arch}\nProfile: {profile}\n'
            f'Koeta Studio {VERSION}; ad-hoc signed, not notarized.\n' + contents)


def stage_bundle(repo, binary, destination, commit, arch, profile, gpu_probe=None, head_pose=None,
                 speech_bundle=None):
    repo, binary, destination = Path(repo).resolve(), Path(binary).resolve(), Path(destination)
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError('binary must be an existing executable file')
    if destination.exists():
        raise ValueError('bundle destination already exists')
    if gpu_probe is not None:
        gpu_probe = Path(gpu_probe)
        if gpu_probe.is_symlink() or not gpu_probe.is_file() or not os.access(gpu_probe, os.X_OK):
            raise ValueError('gpu probe must be a non-symlink executable file')
    if head_pose is not None:
        head_pose = Path(head_pose)
        if head_pose.is_symlink() or not head_pose.is_file() or not os.access(head_pose, os.X_OK):
            raise ValueError('head pose helper must be a non-symlink executable file')
    speech = None
    if speech_bundle is not None:
        speech_bundle = Path(speech_bundle)
        if speech_bundle.is_symlink() or not speech_bundle.is_dir():
            raise ValueError('speech bundle must be a non-symlink directory')
        speech = speech_identity(speech_bundle)
        for path in speech_bundle.rglob('*'):
            if path.is_symlink() or not (path.is_file() or path.is_dir()):
                raise ValueError('speech bundle may contain only regular files and directories')
    license_path = repo / 'LICENSE'
    if license_path.is_symlink() or not license_path.is_file() or not license_path.stat().st_size:
        raise ValueError('missing or unsafe LICENSE: provide the non-empty repository source license')
    required = ['LICENSE', 'NOTICE', 'examples', 'services/kokoro-runtime/runtime.py',
                'services/kokoro-runtime/doctor.py', 'services/kokoro-runtime/smoke_macos.py',
                'services/kokoro-runtime/acceptance.py',
                'services/kokoro-runtime/setup_kokoro.py',
                'services/kokoro-runtime/setup-manifest.json',
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
    if gpu_probe is not None:
        (resources / 'bin').mkdir(exist_ok=True)
        shutil.copy2(gpu_probe, resources / 'bin/gpu_probe')
    if head_pose is not None:
        (resources / 'bin').mkdir(exist_ok=True)
        shutil.copy2(head_pose, resources / 'bin/head_pose')
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
    if speech_bundle is not None:
        shutil.copytree(speech_bundle, resources / 'speech', symlinks=True)
    (resources / 'KoetaStudio.icns').write_bytes(app_icon())
    with (destination / 'Contents/Info.plist').open('wb') as handle:
        plistlib.dump(bundle_info(), handle)
    (resources / 'BUILD-INFO.txt').write_text(build_info(commit, arch, profile, speech), encoding='utf-8')
    return destination


def stage_distribution(repo, binary, directory, commit, arch, profile, gpu_probe=None, head_pose=None,
                       speech_bundle=None):
    """Stage the app and a source license readable without opening its bundle."""
    directory = Path(directory)
    bundle = stage_bundle(repo, binary, directory / APP_NAME, commit, arch, profile, gpu_probe, head_pose,
                          speech_bundle)
    shutil.copy2(bundle / 'Contents/Resources/LICENSE', directory / 'LICENSE')
    return bundle


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--gpu-probe', type=Path, help='Optional native GPU fixture helper')
    parser.add_argument('--head-pose', type=Path, help='Optional native local camera pose helper')
    parser.add_argument('--speech-bundle', type=Path,
                        help='Directory built by scripts/build_speech_bundle.py to ship inside the app')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--arch', choices=['arm64', 'x86_64', 'universal'], required=True)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    if sys.platform != 'darwin':
        parser.error('signing and archive creation require macOS')
    output = args.output.absolute()
    if output.suffix != '.zip' or output.exists():
        parser.error('output must be a new .zip file; existing output is never overwritten')
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='kilsat-package-') as temporary:
        bundle = stage_distribution(args.repo, args.binary, temporary,
                                    args.commit, args.arch, args.profile, args.gpu_probe, args.head_pose,
                                    args.speech_bundle)
        if args.gpu_probe is not None:
            helper = bundle / 'Contents/Resources/bin/gpu_probe'
            subprocess.run(['codesign', '--force', '--sign', '-', str(helper)], check=True)
            subprocess.run(['codesign', '--verify', '--strict', str(helper)], check=True)
        if args.head_pose is not None:
            helper = bundle / 'Contents/Resources/bin/head_pose'
            subprocess.run(['codesign', '--force', '--sign', '-', str(helper)], check=True)
            subprocess.run(['codesign', '--verify', '--strict', str(helper)], check=True)
        subprocess.run(['codesign', '--force', '--sign', '-', str(bundle)], check=True)
        subprocess.run(['codesign', '--verify', '--deep', '--strict', str(bundle)], check=True)
        # Archive the staging directory's contents: the app and adjacent LICENSE.
        # ditto preserves executable permissions and the app's directory layout.
        # With the speech bundle, extended attributes (download provenance) would add
        # an AppleDouble entry per file; the app needs none of them.
        attributes = (['--norsrc', '--noextattr', '--noacl'] if args.speech_bundle is not None
                      else ['--sequesterRsrc'])
        subprocess.run(['ditto', '-c', '-k', *attributes,
                        temporary, str(output)], check=True)
    print(output)


if __name__ == '__main__':
    main()
