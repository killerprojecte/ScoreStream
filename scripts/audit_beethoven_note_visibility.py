#!/usr/bin/env python3
"""Read-only print-object inventory of the four byte-pinned original XML files."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import xml.etree.ElementTree as ET
from audit_beethoven_navigation import HASHES


def audit(inputs):
    result = []
    for movement, digest in HASHES.items():
        data = (inputs / f"Beethoven_Op.67_{movement}.musicxml").read_bytes()
        if hashlib.sha256(data).hexdigest() != digest:
            raise ValueError(f"Movement {movement}: bytes differ from pinned original")
        root = ET.fromstring(data)
        owners = Counter((n.tag, n.get("print-object")) for n in root.iter() if "print-object" in n.attrib)
        samples, rests = [], 0
        for part in root.findall("part"):
            for measure in part.findall("measure"):
                for index, note in enumerate(measure.findall("note")):
                    if "print-object" not in note.attrib:
                        continue
                    if note.get("print-object") != "no" or note.find("rest") is None or "print-spacing" in note.attrib:
                        raise ValueError("Original visibility inventory changed")
                    rests += 1
                    if len(samples) < 2:
                        samples.append({"part": part.get("id"), "measure": measure.get("number"), "note_index": index, "attributes": note.attrib, "duration": note.findtext("duration"), "type": note.findtext("type"), "dots": len(note.findall("dot"))})
        if rests != {1:73, 2:0, 3:7, 4:0}[movement] or set(owners) - {("note", "no")}:
            raise ValueError("Unexpected original visibility count/owner")
        result.append({"movement": movement, "sha256": digest, "hidden_rests": rests, "owners": [{"element": k[0], "value": k[1], "count": v} for k,v in owners.items()], "samples": samples})
    return {"kind": "read_only_field_inventory_not_whole_score_acceptance", "movements": result}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inputs", type=Path)
    print(json.dumps(audit(parser.parse_args().inputs), ensure_ascii=False, indent=2))
