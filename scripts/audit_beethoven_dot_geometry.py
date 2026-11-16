#!/usr/bin/env python3
"""Inventory direct augmentation-dot coordinates in byte-pinned originals, read-only.

Evidence of the field family only, not a claim that whole-score conversion succeeds.
"""
import argparse
from collections import Counter
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import re
import subprocess
import xml.etree.ElementTree as ET
from audit_beethoven_navigation import HASHES


def audit(inputs, cli=None):
    result = []
    for movement, expected in HASHES.items():
        data = (inputs / f"Beethoven_Op.67_{movement}.musicxml").read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        if digest != expected:
            raise ValueError(f"Movement {movement}: bytes differ from pinned original")
        root = ET.fromstring(data)
        counts, samples, styled, total = Counter(), [], 0, 0
        for part in root.findall("part"):
            for measure in part.findall("measure"):
                for note_index, note in enumerate(measure.findall("note")):
                    dots = note.findall("dot")
                    total += len(dots)
                    for index, dot in enumerate(dots):
                        counts.update(dot.attrib.keys())
                        if not dot.attrib:
                            continue
                        styled += 1
                        if set(dot.attrib) - {"default-x", "default-y"}:
                            raise ValueError("Other dot attribute outside this bridge")
                        for value in dot.attrib.values():
                            if not re.fullmatch(r"[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)", value):
                                raise ValueError("Not an exact XML decimal")
                        if len(samples) < 3:
                            samples.append({"part": part.get("id"), "measure": measure.get("number"), "note_index": note_index, "dot_index": index, "dot_count": len(dots), "coordinates": dot.attrib, "exact_tenths": {k: str(Fraction(v)) for k, v in dot.attrib.items()}})
        entry = {"movement": movement, "sha256": digest, "dots": total, "styled_dots": styled, "attribute_counts": dict(counts), "samples": samples}
        if cli is not None:
            run = subprocess.run([str(cli), "convert", "musicxml", "sfs", str(inputs / f"Beethoven_Op.67_{movement}.musicxml")], capture_output=True, check=False)
            report = json.loads(run.stderr)
            direct = [d for d in report["diagnostics"] if re.search(r"/note\[\d+\]/dot\[\d+\]/@", d["path"])]
            if len(direct) != len(counts) or any(d["effect"] != "preserved" or d["feature"] != "geometry.preserved_coordinates" for d in direct):
                raise AssertionError(f"Movement {movement}: direct dot diagnostics were not exactly the two preserved-field audits")
            for name, count in counts.items():
                matching = [d for d in direct if d["path"].endswith("/@" + name)]
                if len(matching) != 1 or not matching[0]["message"].startswith(f"Validated {count} authored note/dot/@{name} values"):
                    raise AssertionError(f"Movement {movement}: dot audit count mismatch")
            entry["converter_dot_audits"] = direct
            entry["whole_score_exit_code"] = run.returncode
            entry["whole_score_output_bytes"] = len(run.stdout)
            entry["whole_score_diagnostics_truncated"] = any(d["feature"] == "diagnostics.truncated" for d in report["diagnostics"])
        result.append(entry)
    return {"kind": "read_only_pinned_dot_field_inventory_not_whole_score_acceptance", "movements": result}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("--cli", type=Path, help="also verify exact direct-dot first-pass audits using this real converter")
    args = parser.parse_args()
    print(json.dumps(audit(args.inputs, args.cli.resolve() if args.cli else None), ensure_ascii=False, indent=2))
