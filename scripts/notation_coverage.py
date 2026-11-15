#!/usr/bin/env python3
"""Offline, evidence-linked notation inventory. This is NOT an importer or XSD validator."""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "docs" / "notation"
LAYERS = ("model", "import", "export", "render", "playback")
STATUSES = {"partial", "blocked", "unassessed", "not_applicable"}
ROLES = {"musical_data", "structure", "metadata", "layout", "glyph", "unclassified"}
XSD = "{http://www.w3.org/2001/XMLSchema}"


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def encode(data):
    return json.dumps(data, ensure_ascii=False, indent=2, sort_keys=True) + "\n"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def regenerate(source_dir, lock, expanded=False):
    """Extract facts only, with schema declaration paths rather than collapsed tag names."""
    sources = {}
    for source in lock["sources"]:
        data = (source_dir / source["file"]).read_bytes()
        require(digest(data) == source["sha256"], f"{source['file']}: upstream SHA-256 mismatch")
        sources[source["file"]] = data
    root = ET.fromstring(sources["musicxml.xsd"])
    entries = []

    def walk(node, path):
        counts = Counter()
        for child in node:
            tag = child.tag.removeprefix(XSD)
            if tag in {"annotation", "documentation"}:
                continue
            label = child.get("name", child.get("ref", child.get("value", "")))
            key = f"{tag}:{label}" if label else tag
            counts[key] += 1
            context = f"{path}/{key}[{counts[key]}]"
            if tag in {"element", "attribute", "enumeration"} or (
                tag in {"simpleType", "complexType", "group", "attributeGroup"} and child.get("name")
            ):
                entries.append({
                    "id": f"musicxml:{context}", "kind": tag, "name": label,
                    "properties": dict(sorted(child.attrib.items())),
                })
            walk(child, context)
    walk(root, "schema")
    glyphs = json.loads(sources["glyphnames.json"])
    for name, data in sorted(glyphs.items()):
        entries.append({"id": f"smufl:glyph:{name}", "kind": "glyph",
                        "name": name, "codepoint": data["codepoint"],
                        **({"alternate_codepoint": data["alternateCodepoint"]} if "alternateCodepoint" in data else {})})
    for filename, kind in (("ranges.json", "range"), ("classes.json", "class")):
        for name, data in sorted(json.loads(sources[filename]).items()):
            members = data["glyphs"] if kind == "range" else data
            require(all(member in glyphs for member in members), f"{filename}:{name}: unknown glyph")
            entries.append({"id": f"smufl:{kind}:{name}", "kind": kind,
                            "name": name, "members": sorted(members),
                            **({"start": data["range_start"], "end": data["range_end"]} if kind == "range" else {})})
    def schema_node(node):
        # Preserve compositors, references, facets, defaults and fixed/use values.
        # This tree is reference data, NOT an implementation of XSD validation.
        return {"kind": node.tag.removeprefix(XSD), "properties": dict(sorted(node.attrib.items())),
                "children": [schema_node(child) for child in node if child.tag != XSD + "annotation"]}
    return {"schema_version": 1, "purpose": "reference_inventory_not_implementation_support",
            **({"musicxml_schema": schema_node(root)} if expanded else {}),
            "entries": sorted(entries, key=lambda entry: entry["id"])}



def write_catalog(catalog, directory):
    """Deterministic <70 KB shards for offline checks and size-limited review tools."""
    directory.mkdir(parents=True, exist_ok=True)
    index = {key: value for key, value in catalog.items() if key != "entries"}
    index["entry_files"] = []
    chunks, chunk, size = [], [], 0
    for row in catalog["entries"]:
        line = json.dumps(row, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
        if chunk and size + len(line.encode()) + 2 > 68000:
            chunks.append(chunk)
            chunk, size = [], 0
        chunk.append(line)
        size += len(line.encode()) + 2
    if chunk:
        chunks.append(chunk)
    for number, lines in enumerate(chunks):
        filename = f"catalog/entries-{number:02d}.json"
        data = ("[\n" + ",\n".join(lines) + "\n]\n").encode()
        (directory / filename).parent.mkdir(parents=True, exist_ok=True)
        (directory / filename).write_bytes(data)
        index["entry_files"].append({"file": filename, "sha256": digest(data)})
    (directory / "catalog.json").write_text(encode(index), encoding="utf-8")


def load_catalog(directory):
    index = read_json(directory / "catalog.json")
    result = {key: value for key, value in index.items() if key != "entry_files"}
    result["entries"] = []
    for ref in index["entry_files"]:
        data = safe_path(directory, ref["file"]).read_bytes()
        require(digest(data) == ref["sha256"], f"{ref['file']}: catalog shard SHA-256 mismatch")
        result["entries"].extend(json.loads(data))
    return result


def safe_path(root, relative):
    path = (root / relative).resolve()
    require(path.is_relative_to(root.resolve()) and path.is_file(), f"missing or unsafe repository path: {relative}")
    return path


def check(catalog, ledger, root=ROOT):
    require(catalog.get("schema_version") == 1 and ledger.get("schema_version") == 1, "unknown schema version")
    entries = catalog["entries"]
    ids = {entry["id"] for entry in entries}
    require(len(ids) == len(entries), "duplicate catalog ID")
    for entry in entries:
        if entry["kind"] in {"range", "class"}:
            require(all(f"smufl:glyph:{m}" in ids for m in entry["members"]), f"{entry['id']}: unknown member")
    evidence = ledger["evidence"]
    for eid, item in evidence.items():
        source = safe_path(root, item["file"]).read_text(encoding="utf-8")
        test = item["test"]
        if item["runner"] == "cargo":
            require(item["file"] == f"crates/{item['package']}/tests/{item['target']}.rs", f"{eid}: cargo target/path mismatch")
            # A helper function or a stale substring is not a test receipt.
            pattern = rf"#\[test\]\s*(?:#\[[^\]]+\]\s*)*fn\s+{re.escape(test)}\s*\("
            require(re.search(pattern, source) is not None, f"{eid}: exact Rust test missing: {test}")
        elif item["runner"] == "vitest":
            require(item["file"].startswith("desktop/tests/"), f"{eid}: invalid Vitest path")
            declaration = item.get("template", test)
            call = r"(?:it|test)\.each\([^;\n]+\)" if "template" in item else r"(?:it|test)"
            pattern = rf"\b{call}\(\s*(['\"]){re.escape(declaration)}\1\s*,"
            require(re.search(pattern, source) is not None, f"{eid}: exact Vitest declaration missing: {declaration}")
        else:
            raise ValueError(f"{eid}: unknown evidence runner")
    used = set()
    features = set()
    roles = {}
    for group in ledger["classifications"]:
        require(group["role"] in ROLES - {"unclassified", "glyph"}, "invalid classification role")
        require(bool(group["reason"]) and bool(group["refs"]), "empty classification")
        for ref in group["refs"]:
            require(ref in ids, f"unknown classified declaration: {ref}")
            require(ref not in roles, f"duplicate classification: {ref}")
            roles[ref] = group["role"]
    for feature in ledger["features"]:
        fid = feature["id"]
        require(fid not in features, f"duplicate feature: {fid}")
        features.add(fid)
        require(feature["scope"].strip() and feature["limitations"].strip(), f"{fid}: scope and limitations required")
        require(feature["refs"] and len(feature["refs"]) == len(set(feature["refs"])), f"{fid}: empty or duplicate refs")
        require(set(feature["refs"]) <= ids, f"{fid}: unknown or context-free reference")
        require(set(feature["layers"]) == set(LAYERS), f"{fid}: all five independent layers required")
        for layer, state in feature["layers"].items():
            require(state["status"] in STATUSES, f"{fid}/{layer}: invalid status (no blanket supported state)")
            require(state["detail"].strip(), f"{fid}/{layer}: missing detail")
            refs = state["evidence"]
            require(set(refs) <= set(evidence), f"{fid}/{layer}: unknown evidence")
            if state["status"] in {"partial", "blocked"}:
                require(refs and state["implementation"], f"{fid}/{layer}: evidence and implementation required")
            if state["status"] == "partial":
                require(any(evidence[e]["assertion"] == "positive" for e in refs), f"{fid}/{layer}: partial needs positive evidence")
            if state["status"] == "blocked":
                require(any(evidence[e]["assertion"] == "rejection" for e in refs), f"{fid}/{layer}: blocked needs rejection evidence")
            for source in state["implementation"]:
                safe_path(root, source)
            used.update(refs)
    require(used == set(evidence), "orphan evidence entry")
    for eid, item in evidence.items():
        require(item["assertion"] in {"positive", "rejection"}, f"{eid}: invalid evidence assertion")
    return roles


def report(catalog, ledger, roles, include_gaps=False):
    links = {}
    for feature in ledger["features"]:
        for ref in feature["refs"]:
            links.setdefault(ref, []).append(feature["id"])
    role_counts = Counter()
    gaps = []
    for entry in catalog["entries"]:
        role = "glyph" if entry["kind"] == "glyph" else roles.get(entry["id"], "unclassified")
        role_counts[role] += 1
        if entry["id"] not in links:
            gaps.append(entry["id"])
    result = {
        "purpose": "inventory_and_bounded_claims_not_support_percentage",
        "inventory_counts": dict(sorted(Counter(e["kind"] for e in catalog["entries"]).items())),
        "distinct_musicxml_element_names": len({e["name"] for e in catalog["entries"] if e["kind"] == "element"}),
        "reviewed_role_counts": dict(sorted(role_counts.items())),
        "entries_without_direct_feature_link": len(gaps),
        "layer_counts": {layer: dict(sorted(Counter(f["layers"][layer]["status"] for f in ledger["features"]).items())) for layer in LAYERS},
        "features": ledger["features"],
        "warning": "A feature link does not cover the referenced declaration's entire value, attribute or context space. Range/class membership is not glyph rendering evidence. Static checks do not execute behavior tests.",
    }
    if include_gaps:
        result["unassessed_inventory_refs"] = gaps
    return result


def run_evidence(ledger, feature_ids):
    selected = [feature for feature in ledger["features"] if not feature_ids or feature["id"] in feature_ids]
    require(not feature_ids or set(feature_ids) <= {f["id"] for f in selected}, "unknown --feature")
    ids = sorted({eid for feature in selected for layer in feature["layers"].values() for eid in layer["evidence"]})
    require(bool(ids), "selected profiles have no executable evidence; unassessed is not a passing test")
    for eid in ids:
        item = ledger["evidence"][eid]
        if item["runner"] == "cargo":
            argv = ["cargo", "test", "--locked", "-p", item["package"], "--test", item["target"], item["test"], "--", "--exact"]
            cwd = ROOT
            marker = r"test result: ok\. 1 passed; 0 failed; 0 ignored;"
        else:
            argv = ["npm", "test", "--", str(Path(item["file"]).relative_to("desktop")), "--testNamePattern", f"^{re.escape(item['suite'])} {re.escape(item['test'])}$", "--no-color"]
            cwd = ROOT / "desktop"
            marker = r"Tests\s+1 passed(?:\s|\()"
        print(f"{eid}: {json.dumps(argv)}", flush=True)
        result = subprocess.run(argv, cwd=cwd, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
        print(result.stdout, end="", flush=True)
        require(result.returncode == 0 and re.search(marker, result.stdout) is not None, f"{eid}: evidence failed or no passing test was executed")
    print(f"Executed {len(ids)} exact evidence tests. This is not the full regression suite.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["check", "report", "regenerate", "test-evidence"])
    parser.add_argument("--source-dir", type=Path, help="local pinned upstream files; never downloaded implicitly")
    parser.add_argument("--expanded-output", type=Path, help="regenerate only: also write the complete derived XSD tree (compositors, facets, references)")
    parser.add_argument("--gaps", action="store_true", help="include all inventory IDs lacking direct bounded feature links")
    parser.add_argument("--feature", action="append", default=[], help="select evidence by bounded feature ID")
    args = parser.parse_args()
    require(not args.expanded_output or args.command == "regenerate", "--expanded-output requires regenerate")
    require(not args.feature or args.command == "test-evidence", "--feature requires test-evidence")
    require(not args.gaps or args.command == "report", "--gaps requires report")
    lock = read_json(DATA / "sources.json")
    if args.command == "regenerate":
        require(args.source_dir is not None, "regenerate requires --source-dir")
        generated = encode(regenerate(args.source_dir, lock))
        require(digest(generated.encode()) == lock["catalog_sha256"], "generated inventory differs from pinned catalog; review before updating lock")
        write_catalog(json.loads(generated), DATA)
        if args.expanded_output:
            args.expanded_output.write_text(encode(regenerate(args.source_dir, lock, expanded=True)), encoding="utf-8")
        print("Regenerated byte-identical pinned inventory.")
        return
    catalog = load_catalog(DATA)
    require(digest(encode(catalog).encode()) == lock["catalog_sha256"], "catalog hash mismatch; regenerate from pinned sources")
    ledger = read_json(DATA / "coverage.json")
    roles = check(catalog, ledger)
    if args.source_dir:
        require(regenerate(args.source_dir, lock) == catalog, "catalog differs from pinned upstream extraction")
    if args.command == "check":
        print(f"Inventory and evidence references valid: {len(catalog['entries'])} reference entries, {len(ledger['features'])} bounded profiles. Behavior tests NOT executed.")
    elif args.command == "report":
        print(encode(report(catalog, ledger, roles, args.gaps)), end="")
    else:
        run_evidence(ledger, args.feature)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, TypeError, OSError, ET.ParseError) as error:
        print(f"notation coverage: {error}", file=sys.stderr)
        sys.exit(1)
