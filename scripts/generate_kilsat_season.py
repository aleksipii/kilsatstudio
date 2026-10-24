#!/usr/bin/env python3
"""Deterministically author the original season projects; never invent speech/media."""
from __future__ import annotations
import argparse
import copy
import hashlib
import json
from pathlib import Path
import struct
import zlib

REPO = Path(__file__).resolve().parents[1]
SEASON = REPO / 'examples/kilsat-season'
WIDTH, HEIGHT, FPS = 1920, 1080, 24


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encode(value) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + '\n').encode()


# Original geometric five-by-seven lettering, solely for the illustrative prop.
FONT = {
'A':['01110','10001','10001','11111','10001','10001','10001'],
'D':['11110','10001','10001','10001','10001','10001','11110'],
'E':['11111','10000','10000','11110','10000','10000','11111'],
'I':['11111','00100','00100','00100','00100','00100','11111'],
'J':['00111','00010','00010','00010','10010','10010','01100'],
'K':['10001','10010','10100','11000','10100','10010','10001'],
'L':['10000','10000','10000','10000','10000','10000','11111'],
'M':['10001','11011','10101','10101','10001','10001','10001'],
'N':['10001','11001','10101','10011','10001','10001','10001'],
'O':['01110','10001','10001','10001','10001','10001','01110'],
'P':['11110','10001','10001','11110','10000','10000','10000'],
'R':['11110','10001','10001','11110','10100','10010','10001'],
'S':['01111','10000','10000','01110','00001','00001','11110'],
'T':['11111','00100','00100','00100','00100','00100','00100'],
'U':['10001','10001','10001','10001','10001','10001','01110'],
'V':['10001','10001','10001','10001','10001','01010','00100'],
'Y':['10001','10001','01010','00100','00100','00100','00100'],
'0':['01110','10001','10011','10101','11001','10001','01110'],
'1':['00100','01100','00100','00100','00100','00100','01110'],
'2':['01110','10001','00001','00010','00100','01000','11111'],
'3':['11110','00001','00001','01110','00001','00001','11110'],
'4':['00010','00110','01010','10010','11111','00010','00010'],
'5':['11111','10000','10000','11110','00001','00001','11110'],
'6':['01110','10000','10000','11110','10001','10001','01110'],
'7':['11111','00001','00010','00100','01000','01000','01000'],
'8':['01110','10001','10001','01110','10001','10001','01110'],
'9':['01110','10001','10001','01111','00001','00001','01110'],
' ':['00000']*7, '-':['00000','00000','00000','11111','00000','00000','00000'],
}


def example_screen() -> bytes:
    side=512
    pixels=bytearray([229,237,233,255] * side * side)
    def rect(x,y,w,h,color):
        color=bytes(color)
        for yy in range(max(0,y),min(side,y+h)):
            lo=(yy*side+max(0,x))*4; hi=(yy*side+min(side,x+w))*4
            pixels[lo:hi]=color*((hi-lo)//4)
    def text(value,x,y,scale,color):
        for c in value:
            for yy,row in enumerate(FONT[c]):
                for xx,bit in enumerate(row):
                    if bit=='1': rect(x+xx*scale,y+yy*scale,scale,scale,color)
            x+=6*scale
    dark=(25,42,38,255); green=(24,112,87,255); white=(255,255,255,255)
    rect(34,12,444,488,dark);rect(48,40,416,430,white)
    rect(48,40,416,95,green);text('KILSAT',152,63,6,white)
    text('AJOPAIVAKIRJA',80,158,4,dark)
    for y,label in [(220,'TYOMATKA'),(285,'ASIAKAS'),(350,'KOTI')]:
        rect(68,y-9,376,52,(237,244,240,255));text(label,80,y,3,dark)
    rect(48,408,416,62,(251,214,81,255));text('ESIMERKKI',148,424,4,dark)
    rows=b''.join(b'\x00'+pixels[y*side*4:(y+1)*side*4] for y in range(side))
    def chunk(kind,data):
        return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data)&0xffffffff)
    return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',side,side,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(rows))+chunk(b'IEND',b'')


def outline(width=2.0,mode='behind'):
    return {'width':width,'mode':mode,'color':[27,28,36,255]}


def polygon(name,joint,points,color,smooth=2,width=2):
    return dict(id=name,joint=joint,points=points,color=color,smooth=smooth,outline=outline(width))


def ellipse(name,joint,cx,cy,rx,ry,color,width=2):
    return dict(id=name,joint=joint,ellipse=dict(cx=cx,cy=cy,rx=rx,ry=ry),color=color,outline=outline(width))


def character(name):
    aino=name=='AINO'
    skin=[233,184,151,255] if aino else [202,149,118,255]
    shirt=[42,136,132,255] if aino else [137,83,58,255]
    hair=[74,45,43,255] if aino else [61,56,63,255]
    parts=[
        polygon('leg_l','root',[[-30,15],[-4,15],[-7,116],[-29,116]],[52,66,81,255]),
        polygon('leg_r','root',[[4,15],[30,15],[29,116],[7,116]],[52,66,81,255]),
        ellipse('shoe_l','root',-20,120,20,9,[38,41,47,255]),
        ellipse('shoe_r','root',20,120,20,9,[38,41,47,255]),
        polygon('torso','torso',[[-30,-116],[30,-116],[45,-92],[43,22],[-43,22],[-45,-92]],shirt,3,2.4),
        polygon('collar','torso',[[-16,-116],[16,-116],[0,-92]],[231,224,208,255],2,1.3),
        polygon('sleeve_l','arm_l',[[-11,-3],[11,-3],[10,66],[-10,66]],shirt,3),
        ellipse('hand_l','arm_l',0,72,10,12,skin,1.8),
        polygon('sleeve_r','arm_r',[[-11,-3],[11,-3],[10,66],[-10,66]],shirt,3),
        ellipse('hand_r','arm_r',0,72,10,12,skin,1.8),
    ]
    if aino:
        parts.append(ellipse('hair_back','head',0,-25,40,46,hair,2.2))
    parts.append(ellipse('face','head',0,-24,31 if aino else 34,37 if aino else 40,skin,2.2))
    parts.append(polygon('hair_top','head',[[-32,-39],[-28,-57],[-12,-69],[12,-65],[29,-51],[32,-37],[12,-50],[-7,-47],[-20,-35]],hair,3,1.8))
    for side,cx in [('l',-13),('r',13)]:
        eye=ellipse('eye_'+side,'head',cx,-27,8.5,10,[249,250,249,255],1.2)
        eye['variants']={'blink':{'ellipse':dict(cx=cx,cy=-27,rx=8.5,ry=1.1),'color':skin}}
        parts.append(eye)
        pupil=ellipse('pupil_'+side,'head',cx,-26,3.6,4.6,[36,40,47,255],0)
        pupil['variants']={'blink':{'color':[0,0,0,0]}};parts.append(pupil)
        x0,x1=cx-9,cx+9
        default=[[x0,-44],[x1,-44 if aino else -39],[x1,-40 if aino else -35],[x0,-40]]
        brow=polygon('brow_'+side,'head',default,hair,2,0)
        brow['variants']={
            'raised':{'points':[[x0,-52],[x1,-52],[x1,-48],[x0,-48]]},
            'worried':{'points':[[x0,-44],[x1,-50],[x1,-46],[x0,-40]]},
            'angry':{'points':[[x0,-47],[x1,-39],[x1,-35],[x0,-43]] if side=='l' else [[x0,-39],[x1,-47],[x1,-43],[x0,-35]]},
        };parts.append(brow)
    nose=polygon('nose','head',[[0,-24],[-4,-9],[5,-9]],skin,2,1)
    parts.append(nose)
    mouth=ellipse('mouth','head',0,3,10,1.7,[86,36,43,255],1.1)
    shapes={'closed':(11,1.4),'teeth_lip':(9,3),'tongue_teeth':(9,5),'tongue_up':(11,6),'sibilant':(12,3),'sh':(9,6),'r':(9,6),'a':(11,11),'e':(13,5),'i':(14,3),'o':(7,10),'u':(5,7)}
    mouth['variants']={k:{'ellipse':dict(cx=0,cy=5,rx=rx,ry=ry)} for k,(rx,ry) in shapes.items()}
    mouth['variants']['teeth_lip']['color']=[247,239,225,255]
    parts.append(mouth)
    if not aino:
        parts.append(polygon('tie','torso',[[-7,-95],[0,-104],[7,-95],[12,-43],[0,-30],[-12,-43]],[58,43,56,255],2,1.1))
    rig={'version':1,'id':'aino_original' if aino else 'rane_original',
         'joints':[{'id':'root','parent':None},{'id':'torso','parent':'root'},
                   {'id':'head','parent':'torso','y':-144},
                   {'id':'arm_l','parent':'torso','x':-43,'y':-103},
                   {'id':'arm_r','parent':'torso','x':43,'y':-103}],
         'parts':parts,'visemes':{'part':'mouth','map':{k:k for k in shapes}},
         'expressions':{'happy':[{'part':'brow_l','variant':'raised'},{'part':'brow_r','variant':'raised'}],
                        'sad':[{'part':'brow_l','variant':'worried'},{'part':'brow_r','variant':'worried'}],
                        'surprised':[{'part':'brow_l','variant':'raised'},{'part':'brow_r','variant':'raised'}],
                        'angry':[{'part':'brow_l','variant':'angry'},{'part':'brow_r','variant':'angry'}]},
         'eyes':{'pupils':['pupil_l','pupil_r'],'travel':3.0,
                 'blink':{'parts':['eye_l','eye_r','pupil_l','pupil_r'],'variant':'blink','interval':3.4 if aino else 4.1,'duration':0.13}}}
    return {'asset':'characters/'+name.lower()+'-original','default_pose':'idle','rig':rig,
            'voice':{'provider':'piper','preset':'piper_harri','language':'fi-fi','speed':1.0 if aino else 0.93}}


def key(at,**values):
    return dict(at=round(at,6),**values)


def layer(scene,name,content,plane='background',opacity=1.0):
    return {'id':scene+'_'+name,'name':name,'scene_id':scene,'visible':True,'plane':plane,'opacity':opacity,'attachment':None,'clip':None,'content':content}


def rectangle(scene,name,x,y,w,h,color,plane='background'):
    return layer(scene,name,{'type':'rectangle','rect':dict(x=x,y=y,width=w,height=h),'color':dict(zip(['r','g','b','a'],color))},plane)


SET_COLORS={'parkkipaikka':'#dce7e4','toimisto':'#e6e1d6','kahvila':'#eddfd4'}
def scenery(scene,setting):
    colors={'parkkipaikka':([166,180,178,255],[105,123,126,255]),'toimisto':([184,174,158,255],[95,119,124,255]),'kahvila':([187,143,113,255],[142,106,88,255])}
    floor,trim=colors[setting]
    items=[rectangle(scene,'floor',0,880,1920,200,floor),rectangle(scene,'skirting',0,872,1920,10,trim)]
    if setting=='parkkipaikka':
        for i in range(4):items.append(rectangle(scene,f'parking_line_{i}',i*520+40,940,220,9,[242,238,211,255]))
        items.append(rectangle(scene,'wall',0,130,1920,100,[176,196,189,255]))
    elif setting=='toimisto':
        items.extend([rectangle(scene,'window_frame',770,160,380,280,[75,95,104,255]),rectangle(scene,'window',785,175,350,250,[166,201,205,255]),rectangle(scene,'window_cross_v',955,175,12,250,[75,95,104,255]),rectangle(scene,'window_cross_h',785,290,350,12,[75,95,104,255])])
    else:
        items.extend([rectangle(scene,'counter',20,550,310,330,[117,83,67,255]),rectangle(scene,'counter_top',15,535,320,24,[228,207,171,255]),rectangle(scene,'poster',1580,210,180,230,[61,112,104,255]),rectangle(scene,'poster_inset',1598,230,144,190,[239,218,183,255])])
    return items


def composition(scene,setting,speaker=None,beat=0,duration=4):
    # Actor root offsets are relative to the evaluator's (width/2,height*.7).
    positions={'AINO':-350,'RANE':350}
    camera_x=0 if beat in (0,4) else (-170 if speaker=='AINO' else 190)
    zoom=1.0 if beat==0 else (1.08 if beat==4 else 1.16+(beat%3)*0.08)
    if beat==7: camera_x=350;zoom=1.38
    gaze={actor:[dict(at=0,target='RANE' if actor=='AINO' else 'AINO')] for actor in positions}
    joints={}
    for actor in positions:
        sign=1 if actor=='AINO' else -1
        emph=actor==speaker
        joints[actor]={'head':[key(0),key(min(0.28,duration/4),rotation=sign*(0.045 if emph else -0.025)),key(min(0.75,duration/2),rotation=sign*(-0.025 if emph else 0.035)),key(duration,rotation=0)],
                       'arm_r' if actor=='RANE' else 'arm_l':[key(0),key(min(0.45,duration/3),rotation=sign*(0.19 if emph else 0.03)),key(duration,rotation=0)]}
    return {'background_color':SET_COLORS[setting], 'camera':[dict(at=0,x=camera_x,y=-75,zoom=zoom),dict(at=duration,x=camera_x,y=-75,zoom=zoom+0.018)],
            'layers':{'AINO':dict(z=0,visible=True),'RANE':dict(z=1,visible=True)},
            'actor_tracks':{actor:[key(0,x=x,y=44,scale_x=2.2,scale_y=2.2)] for actor,x in positions.items()},
            'joint_tracks':joints,'gaze_tracks':gaze}


def episode_project(story):
    number=story['number'];scenes=[];art=[]
    for index,text in enumerate(story['turns']):
        speaker='AINO' if index%2==0 else 'RANE'
        identifier=f'e{number:02d}_line{index+1:02d}'
        duration=round(min(5.4,max([4.0,4.1,3.9,4.1,4.3,4.0,4.2,4.5][index],len(text)/18.0+0.5)),3)
        emotion=('surprised' if index in (2,4) else 'happy') if speaker=='AINO' else ('angry' if index in (5,7) else 'happy')
        events=[dict(type='emotion',at=0,actor=speaker,value=emotion),dict(type='speak',at=0.35,actor=speaker,text=text,emotion=emotion,audio=None)]
        scenes.append({'id':identifier,'duration':duration,'background':story['set'],'composition':composition(identifier,story['set'],speaker,index,duration),'events':events})
        art.extend(scenery(identifier,story['set']))
        if index==4:
            insert=f'e{number:02d}_kilsat';scenes.append({'id':insert,'duration':3.0,'background':'illustrative Kilsat screen, not actual app capture','composition':{'background_color':'#213e35','camera':[dict(at=0,x=0,y=0,zoom=1)],'layers':{'AINO':dict(visible=False),'RANE':dict(visible=False)}},'events':[]})
            art.append(layer(insert,'illustrative_screen',{'type':'image','asset_id':'kilsat_example','rect':dict(x=600,y=180,width=720,height=720)},'foreground'))
    reaction=f'e{number:02d}_reaction'
    comp=composition(reaction,story['set'],None,8,1.8)
    comp['camera']=[dict(at=0,x=0,y=-75,zoom=1.14)]
    comp['joint_tracks']['AINO']['head']=[key(0),key(0.55,rotation=-0.065),key(1.8)]
    comp['joint_tracks']['RANE']['head']=[key(0),key(0.7,rotation=0.075),key(1.8)]
    comp['gaze_tracks']['AINO']=[dict(at=0,target='RANE'),dict(at=0.6,x=960,y=50)]
    scenes.append({'id':reaction,'duration':1.8,'background':story['set'],'composition':comp,'events':[dict(type='emotion',at=0,actor='AINO',value='surprised'),dict(type='emotion',at=0,actor='RANE',value='angry')]})
    art.extend(scenery(reaction,story['set']))
    source={'project':{'title':f'Kilsat {number:02d} – {story["title"]}','width':WIDTH,'height':HEIGHT,'fps':FPS,
                       'captions':{'enabled':True,'font_size':54,'speaker_fill':{'AINO':'#ffdf7c','RANE':'#ffffff'},'outline_width':4,'bottom_margin':0.035,'max_width':0.9,'line_height':1.08,'uppercase':False,'background':'#17232b'}},
            'characters':{name:character(name) for name in ['AINO','RANE']},'scenes':scenes}
    return {'format_version':2,'source':json.dumps(source,ensure_ascii=False,separators=(',',':')),'assets':[{'id':'kilsat_example','path':'assets/kilsat-example.png','kind':'image'}],'art':{'layers':art}}


def generate(destination=SEASON, stories_path=SEASON/'stories.json'):
    destination=Path(destination);catalog=json.loads(Path(stories_path).read_text())
    episodes=catalog['episodes']
    if [e['number'] for e in episodes]!=list(range(1,32)):raise ValueError('season requires exactly ordered episodes1..31')
    if len({e['title'] for e in episodes})!=31:raise ValueError('titles must be unique')
    destination.mkdir(parents=True,exist_ok=True);(destination/'assets').mkdir(exist_ok=True)
    image=example_screen();(destination/'assets/kilsat-example.png').write_bytes(image)
    manifest={'schema_version':1,'title':catalog['title'],'language':'fi-fi','authorship':catalog['authorship'],
              'profile':dict(width=WIDTH,height=HEIGHT,fps=FPS,min_episode_seconds=30,max_episode_seconds=60),
              'publishable':False,'acceptance':{'audio_rights_reviewed':False,'actual_kilsat_capture':False,'artistic_review':False,'real_audio_lipsync_review':False,'physical_mac_300s':False},
              'asset_sha256':{'assets/kilsat-example.png':sha(image)},'episodes':[],'youtube_groups':[]}
    readable=['# Kilsat – Rane tietää paremmin','', 'Uusi alkuperäinen kausi. Ajat ovat luonnoksia; oikea ääni määrää lopullisen ajoituksen.','']
    for story in episodes:
        if len(story['turns'])!=8 or any(any(c.isdigit() for c in text) for text in story['turns']):raise ValueError('eight Finnish spoken-word turns required')
        document=episode_project(story);source=json.loads(document['source']);duration=round(sum(s['duration'] for s in source['scenes']),6)
        if not 30<=duration<=60:raise ValueError('draft episode duration out of bounds')
        name=f'ep{story["number"]:02d}.kilsat';data=encode(document);(destination/name).write_bytes(data)
        manifest['episodes'].append({'number':story['number'],'title':story['title'],'arc':story['arc'],'set':story['set'],'project':name,'project_sha256':sha(data),'source_sha256':sha(document['source'].encode()),'draft_duration_seconds':duration,'turn_count':8,'scene_count':len(source['scenes']),'audio_status':'not_synthesized','video_status':'not_rendered'})
        readable.extend([f'## {story["number"]:02d}. {story["title"]}',f'{story["arc"]} · {story["set"]} · ajoitusluonnos {duration:.2f} s',''])
        readable.extend(f'{"AINO" if i%2==0 else "RANE"}: {text}' for i,text in enumerate(story['turns']))
        readable.extend(['','Kuva: kolmen sekunnin ESIMERKKI-näkymä viidennen vuoron jälkeen. Lopussa hiljainen reaktio.',''])
    for index,first in enumerate(range(1,32,5),1):manifest['youtube_groups'].append({'number':index,'episodes':list(range(first,min(first+5,32))),'title':f'Kilsat – kooste {index}','status':'not_rendered','publishable':False})
    (destination/'manifest.json').write_bytes(encode(manifest))
    (destination/'KAUSI.md').write_text('\n'.join(readable)+'\n')
    (destination/'assets/ORIGINAL-ASSETS.md').write_text('# Original season assets\n\nThe Aino/Rane rigs and geometric sets are original project-authored shapes.\nThe generated Kilsat screen is an original illustrative prop marked ESIMERKKI,\nnot a real screen capture or a claim about actual product features. It must be\nreplaced/reviewed against an approved real capture before marketing publication.\nNo third-party images or downloaded model weights are included.\n')
    return manifest


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,default=SEASON);parser.add_argument('--stories',type=Path,default=SEASON/'stories.json')
    args=parser.parse_args();m=generate(args.output,args.stories);print(json.dumps({'episodes':len(m['episodes']),'groups':len(m['youtube_groups']),'publishable':False}))

if __name__=='__main__':main()
