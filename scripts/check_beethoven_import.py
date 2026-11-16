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


def run(cli, score, irregular_contract=None, continue_final_ending=False):
    command = [str(cli), "convert", "musicxml", "sfs", str(score)]
    if irregular_contract is not None:
        command += ["--musicxml-irregular-measures", str(irregular_contract)]
    if continue_final_ending:
        command += ["--unclosed-final-ending", "continue-to-movement-end"]
    completed = subprocess.run(command, capture_output=True, check=False)
    report = json.loads(completed.stderr) if completed.stderr else {"diagnostics": []}
    diagnostics = report["diagnostics"]
    fingerprint = [(d["effect"], d["feature"], d["path"], d.get("occurrences", 1)) for d in diagnostics]
    counts = Counter((effect, feature) for effect, feature, _, _ in fingerprint)
    occurrences = Counter()
    for effect, feature, _, count in fingerprint:
        occurrences[(effect, feature)] += count
    return {
        "exit_code": completed.returncode,
        "output_bytes": len(completed.stdout),
        "output_sha256": hashlib.sha256(completed.stdout).hexdigest() if completed.stdout else None,
        "diagnostic_count": len(diagnostics),
        "first_error": next((d for d in diagnostics if d["effect"] in ("invalid", "unsupported")), None),
        "diagnostics_truncated": any(d["feature"] == "diagnostics.truncated" for d in diagnostics),
        "diagnostics": [
            {"effect": effect, "feature": feature, "reported_count": count, "occurrence_count": occurrences[(effect, feature)]}
            for (effect, feature), count in counts.most_common()
        ],
    }, completed.stdout, fingerprint


def verify_transcript(cli, source, sfs, contract, continue_final_ending=False):
    with tempfile.TemporaryDirectory(prefix="scorestream-transcript-") as directory:
        native = Path(directory) / "score.sfs"
        exported = Path(directory) / "export.musicxml"
        native.write_bytes(sfs)
        command = [str(cli), "convert", "sfs", "musicxml", str(native)]
        if contract is not None:
            command += ["--musicxml-irregular-measures", str(contract)]
        if continue_final_ending:
            command += ["--unclosed-final-ending", "continue-to-movement-end"]
        conversion = subprocess.run(command, capture_output=True, check=False)
        if conversion.returncode != 0:
            return {"status": "export_failed", "diagnostics": conversion.stderr.decode("utf-8", errors="replace")}
        exported.write_bytes(conversion.stdout)
        try:
            return {"status": "equal", **compare(source, exported)}
        except (TranscriptError, ET.ParseError, OSError) as error:
            return {"status": "unverified_or_different", "error": str(error)}


def check(cli, inputs, movement_i_contract=None, compare_events=False, movement_iii_contract=None, movement_ii_contract=None, movement_iv_final_ending=False):
    result = []
    for movement, expected_hash in HASHES.items():
        stem = f"Beethoven_Op.67_{movement}"
        xml = inputs / (stem + ".musicxml")
        mxl = inputs / (stem + ".mxl")
        if hashlib.sha256(xml.read_bytes()).hexdigest() != expected_hash:
            raise ValueError(f"{xml}: input differs from the pinned original")
        if archive_score_digest(mxl) != expected_hash:
            raise ValueError(f"{mxl}: container score differs from the pinned original")
        contract = {1: movement_i_contract, 2: movement_ii_contract, 3: movement_iii_contract}.get(int(movement))
        final_ending = movement_iv_final_ending and str(movement) == "4"
        xml_report, xml_sfs, xml_diagnostics = run(cli, xml, contract, final_ending)
        mxl_report, mxl_sfs, mxl_diagnostics = run(cli, mxl, contract, final_ending)
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
        event_check = (verify_transcript(cli, xml, xml_sfs, contract, final_ending) if imported else {"status": "not_run_import_failed"}) if compare_events else {"status": "not_requested"}
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
    parser.add_argument("--movement-iii-contract", type=Path, help="exact irregular-measure JSON array, applied only to pinned movement III")
    parser.add_argument("--movement-ii-contract", type=Path, help="exact irregular-measure JSON array, applied only to pinned movement II")
    parser.add_argument("--movement-iv-continue-final-ending", action="store_true", help="explicit existing final-ending continuation policy, only for pinned IV; authored endpoint remains absent")
    args = parser.parse_args()
    result = check(args.cli.resolve(), args.inputs, args.movement_i_contract,
                   compare_events=args.compare_note_events,
                   movement_iii_contract=args.movement_iii_contract,
                   movement_ii_contract=args.movement_ii_contract,
                   movement_iv_final_ending=args.movement_iv_continue_final_ending)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if all(row["imported"] and (not args.compare_note_events or row["note_event_transcript"]["status"] == "equal") for row in result) else 1


if __name__ == "__main__":
    raise SystemExit(main())
