#!/usr/bin/env python3
"""Read-only exact dynamic locations and collisions; not an import or playback policy."""
import argparse
from collections import Counter, defaultdict
from fractions import Fraction as Q
import hashlib
import json
from pathlib import Path
import xml.etree.ElementTree as ET
from audit_beethoven_navigation import HASHES


def audit(directory):
    movements=[]
    for movement,digest in HASHES.items():
        data=(directory/f'Beethoven_Op.67_{movement}.musicxml').read_bytes()
        assert hashlib.sha256(data).hexdigest()==digest
        rows=[]; exceptions=[]; ends={}
        for p in ET.fromstring(data).findall('part'):
            pid=p.get('id'); div=None; nominal=None; absolute=Q(0)
            measures=p.findall('measure')
            for ordinal,m in enumerate(measures,1):
                cursor=Q(0); extent=Q(0); previous=Q(0); pending=[]
                for order,n in enumerate(m):
                    if n.tag=='attributes':
                        if n.find('divisions') is not None: div=Q(n.findtext('divisions'))
                        t=n.find('time')
                        if t is not None: nominal=sum(Q(b) for b in t.findtext('beats').split('+'))*4/Q(t.findtext('beat-type'))
                    elif n.tag=='note' and n.find('grace') is None:
                        duration=Q(n.findtext('duration'))/div
                        if n.find('chord') is None: previous=cursor; cursor+=duration
                        extent=max(extent,cursor,previous+duration)
                    elif n.tag in ('backup','forward'):
                        amount=Q(n.findtext('duration'))/div
                        cursor+= -amount if n.tag=='backup' else amount
                        extent=max(extent,cursor)
                    elif n.tag in ('direction','sound'):
                        d=n.find('direction-type/dynamics'); sound=n if n.tag=='sound' else n.find('sound')
                        if d is None and (sound is None or sound.get('dynamics') is None): continue
                        o=n.find('offset') if n.tag=='direction' else None
                        nested=sound.find('offset') if sound is not None else None
                        offset=Q(o.text)/div if o is not None else Q(0)
                        visual=cursor+offset
                        sounding=cursor+offset if o is not None and o.get('sound','no')=='yes' else cursor
                        if nested is not None: sounding=cursor+Q(nested.text)/div
                        pending.append(dict(part=pid,ordinal=ordinal,label=m.get('number'),child_order=order,staff=n.findtext('staff'),cursor=str(cursor),divisions=str(div),direction_offset=None if o is None else o.text,offset_sound=None if o is None else o.get('sound'),nested_sound_offset=None if nested is None else nested.text,visual=str(visual) if d is not None else None,sounding=str(sounding) if sound is not None and sound.get('dynamics') is not None else None,percent=None if sound is None else sound.get('dynamics'),symbol=None if d is None else [c.tag for c in d],geometry=None if d is None else d.attrib))
                length=extent if m.get('implicit')=='yes' or nominal is None else nominal
                if extent>length:
                    exceptions.append(dict(part=pid,ordinal=ordinal,label=m.get('number'),nominal=str(length),encoded_extent=str(extent)))
                    # Inventory only: describe actual encoded intervals, never resize an imported score.
                    length=extent
                for row in pending:
                    row.update(length=str(length),measure_start=str(absolute),part_final=ordinal==len(measures))
                    for lane in ('visual','sounding'):
                        at=None if row[lane] is None else Q(row[lane])
                        row[lane+'_class']=None if at is None else 'before' if at<0 else 'start' if at==0 else 'inside' if at<length else 'end' if at==length else 'after'
                        row[lane+'_absolute']=None if at is None else str(absolute+at)
                    rows.append(row)
                absolute+=length
            ends[pid]=str(absolute)
        sound=defaultdict(list)
        for row in rows:
            if row['sounding'] is not None: sound[(row['part'],row['staff'],row['sounding_absolute'])].append(row)
        collisions=[dict(part=k[0],staff=k[1],at=k[2],rows=v) for k,v in sound.items() if len(v)>1]
        boundary=[r for r in rows if r['visual_class'] in ('before','end','after') or r['sounding_class'] in ('before','end','after')]
        movements.append(dict(movement=movement,sha256=digest,counts=dict(Counter((lane+':'+str(r[lane+'_class'])) for r in rows for lane in ('visual','sounding') if r[lane] is not None)),part_ends=ends,encoded_overfull_inventory=exceptions,boundary_rows=boundary,equal_sound_positions=collisions))
    return dict(kind='read_only_exact_source_inventory_not_import_or_precedence_policy',movements=movements)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('directory',type=Path);parser.add_argument('--check',type=Path)
    args=parser.parse_args();result=audit(args.directory)
    if args.check:
        assert result==json.loads(args.check.read_text());print('Four pinned original dynamic boundary inventories match')
    else: print(json.dumps(result,ensure_ascii=False,indent=2))
