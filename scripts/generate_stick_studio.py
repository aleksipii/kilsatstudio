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


def source(title, cast, scenes, width=1920, height=1080):
    data = dict(project=dict(title=title, width=width, height=height, fps=24),
                characters=cast, scenes=scenes)
    if cast:
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


def main_project(cast):
    shots = [
        ('vuoron_alku', 'toimisto', ['NOUTAJA', 'VEIKKO'], 'vilkutus', 'ajopaivakirja'),
        ('pomo_kutsuu', 'toimisto', ['PASMA', 'KIIRE'], 'osoitus', 'toimistotuoli'),
        ('pysakkipaniikki', 'pysakki', ['VEIKKO', 'KIIRE'], 'reaktio', 'sateenvarjo'),
        ('kahvitauko', 'kahvila', ['NOUTAJA', 'SISU'], 'odotus', 'kahvikuppi'),
        ('keksinto', 'hautausmaa', ['KIPINA', 'NOUTAJA'], 'vilkutus', 'tyokaluavain'),
        ('mummo_paataa', 'kahvila', ['PASMA', 'SISU'], 'reaktio', 'vuoronumerolaite'),
    ]
    scenes, layers = [], []
    for identifier, setting, active, clip_name, prop in shots:
        backdrop = BACKGROUNDS[setting]
        shot = scene(identifier, cast, active, backdrop['color'])
        # Rig clips are reusable source data; copied joint tracks use the native evaluator.
        for actor in active:
            shot['composition']['joint_tracks'][actor].update(copy.deepcopy(clips()[clip_name]['joint_tracks']))
        shot['events'] = [dict(type='emotion', actor=active[0], at=0,
                               value='surprised' if clip_name == 'reaktio' else 'happy')]
        scenes.append(shot)
        layers.extend(place_layers(backdrop['layers'], identifier, 'tausta'))
        if setting in ('hautausmaa', 'pysakki'):
            # Pale pool behind dark stick limbs keeps the silhouette readable at night.
            layers.append(dict(id=identifier+'_valokeila', scene_id=identifier, name='Vaalea näyttämö',
                               visible=True, plane='background', opacity=0.7,
                               content=dict(type='ellipse', rect=dict(x=300,y=260,width=1300,height=720),
                                            color=dict(r=193,g=209,b=200,a=255))))
        item = PROPS[prop]
        factor = min(180 / item['width'], 170 / item['height'])
        layers.extend(place_layers(item['layers'], identifier, 'esine',
                                   x=1630, y=960-item['height']*factor, scale=factor))
    return document(source('Tikku-ukkostudio – kuusi kohtaamista', cast, scenes), layers)


def generate(destination=DEFAULT):
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    cast = characters()
    starter = main_project(cast)
    starter_bytes = encoded(starter)
    if len(starter['source'].encode()) > 100_000:
        raise ValueError('compact starter source must remain below 100 KB')
    (destination / 'stick-studio.kilsat').write_bytes(starter_bytes)
    manifest = dict(schema_version=1, title='Tikku-ukkostudio',
                    authorship='Original independently authored editable geometric artwork; no third-party artwork or model assets.',
                    cast=CAST, joint_ids=JOINT_IDS, starter_project='stick-studio.kilsat',
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
        shot = scene('esine', [], [], '#f4f1e9',width=512,height=512)
        shot['composition']['camera'] = [dict(at=0,x=0,y=0,zoom=1)]
        factor = min(440/prop['width'],440/prop['height'])
        layers = place_layers(prop['layers'],'esine','esine',
                              x=(512-prop['width']*factor)/2,y=(512-prop['height']*factor)/2,scale=factor)
        add(identifier,prop['name'],'prop',document(source(prop['name'],{},[shot],512,512),layers),True,512,512)
    (destination/'manifest.json').write_bytes(encoded(manifest))
    return manifest


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,default=DEFAULT)
    args = parser.parse_args()
    result = generate(args.output)
    print(json.dumps(dict(items=len(result['items']),source_bytes=result['starter_source_bytes'])))
