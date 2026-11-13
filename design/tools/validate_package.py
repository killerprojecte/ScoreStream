#!/usr/bin/env python3
"""Finite design-package QA, not the production ScoreStream parser.

Reads the versioned descriptor and checks the bundled core examples. This
handwritten line reader does not compile or completely implement the EBNF.
Requires jsonschema and referencing. No renderer, MCP server, Rust build,
MusicXML converter, or annotation database is exercised by this program.
"""
from __future__ import annotations

import hashlib
import importlib.metadata
import json
import re
import sys
from dataclasses import dataclass
from fractions import Fraction
from pathlib import Path

import jsonschema
from referencing import Registry, Resource

ROOT = Path(__file__).resolve().parents[1]
ID = r"[A-Za-z][A-Za-z0-9_-]*"
ID_RE = re.compile(ID + r"\Z")
INT_RE = re.compile(r"-?(?:0|[1-9][0-9]*)\Z")
RAT_RE = re.compile(r"-?(?:0|[1-9][0-9]*)(?:/[1-9][0-9]*)?\Z")
PITCH_RE = re.compile(r"([A-G])(##|#|bb|b|\(([+-]?(?:0|[1-9][0-9]*)(?:/[1-9][0-9]*)?)\))?(-?(?:0|[1-9][0-9]*))\Z")
ATTR_RE = re.compile(rf"(?:{ID}|x\.(?:{ID}\.)+{ID})\Z")
BOOL_FIELDS = {"visible", "slash", "cautionary", "parentheses", "editorial", "sound", "print", "bracket", "line", "sign"}
INT_FIELDS = {"staff", "dots", "order", "actual", "normal", "level", "octaves", "tm_normal_dots", "transpose_diatonic", "transpose_octaves"}


@dataclass
class Problem(Exception):
    code: str
    message: str
    line: int | None = None

    def as_dict(self, severity="error"):
        result = {"code": self.code, "severity": severity, "message": self.message}
        if self.line is not None:
            result["line"] = self.line
        return result


def fail(code, message, line=None):
    raise Problem(code, message, line)


def load(path):
    return json.loads((ROOT / path).read_text(encoding="utf-8"))


def write_json(path, value):
    path = ROOT / path
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def exact(value, *, positive=False, nonnegative=False, line=None):
    if not isinstance(value, str) or not RAT_RE.fullmatch(value):
        fail("RATIONAL_LEXICAL", f"Not an exact rational token: {value!r}", line)
    result = Fraction(value)
    if str(result) != value:
        fail("RATIONAL_NOT_CANONICAL", f"{value!r} must be written {str(result)!r}", line)
    if positive and result <= 0:
        fail("DURATION_NOT_POSITIVE", f"Expected a positive value, found {value}", line)
    if nonnegative and result < 0:
        fail("TIME_NEGATIVE", f"Expected a nonnegative position, found {value}", line)
    return result


def tokens(line, number):
    """Keep JSON-quoted attribute values intact; '#' is never a comment."""
    if line[:1] in (" ", "\t") or line[-1:] in (" ", "\t"):
        fail("LINE_WHITESPACE", "Non-trivia fixture lines have no outer whitespace", number)
    out, current, quoted, escaped = [], [], False, False
    for char in line:
        if ord(char) < 32 and char != "\t":
            fail("CONTROL_CHARACTER", "Control character in a physical source line", number)
        if quoted:
            current.append(char)
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                quoted = False
        elif char == '"':
            quoted = True
            current.append(char)
        elif char in (" ", "\t"):
            if current:
                out.append("".join(current))
                current = []
        else:
            current.append(char)
    if quoted:
        fail("STRING_UNTERMINATED", "JSON string is not closed on this line", number)
    if current:
        out.append("".join(current))
    return out


def attrs(items, number, context):
    result = {}
    for item in items:
        if "=" not in item:
            fail("ATTRIBUTE_SYNTAX", f"Expected key=value, found {item!r}", number)
        key, raw = item.split("=", 1)
        if not ATTR_RE.fullmatch(key) or not raw:
            fail("ATTRIBUTE_SYNTAX", f"Invalid attribute {item!r}", number)
        if key in result:
            fail("ATTRIBUTE_DUPLICATE", f"Repeated attribute {key}", number)
        quoted = raw.startswith('"')
        if quoted:
            try:
                value = json.loads(raw)
            except json.JSONDecodeError as exc:
                fail("STRING_ESCAPE", str(exc), number)
            if not isinstance(value, str):
                fail("STRING_TYPE", "Quoted value must be a JSON string", number)
        else:
            if '"' in raw:
                fail("STRING_SYNTAX", "Quotes must surround a complete attribute value", number)
            value = raw
        if context != "metadata" and key in BOOL_FIELDS:
            if quoted or raw not in ("true", "false"):
                fail("ATTRIBUTE_TYPE", f"{key} requires an unquoted boolean", number)
            value = raw == "true"
        elif context != "metadata" and (key in INT_FIELDS or key == "staves" or (context == "state" and key == "key" and raw != "unset")):
            if quoted or not INT_RE.fullmatch(raw) or raw == "-0":
                fail("ATTRIBUTE_TYPE", f"{key} requires a canonical integer", number)
            value = int(raw)
        result[key] = value
    return result


def parse_source(source, language):
    """Parse only the documented line-oriented core QA subset into an AST."""
    source = source.replace("\r\n", "\n")
    if "\r" in source:
        fail("NEWLINE", "Lone CR is outside this reader's input subset")
    ast = {"format": "ScoreStream", "format_version": language["format_version"], "time_unit": "quarter", "parts": [], "voices": [], "metadata": {}, "required_extensions": [], "frames": []}
    locations = {}
    frame = None
    header = False
    began_frames = False
    ended = False
    for number, raw in enumerate(source.splitlines(keepends=True), 1):
        has_lf = raw.endswith("\n")
        line = raw[:-1] if has_lf else raw
        if re.fullmatch(r"[ \t]*(?:;[^\n]*)?", line):
            if not has_lf and line:
                fail("NEWLINE", "Trivia lines require LF", number)
            continue
        ts = tokens(line, number)
        if ended:
            fail("AFTER_END", "Only trivia may follow !end", number)
        if not has_lf and ts[0] != "!end":
            fail("NEWLINE", "Only the final !end may omit LF", number)
        if not header:
            if ts[:3] not in (["!score", language["format_version"], "unit=q"], ["!fragment", language["format_version"], "unit=q"]):
                fail("HEADER", "Expected the versioned score or fragment header", number)
            ast["document_kind"] = ts[0][1:]
            if ast["document_kind"] == "score" and len(ts) != 3:
                fail("HEADER", "Unexpected score header fields", number)
            if ast["document_kind"] == "fragment":
                if len(ts) != 4 or not ts[3].startswith("span="):
                    fail("HEADER", "Fragment header requires span", number)
                ast["span"] = ts[3][5:]
                exact(ast["span"], positive=True, line=number)
            header = True
            continue
        if ts[0] == "!end":
            if frame is not None:
                fail("FRAME_UNCOMMITTED", "A dot must commit the frame before !end", number)
            if len(ts) != 2 or not ts[1].startswith("at="):
                fail("END_SYNTAX", "Expected !end at=<position>", number)
            ast["end"] = ts[1][3:]
            exact(ast["end"], nonnegative=True, line=number)
            ended = True
            continue
        if ts[0].startswith("!"):
            if began_frames or frame is not None:
                fail("DECLARATION_POSITION", "Declarations must precede frames", number)
            if ast["document_kind"] == "fragment":
                fail("FRAGMENT_DECLARATION", "Fragments use externally supplied aliases", number)
            if ts[0] in ("!part", "!voice"):
                if len(ts) != 4 or not ID_RE.fullmatch(ts[1]):
                    fail("DECLARATION_SYNTAX", "Malformed part/voice declaration", number)
                field_names = [x.split("=", 1)[0] for x in ts[2:]]
                expected = ["instrument", "staves"] if ts[0] == "!part" else ["part", "staff"]
                if field_names != expected:
                    fail("DECLARATION_SYNTAX", f"Expected fields {expected}", number)
                values = attrs(ts[2:], number, "declaration")
                ast["parts" if ts[0] == "!part" else "voices"].append({"id": ts[1], **values})
            elif ts[0] == "!meta" and len(ts) > 1:
                values = attrs(ts[1:], number, "metadata")
                if set(values) & set(ast["metadata"]):
                    fail("METADATA_DUPLICATE", "Repeated metadata key", number)
                ast["metadata"].update(values)
            elif ts[0] == "!requires" and len(ts) == 2 and re.fullmatch(rf"{ID}(?:\.{ID})*@(?:0|[1-9][0-9]*)(?:\.(?:0|[1-9][0-9]*))*", ts[1]):
                ast["required_extensions"].append(ts[1])
            else:
                fail("DECLARATION_SYNTAX", f"Unknown/malformed declaration {ts[0]}", number)
            continue
        if ts[0].startswith("@"):
            if frame is not None:
                fail("FRAME_UNCOMMITTED", "A dot must precede the next frame", number)
            if len(ts) != 1:
                fail("FRAME_SYNTAX", "Frame header contains extra fields", number)
            time = ts[0][1:]
            exact(time, nonnegative=True, line=number)
            frame = {"time": time, "records": []}
            began_frames = True
            continue
        if ts == ["."]:
            if frame is None:
                fail("FRAME_ABSENT", "No frame is open", number)
            ast["frames"].append(frame)
            frame = None
            continue
        if frame is None:
            fail("FRAME_ABSENT", "A record must occur inside a frame", number)
        if ts[0] == "=" and len(ts) >= 3:
            record = {"record": "state", "target": ts[1], "attributes": attrs(ts[2:], number, "state")}
        elif ts[0] == "+" and len(ts) >= 3:
            record = {"record": "mark", "target": ts[1], "kind": ts[2], "attributes": attrs(ts[3:], number, "mark")}
        elif ts[0] == "~" and len(ts) >= 4:
            if "=" not in ts[3]:
                fail("RELATION_SYNTAX", "Relation requires phase=#anchor", number)
            phase, anchor = ts[3].split("=", 1)
            if phase not in ("start", "end", "continue"):
                fail("RELATION_SYNTAX", "Invalid relation phase", number)
            record = {"record": "relation", "kind": ts[1], "id": ts[2], "phase": phase, "anchor": anchor, "attributes": attrs(ts[4:], number, "relation")}
        elif ts[0] == "|" and len(ts) >= 4 and ts[3].startswith("len="):
            record = {"record": "measure", "target": ts[1], "id": ts[2], "length": ts[3][4:], "attributes": attrs(ts[4:], number, "measure")}
        elif ID_RE.fullmatch(ts[0]) and len(ts) >= 2 and ":" in ts[1]:
            pitch, duration = ts[1].split(":", 1)
            record = {"record": "event", "voice": ts[0], "pitch": pitch, "duration": duration, "attributes": attrs(ts[2:], number, "event")}
        else:
            fail("RECORD_SYNTAX", "Record is outside the finite core reader", number)
        locations[(len(ast["frames"]), len(frame["records"]))] = number
        frame["records"].append(record)
    if frame is not None:
        fail("FRAME_UNCOMMITTED", "EOF cannot commit an open frame")
    if not header or not ended:
        fail("END_MISSING", "A complete document requires a header and !end")
    return ast, locations


def pitch_value(pitch, line=None):
    if pitch in ("r", "_") or re.fullmatch(rf"u\.{ID}", pitch):
        return None
    match = PITCH_RE.fullmatch(pitch)
    if not match:
        fail("PITCH_LEXICAL", f"Invalid written pitch {pitch!r}", line)
    step, short, rational, octave = match.groups()
    alteration = Fraction({None: 0, "#": 1, "##": 2, "b": -1, "bb": -2}.get(short, 0))
    if rational is not None:
        alteration = exact(rational.removeprefix("+"), line=line)
    return Fraction(12 * (int(octave) + 1) + {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}[step]) + alteration


def semantic_check(ast, language, locations, fragment_context=None):
    warnings = []
    rules = language["vocabulary"]
    parts, voices = {}, {}
    scopes = {"score"}
    for part in ast["parts"]:
        if part["id"] in scopes:
            fail("ALIAS_DUPLICATE", "Part/voice aliases are disjoint and score is reserved")
        if part["staves"] <= 0:
            fail("STAFF_RANGE", "Part requires at least one staff")
        parts[part["id"]] = part
        scopes.add(part["id"])
    for voice in ast["voices"]:
        if voice["id"] in scopes:
            fail("ALIAS_DUPLICATE", "Part/voice aliases are disjoint and score is reserved")
        if voice["part"] not in parts:
            fail("PART_UNDECLARED", f"Undeclared part {voice['part']}")
        if not 1 <= voice["staff"] <= parts[voice["part"]]["staves"]:
            fail("STAFF_RANGE", "Voice default staff does not exist")
        voices[voice["id"]] = voice
        scopes.add(voice["id"])
    if ast["document_kind"] == "fragment":
        if fragment_context is None:
            fail("FRAGMENT_CONTEXT_REQUIRED", "External voice bindings are required")
        parts.update(fragment_context["parts"])
        voices.update(fragment_context["voices"])
    if ast["required_extensions"]:
        fail("QA_UNSUPPORTED_EXTENSION", "This package checker executes only the ordinary core subset")

    def scope_type(target, line):
        if target == "score":
            return "score"
        match = re.fullmatch(rf"({ID})\.s([1-9][0-9]*)", target)
        if match:
            part, staff = match.groups()
            if part not in parts:
                fail("PART_UNDECLARED", f"Undeclared part {part}", line)
            if int(staff) > parts[part]["staves"]:
                fail("STAFF_RANGE", f"Missing staff {target}", line)
            return "staff"
        if target in parts:
            return "part"
        if target in voices:
            return "voice"
        fail("SCOPE_UNDECLARED", f"Unknown scope {target}", line)

    def fields(values, allowed, line):
        for name in values:
            if name not in allowed:
                code = "QA_UNSUPPORTED_EXTENSION" if name.startswith("x.") else "FIELD_UNKNOWN"
                fail(code, f"Field {name!r} is not in the supported record registry", line)

    end = exact(ast["end"], nonnegative=True)
    span = exact(ast["span"], positive=True) if ast["document_kind"] == "fragment" else None
    if span is not None and end != span:
        fail("FRAGMENT_SPAN", "Fragment end must equal its declared span")
    previous = None
    events, objects, chords, relations, refs = {}, {}, {}, {}, []
    all_events, ordered_relations = [], []
    default_staff = {name: voice["staff"] for name, voice in voices.items()}
    has_transposition = False
    for fi, frame in enumerate(ast["frames"]):
        at = exact(frame["time"], nonnegative=True)
        if previous is not None and at <= previous:
            fail("FRAME_ORDER", "Core frame positions must strictly increase")
        if at > end:
            fail("END_BEFORE_FRAME", "A frame lies beyond !end")
        previous = at
        assignments = {}
        for ri, record in enumerate(frame["records"]):
            if record["record"] != "state":
                continue
            line = locations[(fi, ri)]
            target, values = record["target"], record["attributes"]
            fields(values, rules["state"], line)
            category = scope_type(target, line)
            if span is not None and (category != "voice" or set(values) != {"staff"}):
                fail("FRAGMENT_GLOBAL_STATE", "A replacement fragment cannot change global/part/staff state", line)
            for key, value in values.items():
                if category not in rules["state"][key]["scope"]:
                    fail("STATE_SCOPE", f"{key} cannot target {category}", line)
                address = (target, key)
                if address in assignments and assignments[address] != value:
                    fail("STATE_CONFLICT", f"Conflicting simultaneous assignment to {target}.{key}", line)
                assignments[address] = value
                if key == "staff":
                    if not 1 <= value <= parts[voices[target]["part"]]["staves"]:
                        fail("STAFF_RANGE", "New voice default staff does not exist", line)
                    default_staff[target] = value
                elif key == "tempo":
                    match = re.fullmatch(r"q\.?:([0-9]+(?:/[0-9]+)?)", str(value))
                    if not match:
                        fail("STATE_VALUE", "QA tempo subset is q:<rational> or q.:<rational>", line)
                    exact(match.group(1), positive=True, line=line)
                elif key == "meter" and value != "none" and not re.fullmatch(r"[1-9][0-9]*(?:\+[1-9][0-9]*)*/[1-9][0-9]*", str(value)):
                    fail("STATE_VALUE", "Unsupported meter token", line)
                elif key == "clef" and value not in ("G2", "F4", "C3", "percussion", "TAB"):
                    fail("QA_UNSUPPORTED_CLEF", "Clef is outside the documented fixture vocabulary", line)
                elif key == "transpose_chromatic":
                    exact(value, line=line)
                    has_transposition = True
                elif key.startswith("transpose_"):
                    has_transposition = True
        for ri, record in enumerate(frame["records"]):
            line = locations[(fi, ri)]
            kind, values = record["record"], record["attributes"]
            if kind == "state":
                continue
            if kind == "event":
                fields(values, rules["event"], line)
                if record["voice"] not in voices:
                    fail("VOICE_UNDECLARED", f"Undeclared voice {record['voice']}", line)
                voice = voices[record["voice"]]
                staff = values.get("staff", default_staff[record["voice"]])
                if not 1 <= staff <= parts[voice["part"]]["staves"]:
                    fail("STAFF_RANGE", "Event staff override does not exist", line)
                sounding = pitch_value(record["pitch"], line)
                ekind = values.get("kind", "note")
                if ekind not in rules["event"]["kind"]["values"]:
                    fail("EVENT_KIND", "Unknown event kind", line)
                if ekind == "grace":
                    if record["duration"] != "?":
                        fail("GRACE_DURATION", "Grace duration must be ?", line)
                    if "anchor" not in values:
                        fail("GRACE_ANCHOR", "Grace event requires an anchor", line)
                    if not isinstance(values.get("order"), int) or isinstance(values.get("order"), bool) or values["order"] == 0:
                        fail("GRACE_ORDER", "Grace order must be a nonzero integer", line)
                    duration = None
                else:
                    if record["duration"] == "?":
                        fail("GRACE_KIND", "Only a grace event can use ? in the QA core subset", line)
                    duration = exact(record["duration"], positive=True, line=line)
                    if at + duration > end:
                        fail("END_TRUNCATES_EVENT", "!end truncates a measured event", line)
                if span is not None and at >= span:
                    fail("FRAGMENT_ONSET", "Fragment event onsets must lie in [0,span)", line)
                item = {"record": record, "time": at, "duration": duration, "frame": fi, "line": line, "pitch_number": sounding}
                all_events.append(item)
                if "id" in values:
                    identity = values["id"]
                    if not isinstance(identity, str) or not ID_RE.fullmatch(identity):
                        fail("ID_LEXICAL", "Invalid SFS event identity", line)
                    if identity in objects:
                        fail("ID_DUPLICATE", f"Duplicate object ID {identity}", line)
                    events[identity] = item
                    objects[identity] = item
                if "chord" in values:
                    chords.setdefault(values["chord"], []).append(item)
                if ekind == "grace":
                    refs.append((values["anchor"], fi, line, "grace", item))
                for name in ("w", "tm_normal_type"):
                    if name in values and str(values[name]) not in rules["event"]["w"]["values"]:
                        fail("WRITTEN_TYPE", f"Unknown {name} note type", line)
                for name in ("dots", "tm_normal_dots"):
                    if values.get(name, 0) < 0:
                        fail("DOTS_NEGATIVE", "Dot counts cannot be negative", line)
                if "stem" in values and values["stem"] not in rules["event"]["stem"]["values"]:
                    fail("STEM_VALUE", "Unknown stem value", line)
                if "notehead" in values:
                    warnings.append(Problem("QA_NOTEHEAD_UNINTERPRETED", "The package has no complete registered notehead catalog", line).as_dict("warning"))
                ratio = Fraction(1)
                if "tm" in values:
                    match = re.fullmatch(r"([1-9][0-9]*):([1-9][0-9]*)", str(values["tm"]))
                    if not match:
                        fail("TIME_MODIFICATION", "tm requires positive actual:normal integers", line)
                    actual, normal = map(int, match.groups())
                    ratio = Fraction(normal, actual)
                elif "tm_normal_type" in values or "tm_normal_dots" in values:
                    fail("TIME_MODIFICATION", "Normal-type fields require tm", line)
                if "tm_normal_dots" in values and "tm_normal_type" not in values:
                    fail("TIME_MODIFICATION", "Normal dots require a normal note type", line)
                if "w" in values and duration is not None:
                    normal_given = "tm_normal_type" in values
                    written = str(values.get("tm_normal_type", values["w"]))
                    base = {"breve": Fraction(8), "long": Fraction(16), "maxima": Fraction(32)}.get(written)
                    if base is None:
                        base = Fraction(4, int(written))
                    dots = values.get("tm_normal_dots", 0) if normal_given else values.get("dots", 0)
                    expected = base * (Fraction(2) - Fraction(1, 2 ** dots)) * ratio
                    if duration != expected:
                        warnings.append(Problem("WRITTEN_DURATION_MISMATCH", f"Measured {duration} differs from illustrative written calculation {expected}; both are preserved", line).as_dict("warning"))
            elif kind == "measure":
                fields(values, {"label"}, line)
                if span is not None:
                    fail("FRAGMENT_MEASURE", "Replacement fragments cannot declare measures", line)
                if scope_type(record["target"], line) not in ("part", "staff"):
                    fail("MEASURE_SCOPE", "Measure scope must be a part or staff", line)
                length = exact(record["length"], positive=True, line=line)
                if at + length > end:
                    fail("END_TRUNCATES_MEASURE", "!end truncates a declared measure", line)
                if record["id"] in objects:
                    fail("ID_DUPLICATE", "Duplicate event/measure object ID", line)
                objects[record["id"]] = {"frame": fi, "time": at, "record": record}
            elif kind == "mark":
                if record["kind"] not in rules["marks"]:
                    fail("MARK_UNKNOWN", "Unregistered mark kind", line)
                fields(values, rules["marks"][record["kind"]], line)
                if record["target"].startswith("#"):
                    refs.append((record["target"], fi, line, "mark", record))
                else:
                    scope_type(record["target"], line)
            elif kind == "relation":
                if record["kind"] not in rules["relations"]:
                    fail("RELATION_UNKNOWN", "Unregistered relation kind", line)
                fields(values, rules["relations"][record["kind"]]["attributes"], line)
                for name in ("actual", "normal", "level"):
                    if name in values and values[name] <= 0:
                        fail("RELATION_VALUE", f"{name} must be positive", line)
                refs.append((record["anchor"], fi, line, "relation", record))
                ordered_relations.append((record, fi, line))

    grace_orders = set()
    for reference, fi, line, purpose, owner in refs:
        if not isinstance(reference, str) or not re.fullmatch(rf"#{ID}", reference):
            fail("REFERENCE_LEXICAL", "References use # plus a text object ID", line)
        identity = reference[1:]
        target = objects.get(identity)
        if target is None:
            fail("REFERENCE_UNRESOLVED", f"Unresolved {reference}", line)
        if target["frame"] > fi:
            fail("REFERENCE_FUTURE_FRAME", "References must resolve by the frame commit", line)
        if purpose in ("grace", "relation") and identity not in events:
            fail("REFERENCE_KIND", "This anchor must identify an event", line)
        if purpose == "grace":
            if target["frame"] != fi or target["record"]["attributes"].get("kind") == "grace":
                fail("GRACE_ANCHOR", "Principal grace anchor must be a non-grace event in the same frame", line)
            key = (identity, owner["record"]["attributes"]["order"])
            if key in grace_orders:
                fail("GRACE_ORDER_DUPLICATE", "Order must be unique for the principal anchor", line)
            grace_orders.add(key)
    for identity, members in chords.items():
        if not isinstance(identity, str) or not ID_RE.fullmatch(identity):
            fail("ID_LEXICAL", "Chord identity must be a text identifier")
        if len({x["time"] for x in members}) != 1:
            fail("CHORD_ONSET", "Members of one notated chord must share an onset")
        if len({x["record"]["voice"] for x in members}) != 1:
            fail("CHORD_VOICE", "A notated chord group belongs to one logical voice")
        if any(x["record"]["pitch"] in ("r", "_") for x in members):
            fail("CHORD_EVENT_KIND", "Rests and spacers are not chord note members")
        # Intentionally no equal-duration or head-is-longest rule in the core.
    for record, fi, line in ordered_relations:
        identity, phase = record["id"], record["phase"]
        if phase == "start":
            if identity in relations:
                fail("RELATION_DUPLICATE", "Relation IDs cannot be restarted", line)
            relations[identity] = {"kind": record["kind"], "start": record, "closed": False}
        else:
            if identity not in relations or relations[identity]["closed"]:
                fail("RELATION_PHASE", "Relation continuation/end requires an open start", line)
            rel = relations[identity]
            if rel["kind"] != record["kind"]:
                fail("RELATION_KIND", "A relation cannot change kind", line)
            start = events[rel["start"]["anchor"][1:]]
            finish = events[record["anchor"][1:]]
            if finish["time"] < start["time"]:
                fail("RELATION_ORDER", "Terminal anchor precedes start anchor", line)
            if phase == "end":
                rel["closed"] = True
                rel["end"] = record
                if record["kind"] == "tuplet" and start["record"]["voice"] != finish["record"]["voice"]:
                    fail("TUPLET_VOICE", "This core tuplet group requires one logical voice", line)
                if record["kind"] == "tie" and rel["start"]["attributes"].get("sound", False):
                    if start["duration"] is None or finish["duration"] is None or start["pitch_number"] is None or finish["pitch_number"] is None:
                        fail("QA_UNSUPPORTED_TIE", "Sounding tie QA covers measured pitched endpoints only", line)
                    if start["time"] + start["duration"] != finish["time"]:
                        fail("TIE_CONTINUITY", "Sounding tie endpoints must meet without a gap", line)
                    if has_transposition:
                        warnings.append(Problem("QA_TIE_PITCH_CONTEXT", "Pitch comparison with changing transposition is outside finite QA", line).as_dict("warning"))
                    elif start["pitch_number"] != finish["pitch_number"]:
                        fail("TIE_PITCH", "Sounding tie endpoints have different nominal pitches", line)
    if any(not relation["closed"] for relation in relations.values()):
        fail("RELATION_UNCLOSED", "This standalone QA fixture supplies no external open-relation context")
    if span is not None:
        for alias in voices:
            intervals = sorted((x["time"], x["time"] + x["duration"]) for x in all_events if x["record"]["voice"] == alias and x["duration"] is not None)
            reached = Fraction(0)
            for left, right in intervals:
                if left > reached:
                    fail("FRAGMENT_GAP", f"Uncovered interval in bound voice {alias}")
                reached = max(reached, right)
            if reached != span:
                fail("FRAGMENT_GAP", f"Bound voice {alias} does not cover its complete span")
    return warnings


def fragment_bindings(aliases):
    return {"parts": {"qaPart": {"id": "qaPart", "staves": 2}}, "voices": {alias: {"id": alias, "part": "qaPart", "staff": 1} for alias in aliases}}


def validate_source(source, language, ast_validator, context=None):
    ast = None
    try:
        ast, locations = parse_source(source, language)
        warnings = semantic_check(ast, language, locations, context)
        schema_errors = list(ast_validator.iter_errors(ast))
        if schema_errors:
            return ast, [{"code": "AST_SCHEMA", "severity": "error", "message": error.message, "path": list(error.absolute_path)} for error in schema_errors], warnings
        return ast, [], warnings
    except Problem as exc:
        return ast, [exc.as_dict()], []


def auxiliary_semantics(data, language, ast_validator, fixture_context):
    diagnostics = []

    def walk(node):
        if isinstance(node, dict):
            if {"clock_id", "start", "end"} <= node.keys():
                try:
                    start, end = exact(node["start"]), exact(node["end"])
                    if start >= end:
                        fail("AUX_RANGE_ORDER", "Span start must be before end")
                except Problem as exc:
                    diagnostics.append(exc.as_dict())
            for value in node.values():
                walk(value)
        elif isinstance(node, list):
            for value in node:
                walk(value)
    walk(data)
    selection = data.get("selection", {}).get("score", {})
    if selection and (selection.get("score_id") != data.get("score_id") or selection.get("revision_id") != data.get("expected_revision_id")):
        diagnostics.append(Problem("AUX_SELECTION_REVISION", "Frozen selection and edit base disagree").as_dict())
    fragments = {}
    for index, operation in enumerate(data.get("operations", [])):
        if operation.get("op") != "replace_range":
            continue
        aliases = [binding["fragment_voice"] for binding in operation["voice_bindings"]]
        ast, errors, warnings = validate_source(operation["fragment_sfs"], language, ast_validator, fragment_bindings(aliases))
        diagnostics.extend(errors)
        if ast is not None and ast.get("document_kind") == "fragment":
            try:
                expected = exact(operation["span"]["end"]) - exact(operation["span"]["start"])
                if exact(ast["span"], positive=True) != expected:
                    diagnostics.append(Problem("AUX_REPLACEMENT_LENGTH", "Fragment span differs from preserved target length").as_dict())
                fragments[index] = {r["attributes"]["id"] for f in ast["frames"] for r in f["records"] if r["record"] == "event" and "id" in r["attributes"]}
            except Problem as exc:
                diagnostics.append(exc.as_dict())
    for operation in data.get("operations", []):
        target = operation.get("target", {})
        if operation.get("op") == "retarget_relation" and target.get("kind") == "fragment":
            if target["fragment_event_id"] not in fragments.get(target["operation_index"], set()):
                diagnostics.append(Problem("AUX_FRAGMENT_TARGET", "Retarget references an event not created by that operation").as_dict())
    binding = data.get("binding", {})
    anchor = binding.get("anchor", {})
    if anchor.get("kind") == "visual_snapshot":
        known = fixture_context["render_records"].get(anchor["render_id"])
        if known is None:
            diagnostics.append(Problem("QA_RENDER_CONTEXT_MISSING", "No seeded render metadata for this fixture").as_dict())
        elif known != binding.get("score"):
            diagnostics.append(Problem("AUX_RENDER_REVISION", "Seeded render metadata disagrees with the pinned score").as_dict())
    return diagnostics


def main():
    language = load("spec/scorestream-language.json")
    fixture_context = load("validation/fixture-context.json")
    schemas = [load(str(path.relative_to(ROOT))) for path in sorted((ROOT / "schemas").glob("*.schema.json"))]
    registry = Registry().with_resources((schema["$id"], Resource.from_contents(schema)) for schema in schemas)
    for schema in schemas:
        jsonschema.Draft202012Validator.check_schema(schema)
    ast_validator = jsonschema.Draft202012Validator({"$ref": "urn:scorestream:schema:ast:1.1"}, registry=registry)
    descriptor_validator = jsonschema.Draft202012Validator({"$ref": "urn:scorestream:schema:format-reference:1.1"}, registry=registry)
    descriptor_errors = [error.message for error in descriptor_validator.iter_errors(language)]
    results = []
    artifacts = ["spec/scorestream.ebnf", "spec/scorestream-language.json", "schemas/scorestream-ast.schema.json", "tools/build_format_reference.py", "tools/validate_package.py", "validation/fixture-context.json"]
    for example in language["examples"]:
        source = (ROOT / example["file"]).read_text(encoding="utf-8")
        context = fragment_bindings(fixture_context["standalone_fragment_aliases"]) if example["name"] == "replacement-fragment" else None
        ast, errors, warnings = validate_source(source, language, ast_validator, context)
        if source != example["source"]:
            errors.append(Problem("DESCRIPTOR_SOURCE_MISMATCH", "Descriptor example differs from the source fixture").as_dict())
        if ast is not None:
            event_count = sum(r["record"] == "event" for f in ast["frames"] for r in f["records"])
            expected = fixture_context["expected_counts"][example["name"]]
            if event_count != expected["events"] or len(ast["frames"]) != expected["frames"]:
                errors.append(Problem("FIXTURE_FACT_MISMATCH", "Parsed event/frame counts differ from independently recorded facts").as_dict())
            named_events = {r["attributes"]["id"]: {"time": f["time"], **r} for f in ast["frames"] for r in f["records"] if r["record"] == "event" and "id" in r["attributes"]}
            for fact in fixture_context.get("event_facts", {}).get(example["name"], []):
                actual = named_events.get(fact["id"], {})
                fields_match = all(actual.get(key) == value for key, value in fact.items() if key not in ("id", "attributes"))
                attributes_match = all(actual.get("attributes", {}).get(key) == value for key, value in fact.get("attributes", {}).items())
                if not fields_match or not attributes_match:
                    errors.append(Problem("FIXTURE_FACT_MISMATCH", f"Independent event fact differs for {fact['id']}").as_dict())
            ast_path = f"examples/ast/{example['name']}.json"
            if not errors:
                write_json(ast_path, ast)
                artifacts.append(ast_path)
        else:
            ast_path = None
        results.append({"path": example["file"], "expected": "valid", "passed": not errors and not warnings, "ast": ast_path, "diagnostics": errors, "warnings": warnings})
        artifacts.append(example["file"])
    negative = load("examples/sfs-invalid/manifest.json")
    for case in negative["cases"]:
        source = (ROOT / case["path"]).read_text(encoding="utf-8")
        context = fragment_bindings(fixture_context["standalone_fragment_aliases"])
        _, errors, warnings = validate_source(source, language, ast_validator, context)
        codes = {error["code"] for error in errors}
        results.append({"path": case["path"], "expected": case["expected_code"], "passed": case["expected_code"] in codes, "diagnostics": errors, "warnings": warnings})
        artifacts.append(case["path"])
    # Acceptance probes exercise distinctions that invalid fixtures cannot prove.
    probes = [
        ("unequal_chord_durations_are_legal", "!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\nv C4:1 chord=c\nv E4:2 chord=c\n.\n!end at=2\n", None),
        ("written_duration_difference_is_preserved", "!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\nv C4:1 w=2\n.\n!end at=1\n", "WRITTEN_DURATION_MISMATCH"),
        ("normal_type_and_dots_remain_distinct", "!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\nv C4:1 w=8 dots=1 tm=3:2 tm_normal_type=4 tm_normal_dots=1\n.\n!end at=1\n", None),
    ]
    for name, source, expected_warning in probes:
        _, errors, warnings = validate_source(source, language, ast_validator)
        passed = not errors and ((expected_warning is None and not warnings) or expected_warning in {x["code"] for x in warnings})
        results.append({"probe": name, "expected_warning": expected_warning, "passed": passed, "diagnostics": errors, "warnings": warnings})
    auxiliary = []
    manifest = load("examples/auxiliary/manifest.json")
    for case in manifest["cases"]:
        data = load(case["path"])
        reference = case["schema"] + ("#/$defs/" + case["definition"] if "definition" in case else "")
        validator = jsonschema.Draft202012Validator({"$ref": reference}, registry=registry)
        schema_errors = [error.message for error in validator.iter_errors(data)]
        semantic_errors = [] if schema_errors else auxiliary_semantics(data, language, ast_validator, fixture_context)
        expected = case["expectation"]
        passed = bool(schema_errors) if expected == "schema_invalid" else (not schema_errors and (bool(semantic_errors) if expected == "schema_valid_semantically_invalid" else not semantic_errors))
        auxiliary.append({"path": case["path"], "expected": expected, "passed": passed, "schema_errors": schema_errors, "semantic_diagnostics": semantic_errors})
        artifacts.append(case["path"])
    artifacts.extend(["examples/sfs-invalid/manifest.json", "examples/auxiliary/manifest.json", "validation/README.md"])
    artifacts.extend(str(path.relative_to(ROOT)) for path in sorted((ROOT / "schemas").glob("*.schema.json")))
    failures = sum(not item["passed"] for item in results + auxiliary) + bool(descriptor_errors)
    report = {
        "report_kind": "finite-design-package-qa",
        "format_version": language["format_version"],
        "result": "passed" if failures == 0 else "failed",
        "production_parser": False,
        "ebnf_compiled_or_fully_verified": False,
        "rust_built_or_run": False,
        "renderer_or_mcp_executed": False,
        "annotation_store_implemented": False,
        "dependencies": {name: importlib.metadata.version(name) for name in ("jsonschema", "referencing")},
        "descriptor_schema_errors": descriptor_errors,
        "core_cases": results,
        "auxiliary_cases": auxiliary,
        "summary": {"core_cases": len(results), "auxiliary_cases": len(auxiliary), "failed_cases": int(failures)},
        "scope": "Bundled ordinary core examples, named negative fixtures and documented fixture-local checks only; see validation/README.md.",
        "limitations": [
            "Handwritten finite reader; the EBNF is recorded by hash, not compiled or proven equivalent.",
            "Ordinary common-clock core only; no unknown-extension interpretation or full symbol/instrument catalog validation.",
            "Written-duration arithmetic is an illustrative consistency warning, never silent correction or a universal rejection rule.",
            "Tie pitch comparison is nominal and limited to fixtures without changing transposition/tuning context.",
            "Auxiliary reference checks use explicit synthetic fixture context, not a live score/render database.",
            "No MusicXML, renderer, editor, MCP transport, Rust, audio or annotation-store implementation is validated."
        ],
        "inputs_sha256": {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in sorted(set(artifacts))},
    }
    write_json("validation/format-report.json", report)
    print(f"Package QA: {report['result']}; {len(results)} core cases, {len(auxiliary)} auxiliary cases, {int(failures)} failures.")
    for item in results + auxiliary:
        if not item["passed"]:
            print(json.dumps(item, ensure_ascii=False))
    return 0 if failures == 0 else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Problem as exc:
        print(json.dumps(exc.as_dict()), file=sys.stderr)
        sys.exit(2)
