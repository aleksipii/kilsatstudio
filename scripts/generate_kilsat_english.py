#!/usr/bin/env python3
"""Author three English Aino/Rane projects using the existing original rigs/sets."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

import generate_kilsat_season as original

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / 'examples/kilsat-english'
VOICES = {'AINO': 'af_heart', 'RANE': 'am_michael'}


def validate_catalog(catalog):
    if catalog.get('language') != 'en-us':
        raise ValueError('the English episode pack requires en-us')
    episodes = catalog.get('episodes', [])
    if [episode.get('number') for episode in episodes] != [1, 2, 3]:
        raise ValueError('exactly three ordered English episodes are required')
    if len({episode.get('title') for episode in episodes}) != 3:
        raise ValueError('episode titles must be distinct')
    for episode in episodes:
        turns = episode.get('turns', [])
        if (episode.get('set') not in original.SET_COLORS or len(turns) != 8
                or any(not isinstance(line, str) or not line.strip() or len(line) > 250 for line in turns)
                or 'Kilsat' not in turns[4]):
            raise ValueError('each episode needs a known set, eight spoken turns and a Kilsat insert')
    return episodes


def generate(destination=DEFAULT, stories_path=DEFAULT / 'stories.json'):
    destination = Path(destination)
    catalog = json.loads(Path(stories_path).read_text(encoding='utf-8'))
    episodes = validate_catalog(catalog)
    destination.mkdir(parents=True, exist_ok=True)
    (destination / 'assets').mkdir(exist_ok=True)
    screen = original.example_screen('en-us')
    (destination / 'assets/kilsat-example.png').write_bytes(screen)
    manifest = {
        'schema_version': 1, 'title': catalog['title'], 'language': 'en-us',
        'authorship': catalog['authorship'],
        'profile': dict(width=1920, height=1080, fps=24, min_episode_seconds=30, max_episode_seconds=60),
        'publishable': False,
        'acceptance': {key: False for key in ('audio_rights_reviewed', 'actual_kilsat_capture',
                       'artistic_review', 'real_audio_lipsync_review', 'physical_mac_300s')},
        'asset_sha256': {'assets/kilsat-example.png': original.sha(screen)}, 'episodes': [],
    }
    readable = ['# ' + catalog['title'], '',
                'Three English episodes. Aino is grounded; Rane is self-important and mean.',
                'The declared durations are drafts; actual speech determines final shot timing.', '']
    for story in episodes:
        project = original.episode_project(story)
        source = json.loads(project['source'])
        for actor, voice in VOICES.items():
            source['characters'][actor]['voice'] = {
                'provider': 'kokoro', 'preset': voice, 'language': 'en-us',
                'speed': 1.0 if actor == 'AINO' else 0.97,
            }
        source['project']['title'] = f'Kilsat EN {story["number"]:02d} - {story["title"]}'
        project['source'] = json.dumps(source, ensure_ascii=False, separators=(',', ':'))
        duration = round(sum(scene['duration'] for scene in source['scenes']), 6)
        if not 30 <= duration <= 60:
            raise ValueError('draft episode duration must remain within 30..60 seconds')
        name = f'ep{story["number"]:02d}.kilsat'
        data = original.encode(project)
        (destination / name).write_bytes(data)
        manifest['episodes'].append({
            'number': story['number'], 'title': story['title'], 'arc': story['arc'],
            'set': story['set'], 'project': name, 'project_sha256': original.sha(data),
            'source_sha256': original.sha(project['source'].encode()),
            'draft_duration_seconds': duration, 'turn_count': 8,
            'scene_count': len(source['scenes']), 'audio_status': 'not_synthesized',
            'video_status': 'not_rendered',
        })
        readable.extend([f'## {story["number"]:02d}. {story["title"]}',
                         f'Draft duration: {duration:.2f} seconds.', ''])
        readable.extend(f'{"AINO" if index % 2 == 0 else "RANE"}: {line}'
                        for index, line in enumerate(story['turns']))
        readable.extend(['', 'Picture: show the illustrative Kilsat mileage log after turn five; '
                         'finish with a silent reaction.', ''])
    (destination / 'manifest.json').write_bytes(original.encode(manifest))
    (destination / 'EPISODES.md').write_text('\n'.join(readable) + '\n', encoding='utf-8')
    (destination / 'assets/ORIGINAL-ASSETS.md').write_text(
        '# Original English episode assets\n\n'
        'The original Aino/Rane rigs and geometric sets are shared with the Finnish season.\n'
        'The English mileage-log prop is generated from original geometric lettering.\n'
        'It is visibly marked EXAMPLE, not a real Kilsat screenshot.\n'
        'No model weights or third-party images are included. Final artwork, audio/model\n'
        'rights and an approved real product capture still require review.\n', encoding='utf-8')
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=DEFAULT)
    parser.add_argument('--stories', type=Path, default=DEFAULT / 'stories.json')
    args = parser.parse_args()
    result = generate(args.output, args.stories)
    print(json.dumps({'episodes': len(result['episodes']), 'language': result['language'], 'publishable': False}))


if __name__ == '__main__':
    main()
