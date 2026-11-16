#!/usr/bin/env python3
"""Run the real converter on all four pinned Beethoven Fifth XML/MXL pairs."""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import subprocess
import xml.etree.ElementTree as ET
from zipfile import ZipFile

from audit_beethoven_navigation import HASHES


def archive_score_digest(path):
    with ZipFile(path) as archive:
        metadata = ET.fromstring(archive.read("META-INF/container.xml"))
        entry = metadata.find(".//{*}rootfile")
        if entry is None:
            raise ValueError(f"{path}: no MusicXML rootfile")
        return hashlib.sha256(archive.read(entry.attrib["full-path"])).hexdigest()


def run(cli, score):
    completed = subprocess.run(
        [str(cli), "convert", "musicxml", "sfs", str(score)], capture_output=True, check=False
    )
    report = json.loads(completed.stderr) if completed.stderr else {"diagnostics": []}
    diagnostics = report["diagnostics"]
    fingerprint = [(d["effect"], d["feature"], d["path"]) for d in diagnostics]
    counts = Counter((effect, feature) for effect, feature, _ in fingerprint)
    return {
        "exit_code": completed.returncode,
        "output_bytes": len(completed.stdout),
        "output_sha256": hashlib.sha256(completed.stdout).hexdigest() if completed.stdout else None,
        "diagnostic_count": len(diagnostics),
        "diagnostics_truncated": any(d["feature"] == "diagnostics.truncated" for d in diagnostics),
        "diagnostics": [
            {"effect": effect, "feature": feature, "reported_count": count}
            for (effect, feature), count in counts.most_common()
        ],
    }, completed.stdout, fingerprint


def check(cli, inputs):
    result = []
    for movement, expected_hash in HASHES.items():
        stem = f"Beethoven_Op.67_{movement}"
        xml = inputs / (stem + ".musicxml")
        mxl = inputs / (stem + ".mxl")
        if hashlib.sha256(xml.read_bytes()).hexdigest() != expected_hash:
            raise ValueError(f"{xml}: input differs from the pinned original")
        if archive_score_digest(mxl) != expected_hash:
            raise ValueError(f"{mxl}: container score differs from the pinned original")
        xml_report, xml_sfs, xml_diagnostics = run(cli, xml)
        mxl_report, mxl_sfs, mxl_diagnostics = run(cli, mxl)
        parity = (
            xml_report["exit_code"] == mxl_report["exit_code"]
            and xml_sfs == mxl_sfs
            and xml_diagnostics == mxl_diagnostics
        )
        imported = (
            parity
            and xml_report["exit_code"] == 0
            and bool(xml_sfs)
            and not xml_report["diagnostics_truncated"]
        )
        result.append({
            "movement": movement,
            "source_sha256": expected_hash,
            "imported": imported,
            "xml_mxl_parity": parity,
            "xml": xml_report,
            "mxl": mxl_report,
        })
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cli", type=Path, help="built ScoreStream CLI executable")
    parser.add_argument("inputs", type=Path, help="directory containing original MXL and XML pairs")
    args = parser.parse_args()
    result = check(args.cli.resolve(), args.inputs)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if all(row["imported"] for row in result) else 1


if __name__ == "__main__":
    raise SystemExit(main())
