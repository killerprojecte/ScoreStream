#!/usr/bin/env python3
"""Read-only exact interval evidence, not an importer or a playable score rewrite."""
import argparse
from collections import Counter
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import xml.etree.ElementTree as ET

HASHES = {
    2: 'c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a',
    3: '788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98',
    4: 'cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298',
}
PATTERNS = {
    2: ('123', Counter({('0', '1', False): 11, ('0', '1/2', True): 5})),
    3: ('257', Counter({('0', '3', True): 12, ('1', '2', True): 6})),
    4: ('446', Counter({('0', '4', False): 18, ('2', '4', False): 1})),
}

def audit(directory):
    movements = []
    for number, (label, expected) in PATTERNS.items():
        data = (directory / f'Beethoven_Op.67_{number}.musicxml').read_bytes()
        if hashlib.sha256(data).hexdigest() != HASHES[number]:
            raise ValueError(f'Movement {number}: pinned original bytes differ')
        anchors = []
        for part in ET.fromstring(data).findall('part'):
            divisions = None
            for measure in part.findall('measure'):
                cursor = Fraction(0)
                last_onset = cursor
                for child in measure:
                    if child.tag == 'attributes' and child.find('divisions') is not None:
                        divisions = Fraction(child.findtext('divisions'))
                    elif child.tag in ('backup', 'forward'):
                        amount = Fraction(child.findtext('duration')) / divisions
                        cursor += -amount if child.tag == 'backup' else amount
                    elif child.tag == 'note':
                        duration = Fraction(child.findtext('duration', '0')) / divisions
                        chord = child.find('chord') is not None
                        start = last_onset if chord else cursor
                        if not chord:
                            last_onset = cursor
                            cursor += duration
                        sign = child.find('notations/fermata')
                        if measure.get('number') == label and sign is not None:
                            anchors.append(dict(part=part.get('id'), voice=child.findtext('voice', '1'),
                                staff=child.findtext('staff', '1'), start=str(start), end=str(start + duration),
                                rest=child.find('rest') is not None, shape=sign.text, attributes=sign.attrib))
        actual = Counter((a['start'], a['end'], a['rest']) for a in anchors)
        if actual != expected:
            raise ValueError(f'Movement {number}, measure {label}: {actual} != {expected}')
        movements.append(dict(movement=number, measure=label, sha256=HASHES[number], anchors=anchors))
    return dict(kind='read_only_original_interval_evidence_not_a_full_score_import', movements=movements)

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--check', type=Path, help='Compare the committed evidence without rewriting it')
    args = parser.parse_args()
    result = audit(args.directory)
    if args.check:
        if result != json.loads(args.check.read_text()):
            raise SystemExit('Committed evidence differs from pinned original audit')
        print('Three pinned originals verified; 53 anchors match committed exact interval evidence')
    else:
        print(json.dumps(result, indent=2) + '\n', end='')
