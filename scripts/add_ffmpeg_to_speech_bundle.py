#!/usr/bin/env python3
"""Add a built FFmpeg to a built speech bundle, so the app ships both.

  scripts/add_ffmpeg_to_speech_bundle.py SPEECH_BUNDLE FFMPEG_BUNDLE

SPEECH_BUNDLE comes from scripts/build_speech_bundle.py, FFMPEG_BUNDLE from
scripts/build_ffmpeg_bundle.sh. FFmpeg is copied to SPEECH_BUNDLE/ffmpeg and
recorded under "ffmpeg" in speech-bundle.json; the desktop then fills in the
FFmpeg path for every export by itself. Nothing is downloaded.
"""
import argparse
import json
from pathlib import Path
import shutil
import sys

DESCRIPTOR = 'speech-bundle.json'
FFMPEG_DESCRIPTOR = 'ffmpeg-bundle.json'


def add(speech, ffmpeg):
    speech, ffmpeg = Path(speech), Path(ffmpeg)
    for directory in (speech, ffmpeg):
        if directory.is_symlink() or not directory.is_dir():
            raise ValueError(f'not a directory: {directory}')
    descriptor = json.loads((speech / DESCRIPTOR).read_text(encoding='utf-8'))
    built = json.loads((ffmpeg / FFMPEG_DESCRIPTOR).read_text(encoding='utf-8'))
    if built.get('schema_version') != 1:
        raise ValueError('unsupported FFmpeg bundle descriptor')
    target = speech / 'ffmpeg'
    if target.exists() or 'ffmpeg' in descriptor:
        raise ValueError('speech bundle already contains FFmpeg')
    for path in ffmpeg.rglob('*'):
        if path.is_symlink() or not (path.is_file() or path.is_dir()):
            raise ValueError(f'FFmpeg bundle may contain only regular files: {path}')
    for key in ('ffmpeg', 'ffprobe'):
        if not (ffmpeg / built[key]).is_file():
            raise ValueError(f'FFmpeg bundle lacks {built[key]}')
    shutil.copytree(ffmpeg, target)
    descriptor['ffmpeg'] = dict(
        executable=f"ffmpeg/{built['ffmpeg']}", ffprobe=f"ffmpeg/{built['ffprobe']}",
        version=built['ffmpeg_version'], x264_commit=built['x264_commit'], license=built['license'])
    (speech / DESCRIPTOR).write_text(json.dumps(descriptor, indent=2, sort_keys=True) + '\n',
                                     encoding='utf-8')
    return target


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('speech_bundle', type=Path)
    parser.add_argument('ffmpeg_bundle', type=Path)
    args = parser.parse_args(argv)
    try:
        print(add(args.speech_bundle, args.ffmpeg_bundle))
    except (OSError, ValueError, KeyError) as error:
        print(f'error: {error}', file=sys.stderr)
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
