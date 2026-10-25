#!/usr/bin/env python3
"""Build a hash-bound editable asset pack and a real silent Full HD showcase.

Uses the existing Rust evaluator/exporters and independent FFmpeg checks. This
is an artwork starter pack, not artistic, physical-device or publication approval.
No model, voice, network download or alternate animation clock is involved.
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import struct
import sys
import time
import wave
import zipfile

from render_kilsat_season import (MAX_JSON, file_hash, inspect_media, probe,
                                  read_json, run_bounded, write_json)

REPO = Path(__file__).resolve().parents[1]
COUNTS = {'character': 6, 'background': 4, 'prop': 20}
HAND_PROP_IDS = ('puhelin', 'puhelin_takana', 'puhelin_vino', 'kahvikuppi', 'kirja',
                 'avain', 'tabletti', 'kyna', 'tyokaluavain')
DIRECTORIES = {'character': 'characters', 'background': 'backgrounds', 'prop': 'props'}
PROFILE = {'width': 1920, 'height': 1080, 'fps': 24, 'seconds': 30, 'frames': 720,
           'audio_samples': 720_000, 'speech': False}
MAX_OUTPUT = 256 * 1024 * 1024
MAX_ARCHIVE = 50 * 1024 * 1024
MAX_HAND_CATALOG = 128 * 1024


def safe_path(path: Path) -> Path:
    path = path.absolute()
    if any(part.is_symlink() for part in (path, *path.parents)):
        raise ValueError('source/output paths cannot traverse symbolic links')
    return path


def pack_path(root: Path, relative: str) -> Path:
    if (not isinstance(relative, str) or '\\' in relative or '\0' in relative
            or not relative or any(part in ('', '.', '..') for part in relative.split('/'))
            or Path(relative).is_absolute()):
        raise ValueError('pack paths must be relative and stay inside the source directory')
    return safe_path(root / relative)


def silent_project(path: Path) -> dict:
    project = read_json(path)
    if (project.get('format_version') != 2 or project.get('assets') != []
            or project.get('timing', {}).get('line', '').strip()
            or project.get('timing', {}).get('scene', '').strip()
            or not isinstance(project.get('source'), str)):
        raise ValueError('stick projects must be self-contained version-two documents without speech timing')
    source = json.loads(project['source'])
    if not isinstance(source, dict) or not isinstance(source.get('scenes'), list):
        raise ValueError('expected an explicit native JSON screenplay')
    if any(scene.get('composition', {}).get('audio_cues')
           or any(event.get('type') == 'speak' for event in scene.get('events', []))
           for scene in source['scenes']):
        raise ValueError('this silent asset showcase cannot contain speech or audio cues')
    return source


def hand_prop_catalog(path: Path) -> dict:
    """Check the bounded local-geometry interchange boundary, not reimplement FK.

    The Rust rig validator remains responsible for polygon simplicity and the
    canonical scene evaluator remains responsible for all hand transforms.
    """
    file_hash(path, MAX_HAND_CATALOG)
    catalog = read_json(path)
    if (not isinstance(catalog, dict) or set(catalog) != {'schema_version', 'items'}
            or type(catalog.get('schema_version')) is not int or catalog['schema_version'] != 1
            or not isinstance(catalog.get('items'), list)
            or len(catalog['items']) != len(HAND_PROP_IDS)):
        raise ValueError('hand-prop catalog requires schema 1 and exactly nine items')
    def number(value, bound=10_000):
        return type(value) in (int, float) and math.isfinite(value) and abs(value) <= bound
    def rgba(value):
        return isinstance(value, list) and len(value) == 4 and all(type(v) is int and 0 <= v <= 255 for v in value)
    for expected, item in zip(HAND_PROP_IDS, catalog['items']):
        if (not isinstance(item, dict)
                or set(item) != {'id', 'name', 'width', 'height', 'grip', 'parts', 'grip_parts'}
                or item.get('id') != expected or not isinstance(item.get('name'), str)
                or not 1 <= len(item['name']) <= 128 or any(ord(c) < 32 for c in item['name'])
                or not all(number(item.get(key), 512) and item[key] > 0 for key in ('width', 'height'))
                or item.get('grip') != [0, 0] or any(type(v) not in (int, float) for v in item['grip'])
                or not isinstance(item.get('parts'), list) or not item['parts']
                or not isinstance(item.get('grip_parts'), list) or not item['grip_parts']
                or len(item['parts']) + len(item['grip_parts']) > 32):
            raise ValueError('hand-prop catalog IDs, grip, dimensions or native parts differ')
        seen = set()
        for part in item['parts'] + item['grip_parts']:
            if (not isinstance(part, dict) or not {'id', 'joint', 'color'} <= set(part)
                    or not set(part) <= {'id', 'joint', 'color', 'points', 'ellipse', 'outline'}
                    or not isinstance(part.get('id'), str) or not re.fullmatch(r'[a-z][a-z0-9_]{0,63}', part['id'])
                    or part['id'] in seen or part['joint'] != 'grip' or not rgba(part['color'])
                    or ('points' in part) == ('ellipse' in part)):
                raise ValueError('hand props must contain unique local native polygon/ellipse parts only')
            seen.add(part['id'])
            if 'points' in part:
                points = part['points']
                if (not isinstance(points, list) or not 3 <= len(points) <= 128
                        or any(not isinstance(p, list) or len(p) != 2 or not all(number(v) for v in p) for p in points)):
                    raise ValueError('hand-prop polygon exceeds finite vertex bounds')
            if 'ellipse' in part:
                ellipse = part['ellipse']
                if (not isinstance(ellipse, dict) or set(ellipse) != {'cx', 'cy', 'rx', 'ry'}
                        or not all(number(v) for v in ellipse.values())
                        or ellipse['rx'] <= 0 or ellipse['ry'] <= 0):
                    raise ValueError('hand-prop ellipse exceeds finite positive bounds')
            if 'outline' in part:
                outline = part['outline']
                if (not isinstance(outline, dict) or not {'color', 'width'} <= set(outline)
                        or not set(outline) <= {'color', 'width', 'mode'} or not rgba(outline['color'])
                        or not number(outline['width'], 64) or outline['width'] < 0
                        or outline.get('mode', 'over') not in ('over', 'behind', 'silhouette')):
                    raise ValueError('hand-prop outline exceeds native bounds')
    return catalog


def source_identity(root: Path, roadmap: Path) -> tuple[dict, dict]:
    root, roadmap = safe_path(root), safe_path(roadmap)
    manifest = read_json(root / 'manifest.json')
    if (manifest.get('schema_version') != 1
            or type(manifest.get('pack_revision')) is not int or manifest['pack_revision'] != 2
            or manifest.get('artistic_review') is not False
            or manifest.get('physical_mac_review') is not False
            or manifest.get('starter_project') != 'stick-studio.kilsat'):
        raise ValueError('expected an unreviewed schema-one revision-two stick-studio manifest')
    items = manifest.get('items')
    if (not isinstance(items, list) or len(items) != 30
            or any(not isinstance(item, dict) for item in items)
            or Counter(item.get('kind') for item in items) != COUNTS):
        raise ValueError('stick pack requires exactly six characters, four backgrounds and 20 props')
    hashes = {'manifest.json': file_hash(root / 'manifest.json', MAX_JSON)}
    if manifest.get('hand_props_catalog') != 'hand-props.json':
        raise ValueError('hand-prop catalog path must be hand-props.json')
    catalog = pack_path(root, manifest['hand_props_catalog'])
    hand_prop_catalog(catalog)
    hashes['hand-props.json'] = file_hash(catalog, MAX_HAND_CATALOG)
    if hashes['hand-props.json'] != manifest.get('hand_props_sha256'):
        raise ValueError('hand-prop catalog changed')
    seen = set()
    for item in items:
        identifier, kind = item.get('id'), item.get('kind')
        if (not isinstance(identifier, str) or not re.fullmatch(r'[a-z][a-z0-9_-]{0,63}', identifier)
                or identifier in seen):
            raise ValueError('asset identifiers must be unique safe lowercase names')
        seen.add(identifier)
        relative = item.get('project')
        if relative != f'{DIRECTORIES[kind]}/{identifier}.kilsat':
            raise ValueError('project path must match its asset category and identifier')
        if item.get('transparent') is not (kind != 'background'):
            raise ValueError('characters/props must be transparent; backgrounds must be opaque')
        dimensions = item.get('export_width'), item.get('export_height')
        if any(type(value) is not int or not 1 <= value <= 4096 for value in dimensions):
            raise ValueError('invalid bounded asset export dimensions')
        path = pack_path(root, relative)
        hashes[relative] = file_hash(path, MAX_JSON)
        if hashes[relative] != item.get('project_sha256'):
            raise ValueError(f'project changed: {relative}')
        source = silent_project(path)
        if (source.get('project', {}).get('width'), source.get('project', {}).get('height')) != dimensions:
            raise ValueError('project dimensions differ from the asset export dimensions')
    starter = root / 'stick-studio.kilsat'
    hashes['stick-studio.kilsat'] = file_hash(starter, MAX_JSON)
    if hashes['stick-studio.kilsat'] != manifest.get('starter_sha256'):
        raise ValueError('starter project changed')
    source = silent_project(starter)
    profile = source.get('project', {})
    if ((profile.get('width'), profile.get('height'), profile.get('fps')) != (1920, 1080, 24)
            or len(source.get('characters', {})) != 6 or len(source['scenes']) != 6
            or any(scene.get('duration') != 5 for scene in source['scenes'])
            or len(read_json(starter)['source'].encode()) != manifest.get('starter_source_bytes')):
        raise ValueError('starter must retain six characters and six five-second Full HD scenes')
    hashes['README.md'] = file_hash(root / 'README.md', MAX_JSON)
    return manifest, {'schema_version': 1, 'pack_revision': 2, 'source_sha256': hashes,
                      'roadmap': {'path': str(roadmap), 'sha256': file_hash(roadmap, MAX_JSON)},
                      'license_notices': {name: file_hash(REPO/name, MAX_JSON) for name in ('LICENSE','NOTICE')},
                      'profile': PROFILE, 'artistic_review': False, 'physical_mac_review': False,
                      'publication_approved': False}


def tool_identity(paths: dict[str, Path]) -> dict:
    result = {}
    for name, path in paths.items():
        if not path.is_absolute() or not os.access(path, os.X_OK):
            raise ValueError('tools must be explicit absolute executable paths')
        actual = path.resolve(strict=True)
        result[name] = {'path': str(actual), 'sha256': file_hash(actual)}
    return result


def catalog_identity(path: Path) -> dict:
    if not path.is_absolute():
        raise ValueError('catalog must be an explicit absolute PNG path')
    path = safe_path(path)
    digest = file_hash(path, 16 * 1024 * 1024)
    with path.open('rb') as stream:
        header = stream.read(24)
    if len(header) != 24 or header[:8] != b'\x89PNG\r\n\x1a\n' or header[12:16] != b'IHDR':
        raise ValueError('catalog must have a PNG image header')
    width,height = struct.unpack('>II',header[16:24])
    if not 1 <= width <= 4096 or not 1 <= height <= 4096:
        raise ValueError('catalog image dimensions exceed the 4096-pixel budget')
    return {'path':str(path),'sha256':digest,'width':width,'height':height}


def validate_images(directory: Path, manifest: dict) -> dict:
    expected = {'export-report.json'}
    hashes = {}
    report = read_json(directory / 'export-report.json')
    exported = report.get('items')
    if (report.get('schema_version') != 1 or report.get('pack_revision') != 2 or not isinstance(exported, list)
            or len(exported) != len(manifest['items'])):
        raise ValueError('native asset exporter report is incomplete')
    observed = {(item.get('kind'),item.get('id')): item for item in exported}
    if len(observed) != 30:
        raise ValueError('native asset exporter report has duplicate items')
    for item in manifest['items']:
        actual = observed.get((item['kind'],item['id']), {})
        pixels = item['export_width'] * item['export_height']
        if ((actual.get('width'), actual.get('height'), actual.get('transparent')) !=
                (item['export_width'], item['export_height'], item['transparent'])
                or actual.get('declared_project_sha256') != item['project_sha256']
                or actual.get('source_sha256_verified_by_this_exporter') is not False
                or type(actual.get('transparent_pixels')) is not int
                or type(actual.get('visible_pixels')) is not int
                or not 0 < actual['visible_pixels'] <= pixels
                or not 0 <= actual['transparent_pixels'] < pixels
                or (item['transparent'] and actual['transparent_pixels'] == 0)
                or (not item['transparent'] and actual['transparent_pixels'] != 0)):
            raise ValueError('native asset report dimensions, source declaration or raster alpha differ')
        for extension in ('svg', 'png'):
            relative = f'{item["kind"]}/{item["id"]}.{extension}'
            expected.add(relative)
            path = pack_path(directory, relative)
            hashes[relative] = file_hash(path, 16 * 1024 * 1024)
            if extension == 'png':
                with path.open('rb') as stream:
                    header = stream.read(24)
                if (len(header) != 24 or header[:8] != b'\x89PNG\r\n\x1a\n'
                        or header[12:16] != b'IHDR'
                        or struct.unpack('>II', header[16:24]) != (item['export_width'], item['export_height'])):
                    raise ValueError(f'PNG dimensions differ from the manifest: {relative}')
    actual = set()
    total = 0
    for path in directory.rglob('*'):
        safe_path(path)
        if path.is_file():
            actual.add(path.relative_to(directory).as_posix())
            total += path.stat().st_size
    if actual != expected or total > MAX_OUTPUT:
        raise ValueError('asset exporter output inventory or total byte budget differs')
    # The native exporter validates raster alpha using the same renderer as preview.
    hashes['export-report.json'] = file_hash(directory / 'export-report.json', MAX_JSON)
    return hashes


def validate_video(directory: Path, ffmpeg: Path, ffprobe: Path, logs: Path) -> dict:
    pipeline = read_json(directory / 'pipeline-report.json')
    streaming = pipeline.get('streaming', {})
    if (pipeline.get('schema_version') != 1 or pipeline.get('status') != 'completed'
            or pipeline.get('publication_approved') is not False
            or pipeline.get('dialogue_audio') != []
            or pipeline.get('private_system_preview_used') is not False
            or pipeline.get('review') is not False
            or (streaming.get('width'), streaming.get('height'), streaming.get('fps'),
                streaming.get('frame_count'), streaming.get('total_samples')) != (1920, 1080, 24, 720, 720_000)):
        raise ValueError('production pipeline did not complete the exact silent 30-second Full HD showcase')
    media = probe(ffprobe, directory / 'sequence.mp4', logs, 'independent-probe')
    observed = inspect_media(media, 1920, 1080, 720)
    run_bounded([str(ffmpeg), '-v', 'error', '-xerror', '-i', str(directory / 'sequence.mp4'),
                 '-map', '0:v:0', '-map', '0:a:0', '-f', 'null', '-'], logs, 'full-decode', 180)
    with wave.open(str(directory / 'audio.wav'), 'rb') as stream:
        if (stream.getnchannels(), stream.getsampwidth(), stream.getframerate(), stream.getnframes()) != (1, 2, 24_000, 720_000):
            raise ValueError('showcase PCM sample clock differs')
        pcm = stream.readframes(720_001)
        if len(pcm) != 1_440_000 or any(pcm):
            raise ValueError('the silent showcase must contain exactly its declared zero PCM samples')
    return {'observed': observed, 'full_decode_passed': True, 'speech_synthesized': False,
            'silent_pcm_confirmed': True, 'sha256': {
                name: file_hash(directory / name) for name in ('sequence.mp4', 'audio.wav', 'pipeline-report.json')}}


def archive_pack(output: Path) -> Path:
    # Publish only completed deliverables. A renderer's best-effort cleanup can
    # leave staging trees behind; those are never delivery inputs, even when
    # they happen to contain a plausible sequence.mp4.
    directories = {'images', 'video', 'projects', 'logs'}
    files = {'LICENSE', 'NOTICE', 'ALOITA-TAALTA.md', 'professional-studio-next.md',
             'pack-report.json', 'render-inputs.json'}
    optional = {'HAHMOT-JA-ESINEET.png'}
    output = safe_path(output)
    if not output.is_dir():
        raise ValueError('completed delivery must be a regular directory')
    entries = {entry.name: entry for entry in output.iterdir()
               if not entry.name.startswith('.kilsat-')}
    if set(entries) - directories - files - optional:
        raise ValueError('unexpected root entry in completed delivery')
    if not directories | files <= set(entries):
        raise ValueError('completed delivery is missing a required root entry')
    inputs = []
    for name, entry in sorted(entries.items()):
        safe_path(entry)
        if name in directories:
            if not entry.is_dir():
                raise ValueError('completed delivery directory has the wrong entry kind')
            for parent, children, names in os.walk(entry, followlinks=False):
                # Prune staging directories before walking them. Check every
                # retained directory explicitly so symlinks cannot be skipped
                # silently by os.walk(followlinks=False).
                children[:] = sorted(child for child in children if not child.startswith('.kilsat-'))
                for child in children:
                    safe_path(Path(parent) / child)
                for child in sorted(names):
                    if child.startswith('.kilsat-'):
                        continue
                    path = safe_path(Path(parent) / child)
                    if not path.is_file():
                        raise ValueError('completed delivery contains a non-regular file')
                    inputs.append(path)
        else:
            if not entry.is_file():
                raise ValueError('completed delivery file has the wrong entry kind')
            inputs.append(entry)
    archive = output.parent / 'Kilsat-Tikku-ukkostudio.zip'
    # Never replace another completed delivery.
    created = False
    try:
        with zipfile.ZipFile(archive, 'x', compression=zipfile.ZIP_DEFLATED, compresslevel=6) as target:
            created = True
            total = 0
            for path in sorted(inputs):
                safe_path(path)
                size = path.stat().st_size
                total += size
                if total > MAX_OUTPUT:
                    raise ValueError('delivery exceeds its uncompressed byte budget')
                target.write(path, path.relative_to(output).as_posix())
        if archive.stat().st_size >= MAX_ARCHIVE:
            raise ValueError('delivery ZIP exceeds the 50 MiB single-file budget')
    except BaseException:
        if created:
            archive.unlink()
        raise
    return archive


def build(pack: Path, roadmap: Path, paths: dict[str, Path], output: Path,
          catalog: Path | None = None) -> dict:
    pack, output = safe_path(pack), safe_path(output)
    archive = output.parent / 'Kilsat-Tikku-ukkostudio.zip'
    if output.exists() or archive.exists() or output.is_relative_to(pack):
        raise ValueError('select a new output directory and ZIP outside the source pack')
    manifest, identity = source_identity(pack, roadmap)
    identity['tools'] = tool_identity(paths)
    if catalog is not None:
        identity['catalog'] = catalog_identity(catalog)
    output.mkdir(parents=True)
    logs = output / 'logs'
    logs.mkdir()
    write_json(output / 'render-inputs.json', identity)
    started = time.monotonic()
    report = {'schema_version': 1, 'status': 'failed', 'profile': PROFILE,
              'artistic_review': False, 'physical_mac_review': False, 'publication_approved': False}
    try:
        run_bounded([identity['tools']['asset_exporter']['path'], '--pack', str(pack),
                     '--output', str(output / 'images')], logs, 'assets', 300)
        report['image_sha256'] = validate_images(output / 'images', manifest)
        run_bounded([identity['tools']['exporter']['path'], '--project', str(pack / 'stick-studio.kilsat'),
                     '--ffmpeg', identity['tools']['ffmpeg']['path'], '--output', str(output / 'video'),
                     '--review-frames', '--deadline-seconds', '600'], logs, 'production-export', 630)
        report['video'] = validate_video(output / 'video', Path(identity['tools']['ffmpeg']['path']),
                                         Path(identity['tools']['ffprobe']['path']), logs)
        _, after = source_identity(pack, roadmap)
        after['tools'] = tool_identity(paths)
        if catalog is not None:
            after['catalog'] = catalog_identity(catalog)
        if after != identity:
            raise ValueError('source artwork, documentation or tools changed during rendering')
        projects = output / 'projects'
        projects.mkdir()
        for relative in identity['source_sha256']:
            source = pack_path(pack, relative)
            target = projects / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            if file_hash(target, MAX_JSON) != identity['source_sha256'][relative]:
                raise ValueError('copied source differs from the checked source')
        shutil.copyfile(roadmap, output / 'professional-studio-next.md')
        if file_hash(output / 'professional-studio-next.md', MAX_JSON) != identity['roadmap']['sha256']:
            raise ValueError('copied roadmap differs from checked source')
        for name,digest in identity['license_notices'].items():
            shutil.copyfile(REPO/name, output/name)
            if file_hash(output/name, MAX_JSON) != digest:
                raise ValueError('copied license/notice differs from checked source')
        shutil.copyfile(pack / 'README.md', output / 'ALOITA-TAALTA.md')
        if catalog is not None:
            shutil.copyfile(catalog,output/'HAHMOT-JA-ESINEET.png')
            report['catalog_sha256'] = file_hash(output/'HAHMOT-JA-ESINEET.png',16*1024*1024)
            if report['catalog_sha256'] != identity['catalog']['sha256']:
                raise ValueError('copied catalog differs from the checked image')
        report.update(status='completed', item_counts=COUNTS,
                      source_identity_sha256=hashlib.sha256(json.dumps(identity,sort_keys=True).encode()).hexdigest())
    except (OSError, ValueError, KeyError, TypeError, TimeoutError, wave.Error) as error:
        report['error'] = str(error)
    report['elapsed_seconds'] = time.monotonic() - started
    write_json(output / 'pack-report.json', report)
    if report['status'] == 'completed':
        archive = archive_pack(output)
        report['archive'] = {'path': str(archive), 'sha256': file_hash(archive, MAX_ARCHIVE),
                             'size_bytes': archive.stat().st_size}
    return report


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pack', type=Path, default=REPO / 'examples/stick-studio')
    parser.add_argument('--roadmap', type=Path, default=REPO / 'docs/professional-studio-next.md')
    parser.add_argument('--asset-exporter', type=Path, required=True)
    parser.add_argument('--exporter', type=Path, required=True)
    parser.add_argument('--ffmpeg', type=Path, required=True)
    parser.add_argument('--ffprobe', type=Path, required=True)
    parser.add_argument('--catalog', type=Path, help='Optional absolute PNG of the actually rendered assets')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        report = build(args.pack, args.roadmap,
                       {name: getattr(args,name) for name in ('asset_exporter','exporter','ffmpeg','ffprobe')},
                       args.output, args.catalog)
        print(json.dumps(report, ensure_ascii=False))
        return 0 if report['status'] == 'completed' else 1
    except (OSError, ValueError, KeyError, TypeError, TimeoutError, zipfile.BadZipFile) as error:
        print(json.dumps({'status':'failed','error':str(error)}), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
