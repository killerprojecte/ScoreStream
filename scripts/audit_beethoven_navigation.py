#!/usr/bin/env python3
"""Read-only navigation audit of four pinned original Beethoven MusicXML payloads.

This is corpus evidence generation, NOT the application's MusicXML importer.
No source XML is rewritten and no note/pitch/performance data is synthesized.
"""
import argparse
import hashlib
import json
from pathlib import Path
import xml.etree.ElementTree as ET

HASHES = {
    1: "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
    2: "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
    3: "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
    4: "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
}
NAVIGATION_SOUND = {"dacapo", "dalsegno", "segno", "tocoda", "coda", "fine", "forward-repeat", "time-only"}


def audit(inputs):
    result = {"source_tree": "8f677853c45b74ca0afcd79677e5379c06c82354", "kind": "read_only_navigation_evidence_not_an_import", "movements": []}
    for number, expected in HASHES.items():
        path = inputs / f"Beethoven_Op.67_{number}.musicxml"
        data = path.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        if digest != expected:
            raise ValueError(f"{path.name}: bytes differ from pinned original")
        root = ET.fromstring(data)
        parts = root.findall("part")
        labels = [m.attrib["number"] for m in parts[0].findall("measure")]
        if any([m.attrib["number"] for m in p.findall("measure")] != labels for p in parts):
            raise ValueError(f"{path.name}: part measure labels diverge")
        repeat_marks, ending_marks, attacca, sound_navigation = {}, {}, [], []
        for part in parts:
            pid = part.attrib["id"]
            for index, measure in enumerate(part.findall("measure")):
                for barline in measure.findall("barline"):
                    side = barline.get("location", "right")
                    if side not in ("left", "right"):
                        raise ValueError("Middle barline outside this audit profile")
                    boundary = index + (side == "right")
                    for repeat in barline.findall("repeat"):
                        key = (boundary, tuple(sorted(repeat.attrib.items())))
                        row = repeat_marks.setdefault(key, {"boundary": boundary, "attributes": dict(repeat.attrib), "parts": []})
                        row["parts"].append(pid)
                    for ending in barline.findall("ending"):
                        key = (boundary, ending.get("number"), ending.get("type"))
                        row = ending_marks.setdefault(key, {"boundary": boundary, "number": ending.get("number"), "phase": ending.get("type"), "text": ending.text or "", "parts": []})
                        row["parts"].append(pid)
                for sound in measure.findall(".//sound"):
                    if NAVIGATION_SOUND.intersection(sound.attrib):
                        sound_navigation.append({"part": pid, "measure_index": index, "attributes": dict(sound.attrib)})
                for words in measure.findall(".//direction-type/words"):
                    if (words.text or "").strip().lower() == "attacca":
                        attacca.append({"part": pid, "measure_index": index, "text": words.text})
        result["movements"].append({"id": f"beethoven-67-{number}", "source": path.name, "sha256": digest, "part_count": len(parts), "measure_labels": labels, "repeat_marks": sorted(repeat_marks.values(), key=lambda x: (x["boundary"], x["attributes"]["direction"])), "ending_marks": sorted(ending_marks.values(), key=lambda x: (x["boundary"], x["number"], x["phase"])), "sound_navigation": sound_navigation, "attacca": attacca})
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.write_text(json.dumps(audit(args.inputs), ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
