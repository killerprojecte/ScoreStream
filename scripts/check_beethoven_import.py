#!/usr/bin/env python3
"""Run the real converter on all four pinned Beethoven Fifth XML/MXL pairs."""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
from compare_musicxml_transcript import compare, TranscriptError
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


def run(cli, score, irregular_contract=None):
    command = [str(cli), "convert", "musicxml", "sfs", str(score)]
    if irregular_contract is not None:
        command += ["--musicxml-irregular-measures", str(irregular_contract)]
    completed = subprocess.run(command, capture_output=True, check=False)
    report = json.loads(completed.stderr) if completed.stderr else {"diagnostics": []}
    diagnostics = report["diagnostics"]
    fingerprint = [(d["effect"], d["feature"], d["path"]) for d in diagnostics]
    counts = Counter((effect, feature) for effect, feature, _ in fingerprint)
    return {
        "exit_code": completed.returncode,
        "output_bytes": len(completed.stdout),
        "output_sha256": hashlib.sha256(completed.stdout).hexdigest() if completed.stdout else None,
        "diagnostic_count": len(diagnostics),
        "first_error": next((d for d in diagnostics if d["effect"] in ("invalid", "unsupported")), None),
        "diagnostics_truncated": any(d["feature"] == "diagnostics.truncated" for d in diagnostics),
        "diagnostics": [
            {"effect": effect, "feature": feature, "reported_count": count}
            for (effect, feature), count in counts.most_common()
        ],
    }, completed.stdout, fingerprint


def verify_transcript(cli, source, sfs, contract):
    with tempfile.TemporaryDirectory(prefix="scorestream-transcript-") as directory:
        native = Path(directory) / "score.sfs"
        exported = Path(directory) / "export.musicxml"
        native.write_bytes(sfs)
        command = [str(cli), "convert", "sfs", "musicxml", str(native)]
        if contract is not None:
            command += ["--musicxml-irregular-measures", str(contract)]
        conversion = subprocess.run(command, capture_output=True, check=False)
        if conversion.returncode != 0:
            return {"status": "export_failed", "diagnostics": conversion.stderr.decode("utf-8", errors="replace")}
        exported.write_bytes(conversion.stdout)
        try:
            return {"status": "equal", **compare(source, exported)}
        except (TranscriptError, ET.ParseError, OSError) as error:
            return {"status": "unverified_or_different", "error": str(error)}


def check(cli, inputs, movement_i_contract=None, compare_events=False):
    result = []
    for movement, expected_hash in HASHES.items():
        stem = f"Beethoven_Op.67_{movement}"
        xml = inputs / (stem + ".musicxml")
        mxl = inputs / (stem + ".mxl")
        if hashlib.sha256(xml.read_bytes()).hexdigest() != expected_hash:
            raise ValueError(f"{xml}: input differs from the pinned original")
        if archive_score_digest(mxl) != expected_hash:
            raise ValueError(f"{mxl}: container score differs from the pinned original")
        contract = movement_i_contract if str(movement) == "1" else None
        xml_report, xml_sfs, xml_diagnostics = run(cli, xml, contract)
        mxl_report, mxl_sfs, mxl_diagnostics = run(cli, mxl, contract)
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
        event_check = (verify_transcript(cli, xml, xml_sfs, contract) if imported else {"status": "not_run_import_failed"}) if compare_events else {"status": "not_requested"}
        result.append({
            "movement": movement,
            "source_sha256": expected_hash,
            "imported": imported,
            "note_event_transcript": event_check,
            "xml_mxl_parity": parity,
            "xml": xml_report,
            "mxl": mxl_report,
        })
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cli", type=Path, help="built ScoreStream CLI executable")
    parser.add_argument("inputs", type=Path, help="directory containing original MXL and XML pairs")
    parser.add_argument("--movement-i-contract", type=Path, help="exact irregular-measure JSON array, applied only to pinned movement I")
    parser.add_argument("--compare-note-events", action="store_true", help="after successful authentic import, export and independently compare exact note-event transcripts")
    args = parser.parse_args()
    result = check(args.cli.resolve(), args.inputs, args.movement_i_contract, args.compare_note_events)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if all(row["imported"] and (not args.compare_note_events or row["note_event_transcript"]["status"] == "equal") for row in result) else 1


if __name__ == "__main__":
    raise SystemExit(main())
