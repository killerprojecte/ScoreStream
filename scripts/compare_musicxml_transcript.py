#!/usr/bin/env python3
"""Independent exact note-event comparison, not a renderer or audio oracle."""
import argparse
from collections import Counter, defaultdict
from fractions import Fraction
import json
from pathlib import Path
import xml.etree.ElementTree as ET


class TranscriptError(ValueError):
    pass


def transcript(path):
    root = ET.parse(path).getroot()
    if root.tag != "score-partwise":
        raise TranscriptError(f"{path}: expected unnamespaced score-partwise XML")
    declared = [p.attrib["id"] for p in root.findall("part-list/score-part")]
    parts = root.findall("part")
    if len(set(declared)) != len(declared) or [p.get("id") for p in parts] != declared:
        raise TranscriptError(f"{path}: ambiguous or reordered declared parts")
    result = []
    for part_index, part in enumerate(parts, 1):
        divisions = None
        lanes = defaultdict(list)
        transpose = {None: Fraction(0)}
        labels = []
        extents = []
        for ordinal, measure in enumerate(part.findall("measure"), 1):
            where = f"{path}: part {part_index}, measure {ordinal} ({measure.get('number')!r})"
            labels.append(measure.get("number"))
            cursor = Fraction(0)
            extent = Fraction(0)
            previous = None
            pending = defaultdict(list)
            occupied = set()

            def fail(message):
                raise TranscriptError(f"{where}: {message}")

            def number(text, name):
                try:
                    return Fraction(text)
                except (ValueError, TypeError, ZeroDivisionError):
                    fail(f"invalid {name}: {text!r}")

            def duration(node):
                if divisions is None:
                    fail("duration before divisions")
                value = number(node.findtext("duration"), "duration") / divisions
                if value <= 0:
                    fail("nonpositive duration")
                return value

            for child in measure:
                if child.tag == "attributes":
                    if child.find("divisions") is not None:
                        divisions = number(child.findtext("divisions"), "divisions")
                        if divisions <= 0:
                            fail("nonpositive divisions")
                    for tr in child.findall("transpose"):
                        if tr.find("double") is not None:
                            fail("double transposition not represented")
                        staff = tr.get("number")
                        shift = number(tr.findtext("chromatic"), "transpose chromatic")
                        shift += 12 * number(tr.findtext("octave-change", "0"), "octave-change")
                        transpose[staff] = shift
                    previous = None
                elif child.tag in ("backup", "forward"):
                    cursor += duration(child) * (-1 if child.tag == "backup" else 1)
                    if cursor < 0:
                        fail("backup precedes measure")
                    previous = None
                elif child.tag == "note":
                    voice = child.findtext("voice", "1")
                    staff = child.findtext("staff", "1")
                    if not voice or not staff.isdigit() or int(staff) < 1:
                        fail("invalid voice/staff")
                    grace = child.find("grace")
                    is_grace = grace is not None
                    chord = child.find("chord") is not None
                    if child.find("cue") is not None:
                        fail("cue-note sounding policy is not represented")
                    if child.find("unpitched") is not None:
                        fail("unpitched instrument mapping is not represented")
                    pitch = child.find("pitch")
                    rest = child.find("rest")
                    if (pitch is None) == (rest is None):
                        fail("note must contain exactly one pitch or rest")
                    if pitch is not None:
                        step = pitch.findtext("step")
                        if step not in dict(C=0, D=2, E=4, F=5, G=7, A=9, B=11):
                            fail("invalid pitch step")
                        actual = 12 * (number(pitch.findtext("octave"), "octave") + 1)
                        actual += dict(C=0, D=2, E=4, F=5, G=7, A=9, B=11)[step]
                        actual += number(pitch.findtext("alter", "0"), "pitch alter")
                        actual += transpose.get(staff, transpose[None])
                        actual = str(actual)
                    else:
                        actual = "rest"
                    ties = []
                    for tie in child.findall("tie"):
                        if set(tie.attrib) - {"type"} or tie.get("type") not in ("start", "stop"):
                            fail("unsupported sound tie attributes")
                        ties.append(tie.get("type"))
                    if len(set(ties)) != len(ties) or (ties and rest is not None):
                        fail("duplicate or rest sound tie")
                    timing = None
                    if is_grace:
                        if child.find("duration") is not None or set(grace.attrib) - {"slash", "steal-time-previous", "steal-time-following", "make-time"}:
                            fail("unsupported grace timing structure")
                        timing = {}
                        for key in ("steal-time-previous", "steal-time-following", "make-time"):
                            if key in grace.attrib:
                                value = number(grace.get(key), key)
                                if value < 0 or (key != "make-time" and value > 100):
                                    fail("invalid grace timing")
                                if key == "make-time":
                                    if divisions is None:
                                        fail("grace make-time before divisions")
                                    value /= divisions
                                timing[key] = str(value)
                        length = None
                    else:
                        length = str(duration(child))
                    note = {"staff": int(staff), "pitch": actual, "duration": length, "ties": sorted(ties), "grace_timing": timing}
                    if chord:
                        if previous is None or previous[0] != voice or previous[1] != is_grace:
                            fail("chord has no adjacent matching voice/grace head")
                        group = previous[2]
                        if rest is not None or any(n["pitch"] == "rest" for n in group["notes"]):
                            fail("rest chord")
                        group["notes"].append(note)
                        if length is not None:
                            extent = max(extent, Fraction(group["onset"]) + Fraction(length))
                    else:
                        group = {"measure": ordinal, "onset": str(cursor), "notes": [note]}
                        previous = (voice, is_grace, group)
                        if is_grace:
                            pending[voice].append(group)
                        else:
                            if (voice, cursor) in occupied:
                                fail("ambiguous independent attacks in one voice at one onset")
                            occupied.add((voice, cursor))
                            grace_groups = pending.pop(voice, [])
                            if any(g["onset"] != str(cursor) for g in grace_groups):
                                fail("grace group is not at its principal onset")
                            if grace_groups and rest is not None:
                                fail("grace principal is a rest; realization ambiguous")
                            group["graces"] = grace_groups
                            lanes[voice].append(group)
                            cursor += Fraction(length)
                elif child.tag not in ("direction", "barline", "print", "harmony", "figured-bass", "sound", "grouping", "link", "bookmark"):
                    fail(f"unsupported measure cursor structure {child.tag}")
                extent = max(extent, cursor)
            extents.append(str(extent))
            if any(pending.values()):
                fail("unattached grace group at end of measure")
        # Voice strings are not identities across adapters. Compare the multiset of
        # complete lane histories, including staff and ordered grace context.
        encoded = []
        for groups in lanes.values():
            for group in groups:
                for item in [group] + group["graces"]:
                    item["notes"].sort(key=lambda n: json.dumps(n, sort_keys=True))
            groups.sort(key=lambda g: (g["measure"], Fraction(g["onset"])))
            encoded.append(json.dumps(groups, sort_keys=True, separators=(",", ":")))
        if any(count > 1 for count in Counter(encoded).values()):
            raise TranscriptError(f"{path}: part {part_index}: indistinguishable voice lanes; identity matching ambiguous")
        result.append({"part": part_index, "measure_labels": labels, "encoded_extents": extents, "voices": sorted(encoded)})
    return result


def compare(source, exported):
    left, right = transcript(source), transcript(exported)
    if len(left) != len(right):
        raise TranscriptError("declared part count changed")
    for a, b in zip(left, right):
        if len(a["measure_labels"]) != len(b["measure_labels"]):
            raise TranscriptError(f"part {a['part']}: measure count changed from {len(a['measure_labels'])} to {len(b['measure_labels'])}")
        for ordinal, (source_label, export_label) in enumerate(zip(a["measure_labels"], b["measure_labels"]), 1):
            if source_label != export_label:
                raise TranscriptError(f"part {a['part']}, measure {ordinal}: label changed from {source_label!r} to {export_label!r}")
        for ordinal, (source_extent, export_extent) in enumerate(zip(a["encoded_extents"], b["encoded_extents"]), 1):
            if source_extent != export_extent:
                label = a["measure_labels"][ordinal - 1]
                raise TranscriptError(f"part {a['part']}, measure {ordinal} ({label!r}): encoded extent differs: source {source_extent} quarters, export {export_extent} quarters; note durations and forward/backup cursor movement are compared without inferred meter padding")
        if a["voices"] != b["voices"]:
            raise TranscriptError(f"part {a['part']}: note-event transcript differs (lane assignments, time, pitch, chords, grace timing or sound ties)")
    return {"equal": True, "parts": len(left), "scope": "exact authored note events; not audio or complete performance equivalence"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("exported", type=Path)
    args = parser.parse_args()
    try:
        print(json.dumps(compare(args.source, args.exported), indent=2))
    except (TranscriptError, ET.ParseError, OSError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
