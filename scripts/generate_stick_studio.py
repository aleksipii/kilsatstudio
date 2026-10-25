#!/usr/bin/env python3
"""Generate original, self-contained, editable stick-figure studio projects."""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path

from stick_characters import CAST, JOINT_IDS, characters
from stick_scenery import BACKGROUNDS, PROPS

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / 'examples/stick-studio'


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + '\n').encode('utf-8')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def key(at, **values):
    return dict(at=at, easing='smoothstep', **values)


def clips():
    """Authored joint-offset clips; apply them through the existing rig-clip tool."""
    def clip(tracks, duration=4):
        return dict(duration=duration, joint_tracks=tracks)
    return {
        'odotus': clip({'head': [key(0), key(2, rotation=0.035), key(4)],
                        'torso': [key(0), key(2, y=-2), key(4)]}),
        'vilkutus': clip({'upper_arm_l': [key(0), key(0.6, rotation=2.3), key(3, rotation=2.3), key(4)],
                          'forearm_l': [key(0), key(0.6, rotation=-0.7), key(1.2, rotation=-0.3),
                                        key(1.8, rotation=-0.9), key(2.4, rotation=-0.3), key(3, rotation=-0.7), key(4)]}),
        'osoitus': clip({'upper_arm_l': [key(0), key(1, rotation=1.55), key(3, rotation=1.55), key(4)],
                         'head': [key(0), key(1, rotation=-0.08), key(4)]}),
        'reaktio': clip({'head': [key(0), key(0.35, rotation=-0.16, y=-4), key(1, rotation=0.06), key(4)],
                         'torso': [key(0), key(0.35, rotation=0.06), key(4)]}),
        'askel': clip({'thigh_l': [key(0, rotation=-0.22), key(0.6, rotation=0.22), key(1.2, rotation=-0.22)],
                       'thigh_r': [key(0, rotation=0.22), key(0.6, rotation=-0.22), key(1.2, rotation=0.22)],
                       'shin_l': [key(0), key(0.6, rotation=-0.18), key(1.2)],
                       'shin_r': [key(0, rotation=-0.18), key(0.6), key(1.2, rotation=-0.18)]}, 1.2),
    }


def source(title, cast, scenes, width=1920, height=1080, with_clips=True):
    data = dict(project=dict(title=title, width=width, height=height, fps=24),
                characters=cast, scenes=scenes)
    if cast and with_clips:
        data['rig_clips'] = clips()
    return json.dumps(data, ensure_ascii=False, separators=(',', ':'))


def document(screenplay, layers):
    return dict(format_version=2, source=screenplay, assets=[], art=dict(layers=layers))


def place_layers(layers, scene_id, prefix, x=0, y=0, scale=1):
    """Transform native artwork, retaining its editable geometric content."""
    result = copy.deepcopy(layers)
    for layer in result:
        layer['id'] = f'{scene_id}_{prefix}_{layer["id"]}'
        layer['scene_id'] = scene_id
        if layer.get('clip') is not None:
            rect = layer['clip']
            rect.update(x=x + rect['x'] * scale, y=y + rect['y'] * scale,
                        width=rect['width'] * scale, height=rect['height'] * scale)
        content = layer['content']
        if content['type'] in ('rectangle', 'ellipse'):
            rect = content['rect']
            rect.update(x=x + rect['x'] * scale, y=y + rect['y'] * scale,
                        width=rect['width'] * scale, height=rect['height'] * scale)
        elif content['type'] == 'stroke':
            content['points'] = [dict(x=x + p['x'] * scale, y=y + p['y'] * scale)
                                 for p in content['points']]
            content['width'] *= scale
        else:
            raise ValueError('only original editable geometric artwork is supported')
    return result


def scene(identifier, cast, active, background, duration=5, scale=1.6, width=1920, height=1080):
    positions = {actor: (index - (len(active) - 1) / 2) * 710
                 for index, actor in enumerate(active)}
    composition = dict(
        background_color=background,
        camera=[dict(at=0, x=0, y=0, zoom=1), dict(at=duration, x=0, y=-8, zoom=1.025)],
        layers={actor: dict(z=index, visible=actor in active) for index, actor in enumerate(cast)},
        actor_tracks={actor: [key(0, x=x, y=-12 if height == 1080 else -20,
                                  scale_x=scale, scale_y=scale)] for actor, x in positions.items()},
        joint_tracks={actor: {'head': [key(0), key(1.5, rotation=0.04 if index else -0.05), key(duration)]}
                      for index, actor in enumerate(active)},
    )
    if len(active) == 2:
        composition['gaze_tracks'] = {actor: [dict(at=0, target=active[1-index])]
                                       for index, actor in enumerate(active)}
    return dict(id=identifier, duration=duration, composition=composition, events=[])


def hand_catalog():
    raw = (DEFAULT / 'hand-props.json').read_bytes()
    if len(raw) > 128 * 1024:
        raise ValueError('hand prop catalog exceeds 128 KiB')
    catalog = json.loads(raw)
    expected = ['puhelin', 'puhelin_takana', 'puhelin_vino', 'kahvikuppi',
                'kirja', 'avain', 'tabletti', 'kyna', 'tyokaluavain']
    if catalog.get('schema_version') != 1 or [item['id'] for item in catalog['items']] != expected:
        raise ValueError('unexpected native hand prop catalog')
    return raw, {item['id']: item for item in catalog['items']}


def held_prop(character, item, hand):
    """Ordinary hand-bound rig parts share all existing joint/root evaluation."""
    prefix = f'heldprop_{hand}_'
    parts = character['rig']['parts']
    parts[:] = [part for part in parts if not part['id'].startswith(prefix)]
    for original in item['parts'] + item['grip_parts']:
        part = copy.deepcopy(original)
        part['id'] = f'{prefix}{item["id"]}_{part["id"]}'
        part['joint'] = f'hand_{hand}'
        parts.append(part)


def prop_project(item):
    # The standalone image contains the exact held geometry, without fingers.
    points = []
    for part in item['parts']:
        if 'ellipse' in part:
            e = part['ellipse']
            points.extend([(e['cx']-e['rx'], e['cy']-e['ry']),
                           (e['cx']+e['rx'], e['cy']+e['ry'])])
        else:
            points.extend(part['points'])
    xmin, ymin = map(min, zip(*points))
    xmax, ymax = map(max, zip(*points))
    factor = 420 / max(xmax-xmin, ymax-ymin)
    parts = copy.deepcopy(item['parts'])
    for part in parts:
        part['joint'] = 'root'
    cast = {'ESINE': {'asset': f'props/{item["id"]}-original',
                      'rig': {'version': 1, 'id': f'{item["id"]}_original',
                              'joints': [{'id': 'root', 'parent': None}],
                             'parts': parts}}}
    shot = dict(id='esine', duration=5, events=[], composition=dict(
        background_color='#f4f1e9', camera=[dict(at=0, x=0, y=0, zoom=1)],
        actor_tracks={'ESINE': [key(0, x=-(xmin+xmax)/2*factor,
                                     y=256-512*0.7-(ymin+ymax)/2*factor,
                                     scale_x=factor, scale_y=factor)]}))
    return document(source(item['name'], cast, [shot], 512, 512, with_clips=False), [])


def main_project(cast, catalog):
    cast = copy.deepcopy(cast)
    for actor, hand, identifier in [('VEIKKO', 'r', 'puhelin'),
                                     ('KIIRE', 'l', 'puhelin_takana'),
                                     ('SISU', 'l', 'kahvikuppi'),
                                     ('KIPINA', 'r', 'tyokaluavain')]:
        held_prop(cast[actor], catalog[identifier], hand)
    shots = [
        ('vuoron_alku', 'toimisto', ['NOUTAJA', 'VEIKKO'], 'vilkutus', 'ajopaivakirja'),
        ('pomo_kutsuu', 'toimisto', ['PASMA', 'KIIRE'], 'osoitus', 'toimistotuoli'),
        ('pysakkipaniikki', 'pysakki', ['VEIKKO', 'KIIRE'], 'reaktio', 'sateenvarjo'),
        ('kahvitauko', 'kahvila', ['NOUTAJA', 'SISU'], 'odotus', 'kahvikuppi'),
        ('keksinto', 'hautausmaa', ['KIPINA', 'NOUTAJA'], 'vilkutus', 'tyokaluavain'),
        ('mummo_paataa', 'kahvila', ['PASMA', 'SISU'], 'reaktio', 'vuoronumerolaite'),
    ]
    scenes, layers = [], []
    framings = [(0, 0, 1), (0, 30, 1.2), (-310, -105, 2.6),
                (355, -5, 1.9), (-355, 10, 1.8), (355, -35, 2.8)]
    for index, (identifier, setting, active, clip_name, prop) in enumerate(shots):
        backdrop = BACKGROUNDS[setting]
        shot = scene(identifier, cast, active, backdrop['color'])
        # Rig clips are reusable source data; copied joint tracks use the native evaluator.
        for actor in active:
            shot['composition']['joint_tracks'][actor].update(copy.deepcopy(clips()[clip_name]['joint_tracks']))
        cx, cy, zoom = framings[index]
        shot['composition']['camera'] = [dict(at=0, x=cx, y=cy, zoom=zoom),
                                          dict(at=5, x=cx, y=cy-3, zoom=zoom*1.012)]
        # Close shots isolate their subject; a cropped second face or scythe
        # distracts from the hand action. Actor placement remains editable.
        if index in (2, 3, 4, 5):
            subject = active[0] if index in (2, 4) else active[1]
            for actor in active:
                shot['composition']['layers'][actor]['visible'] = actor == subject
        if index == 2:
            shot['composition']['joint_tracks']['VEIKKO'].update({
                'upper_arm_r': [key(0, rotation=-1.25), key(1.2, rotation=-1.4), key(5, rotation=-1.4)],
                'forearm_r': [key(0, rotation=-2.1), key(1.2, rotation=-2.3), key(5, rotation=-2.3)],
                'hand_r': [key(0, rotation=3.51), key(1.2, rotation=3.86), key(5, rotation=3.86)]})
        if index in (3, 5):
            shot['composition']['joint_tracks']['SISU'].update({
                'upper_arm_l': [key(0, rotation=0.65), key(2.5, rotation=0.8), key(5, rotation=0.65)],
                'forearm_l': [key(0, rotation=1.1), key(2.5, rotation=1.25), key(5, rotation=1.1)],
                'hand_l': [key(0, rotation=-1.91), key(2.5, rotation=-2.21), key(5, rotation=-1.91)]})
        shot['events'] = [dict(type='emotion', actor=active[0], at=0,
                               value='surprised' if clip_name == 'reaktio' else 'happy')]
        scenes.append(shot)
        layers.extend(place_layers(backdrop['layers'], identifier, 'tausta'))
    return document(source('Tikku-ukkostudio 2 – esineet ja kuvakoot', cast, scenes), layers)


def generate(destination=DEFAULT):
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    cast = characters()
    catalog_bytes, catalog = hand_catalog()
    if destination != DEFAULT:
        (destination / 'hand-props.json').write_bytes(catalog_bytes)
    starter = main_project(cast, catalog)
    starter_bytes = encoded(starter)
    if len(starter['source'].encode()) > 100_000:
        raise ValueError('compact starter source must remain below 100 KB')
    (destination / 'stick-studio.kilsat').write_bytes(starter_bytes)
    manifest = dict(schema_version=1, pack_revision=2, title='Tikku-ukkostudio 2',
                    authorship='Original independently authored editable geometric artwork; no third-party artwork or model assets.',
                    cast=CAST, joint_ids=JOINT_IDS, starter_project='stick-studio.kilsat',
                    hand_props_catalog='hand-props.json', hand_props_sha256=sha(catalog_bytes),
                    starter_sha256=sha(starter_bytes), starter_source_bytes=len(starter['source'].encode()),
                    artistic_review=False, physical_mac_review=False, items=[])
    def add(identifier, name, kind, project, transparent, width, height):
        directory = dict(character='characters', background='backgrounds', prop='props')[kind]
        relative = f'{directory}/{identifier}.kilsat'
        data = encoded(project)
        (destination / directory).mkdir(exist_ok=True)
        (destination / relative).write_bytes(data)
        manifest['items'].append(dict(id=identifier, name=name, kind=kind, project=relative,
                                     transparent=transparent, export_width=width, export_height=height,
                                     project_sha256=sha(data)))
    for actor, character in cast.items():
        identifier = actor.lower()
        metadata = next(item for item in CAST if item['id'] == actor)
        shot = scene('hahmo', [actor], [actor], '#f4f1e9', scale=1.8, width=768, height=1024)
        # Still exports share an unzoomed camera so transparent padding is predictable.
        shot['composition']['camera'] = [dict(at=0,x=0,y=0,zoom=1)]
        add(identifier, metadata['name'], 'character',
            document(source(metadata['name'], {actor:character}, [shot],768,1024), []), True,768,1024)
    for identifier, backdrop in BACKGROUNDS.items():
        shot = scene('tausta', [], [], backdrop['color'])
        shot['composition']['camera'] = [dict(at=0,x=0,y=0,zoom=1)]
        add(identifier,backdrop['name'],'background',
            document(source(backdrop['name'],{},[shot]),place_layers(backdrop['layers'],'tausta','tausta')),
            False,1920,1080)
    for identifier, prop in PROPS.items():
        if identifier in catalog:
            add(identifier, catalog[identifier]['name'], 'prop', prop_project(catalog[identifier]), True, 512, 512)
            continue
        shot = scene('esine', [], [], '#f4f1e9',width=512,height=512)
        shot['composition']['camera'] = [dict(at=0,x=0,y=0,zoom=1)]
        factor = min(440/prop['width'],440/prop['height'])
        layers = place_layers(prop['layers'],'esine','esine',
                              x=(512-prop['width']*factor)/2,y=(512-prop['height']*factor)/2,scale=factor)
        add(identifier,prop['name'],'prop',document(source(prop['name'],{},[shot],512,512),layers),True,512,512)
    for identifier, item in catalog.items():
        if identifier not in PROPS:
            add(identifier, item['name'], 'prop', prop_project(item), True, 512, 512)
    (destination/'manifest.json').write_bytes(encoded(manifest))
    return manifest


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,default=DEFAULT)
    args = parser.parse_args()
    result = generate(args.output)
    print(json.dumps(dict(items=len(result['items']),source_bytes=result['starter_source_bytes'])))
