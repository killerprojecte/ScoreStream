#!/usr/bin/env python3
"""Hash-pinned timing/pitch/rest/clef family projection, NOT whole-score acceptance.
Retains source note pitch/rest/chord/duration/voice/staff/written rhythm and exact
backup/forward sequence. Omits unrelated engraving, ties, directions, and notation.
Inherited divisions/meter/initial clef are copied, not inferred.
"""
import copy, hashlib, json, pathlib, sys, zipfile
import xml.etree.ElementTree as E
from fractions import Fraction as Q
HASHES=['5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b','c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a','788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98','cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298']
rows=[];counts=[]
for movement in range(1,5):
 root=pathlib.Path(sys.argv[1]); data=(root/f'Beethoven_Op.67_{movement}.musicxml').read_bytes()
 assert hashlib.sha256(data).hexdigest()==HASHES[movement-1]
 with zipfile.ZipFile(root/f'Beethoven_Op.67_{movement}.mxl') as z:
  container=E.fromstring(z.read('META-INF/container.xml'))
  source=next(n.attrib['full-path'] for n in container.iter() if n.tag.endswith('rootfile'))
  assert z.read(source)==data
 before=len(rows)
 for part in E.fromstring(data).findall('part'):
  inherited={};div=None
  for ordinal,m in enumerate(part.findall('measure'),1):
   cur=Q(0); changes=[]; initial=copy.deepcopy(inherited); body=[]
   for child in m:
    if child.tag=='attributes':
     if cur:
      assert [n.tag for n in child]==['clef']
      changes.append({'offset':str(cur),'sign':child.findtext('clef/sign'),'line':int(child.findtext('clef/line'))})
      body.append(copy.deepcopy(child))
     else:
      for n in child:
       if n.tag in ['divisions','time','clef']: inherited[n.tag]=copy.deepcopy(n)
      initial=copy.deepcopy(inherited)
      div=Q(inherited['divisions'].text)
    elif child.tag in ['backup','forward']:
     cur+=Q(child.findtext('duration'))/div*(-1 if child.tag=='backup' else 1)
     body.append(copy.deepcopy(child))
    elif child.tag=='note':
     note=E.Element('note')
     for n in child:
      if n.tag in ['chord','pitch','rest','duration','voice','type','dot','time-modification','staff']:
       selected=copy.deepcopy(n)
       if selected.tag=='dot':selected.attrib.clear()
       note.append(selected)
     body.append(note)
     if child.find('chord') is None and child.find('grace') is None:cur+=Q(child.findtext('duration'))/div
   if not changes:continue
   score=E.Element('score-partwise',version='4.0');pl=E.SubElement(score,'part-list');sp=E.SubElement(pl,'score-part',id=part.attrib['id']);E.SubElement(sp,'part-name').text='Piano';ins=E.SubElement(sp,'score-instrument',id='piano');E.SubElement(ins,'instrument-name').text='piano'
   p=E.SubElement(score,'part',id=part.attrib['id']);mm=E.SubElement(p,'measure',id=f'm{ordinal}',number=m.attrib['number']);a=E.SubElement(mm,'attributes')
   for tag in ['divisions','time','clef']:
    if tag in initial:a.append(initial[tag])
   mm.extend(body)
   rows.append(dict(movement=movement,part=part.attrib['id'],ordinal=ordinal,changes=changes,xml=E.tostring(score,encoding='unicode')))
 counts.append(len(rows)-before)
assert counts==[22,9,12,19],counts
print(json.dumps(rows))
