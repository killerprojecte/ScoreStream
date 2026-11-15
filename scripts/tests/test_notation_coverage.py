"""Offline integrity, context and false-claim regressions (Python standard library only)."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import notation_coverage as n


class CoverageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = n.load_catalog(n.DATA)
        cls.ledger = n.read_json(n.DATA / "coverage.json")

    def test_pinned_counts_are_reference_counts_not_support(self):
        result = n.report(self.catalog, self.ledger, n.check(self.catalog, self.ledger), True)
        self.assertEqual(result["inventory_counts"]["element"], 480)
        self.assertEqual(result["distinct_musicxml_element_names"], 442)
        self.assertEqual(result["inventory_counts"]["attribute"], 352)
        self.assertEqual(result["inventory_counts"]["enumeration"], 620)
        self.assertEqual(result["inventory_counts"]["glyph"], 2932)
        self.assertEqual(result["inventory_counts"]["range"], 131)
        self.assertEqual(result["inventory_counts"]["class"], 85)
        self.assertGreater(len(result["unassessed_inventory_refs"]), 4000)
        self.assertNotIn("support_percentage", result)

    def test_reused_tags_never_collapse_context(self):
        for name, expected in (("tie", 2), ("cue", 2), ("fermata", 2), ("dynamics", 2)):
            matches = [e for e in self.catalog["entries"] if e["kind"] == "element" and e["name"] == name]
            self.assertEqual(len(matches), expected)
            self.assertEqual(len({e["id"] for e in matches}), expected)
        linked = next(f for f in self.ledger["features"] if f["id"] == "note-fermatas")["refs"]
        self.assertTrue(all("complexType:notations[1]" in ref for ref in linked))

    def test_no_blanket_supported_or_glyph_to_playback_promotion(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["features"][0]["layers"]["render"]["status"] = "supported"
        with self.assertRaisesRegex(ValueError, "invalid status"):
            n.check(self.catalog, ledger)
        roles = n.check(self.catalog, self.ledger)
        report = n.report(self.catalog, self.ledger, roles, True)
        self.assertIn("smufl:glyph:accidentalSharp", report["unassessed_inventory_refs"])

    def test_missing_layer_and_unscoped_claim_fail(self):
        for mutation in (lambda f: f["layers"].pop("export"), lambda f: f.update(scope="")):
            ledger = copy.deepcopy(self.ledger)
            mutation(ledger["features"][0])
            with self.assertRaises(ValueError):
                n.check(self.catalog, ledger)

    def test_positive_claim_needs_real_positive_test_and_source(self):
        ledger = copy.deepcopy(self.ledger)
        layer = ledger["features"][0]["layers"]["model"]
        layer["evidence"] = ["notes.reject"]
        with self.assertRaisesRegex(ValueError, "positive evidence"):
            n.check(self.catalog, ledger)
        layer["evidence"] = ["notes.roundtrip"]
        layer["implementation"] = []
        with self.assertRaisesRegex(ValueError, "implementation required"):
            n.check(self.catalog, ledger)

    def test_stale_test_and_helper_function_references_fail(self):
        for name in ("deleted_test", "normalized"):
            ledger = copy.deepcopy(self.ledger)
            ledger["evidence"]["notes.roundtrip"]["test"] = name
            with self.assertRaisesRegex(ValueError, "exact Rust test missing"):
                n.check(self.catalog, ledger)

    def test_bare_tag_unknown_ref_and_duplicate_ids_fail(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["features"][0]["refs"] = ["fermata"]
        with self.assertRaisesRegex(ValueError, "context-free"):
            n.check(self.catalog, ledger)
        catalog = copy.deepcopy(self.catalog)
        catalog["entries"].append(catalog["entries"][0])
        with self.assertRaisesRegex(ValueError, "duplicate catalog"):
            n.check(catalog, self.ledger)

    def test_shard_tampering_is_detected(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            n.write_catalog(self.catalog, directory)
            self.assertEqual(n.load_catalog(directory), self.catalog)
            shard = directory / n.read_json(directory / "catalog.json")["entry_files"][0]["file"]
            shard.write_text("[]")
            with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
                n.load_catalog(directory)

    def test_paths_cannot_escape_checkout(self):
        with self.assertRaisesRegex(ValueError, "unsafe"):
            n.safe_path(n.ROOT, "../../../../etc/passwd")

    def test_regenerator_preserves_ambiguity_and_expanded_facets_without_network(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            data = {
                "musicxml.xsd": b'''<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"><xs:complexType name="note"><xs:choice><xs:sequence><xs:element name="cue" minOccurs="0"/></xs:sequence><xs:sequence><xs:element name="cue"/></xs:sequence></xs:choice><xs:attributeGroup ref="print-style"/></xs:complexType><xs:simpleType name="limit"><xs:restriction base="xs:integer"><xs:minInclusive value="1"/><xs:maxInclusive value="8"/></xs:restriction></xs:simpleType></xs:schema>''',
                "glyphnames.json": b'{"note":{"codepoint":"U+E000"}}',
                "ranges.json": b'{"notes":{"glyphs":["note"],"range_start":"U+E000","range_end":"U+E00F"}}',
                "classes.json": b'{"notes":["note"]}',
            }
            lock = {"sources": [{"file": name, "sha256": n.digest(content)} for name, content in data.items()]}
            for name, content in data.items():
                (directory / name).write_bytes(content)
            compact = n.regenerate(directory, lock)
            cues = [e for e in compact["entries"] if e["name"] == "cue"]
            self.assertEqual(len({e["id"] for e in cues}), 2)
            self.assertNotIn("musicxml_schema", compact)
            expanded = n.regenerate(directory, lock, expanded=True)
            tree = json.dumps(expanded["musicxml_schema"])
            self.assertIn('"minInclusive"', tree)
            self.assertIn('"maxInclusive"', tree)
            self.assertIn('"ref": "print-style"', tree)
            (directory / "musicxml.xsd").write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "upstream SHA-256 mismatch"):
                n.regenerate(directory, lock)

    def test_zero_tests_or_many_tests_do_not_count_as_exact_receipt(self):
        for output in ("test result: ok. 0 passed; 0 failed; 0 ignored;", "test result: ok. 11 passed; 0 failed; 0 ignored;"):
            fake = type("Result", (), {"returncode": 0, "stdout": output})()
            with patch.object(n.subprocess, "run", return_value=fake), patch("builtins.print"):
                with self.assertRaisesRegex(ValueError, "no passing test"):
                    n.run_evidence(self.ledger, ["measured-ties"])

    def test_parameterized_visual_evidence_checks_template_and_exact_case(self):
        ledger = copy.deepcopy(self.ledger)
        evidence = ledger["evidence"]["accidentals.render.natural"]
        self.assertIn("'natural' on 'C4'", evidence["test"])
        self.assertIn("$value", evidence["template"])
        n.check(self.catalog, ledger)
        evidence["template"] = "missing parameterized declaration"
        with self.assertRaisesRegex(ValueError, "exact Vitest declaration missing"):
            n.check(self.catalog, ledger)

    def test_unassessed_profile_cannot_report_zero_tests_as_passed(self):
        with self.assertRaisesRegex(ValueError, "no executable evidence"):
            n.run_evidence(self.ledger, ["ornaments"])

    def test_all_articulation_choices_remain_bounded_and_visible(self):
        profile = next(f for f in self.ledger["features"] if f["id"] == "articulations")
        choices = profile["choice_inventory"]["entries"]
        self.assertEqual(len(choices), 17)
        self.assertEqual({c["name"] for c in choices if c["import_status"] == "partial"},
                         {"accent", "staccato", "staccatissimo", "tenuto"})
        self.assertEqual({c["name"] for c in choices if c["import_status"] == "blocked"}, {
            "breath-mark", "caesura", "detached-legato", "doit", "falloff",
            "other-articulation", "plop", "scoop", "soft-accent", "spiccato",
            "stress", "strong-accent", "unstress",
        })
        report = n.report(self.catalog, self.ledger, n.check(self.catalog, self.ledger))
        self.assertEqual(report["choice_import_counts"]["articulations"], {"blocked": 13, "partial": 4})
        self.assertTrue(all(c["scope_class"] and c["why_incomplete"] for c in choices))

    def test_authored_family_remains_opt_in_with_bounded_render_and_no_playback_credit(self):
        by_id = {f["id"]: f for f in self.ledger["features"]}
        authored = by_id["authored-articulations"]
        legacy = by_id["articulations"]
        self.assertIn("AuthoredV1", authored["scope"])
        self.assertIn("LegacySimple", legacy["scope"])
        self.assertEqual(len(authored["choice_inventory"]["entries"]), 17)
        self.assertTrue(all(c["import_status"] == "partial" for c in authored["choice_inventory"]["entries"]))
        for layer in ["model", "import", "export"]:
            self.assertEqual(authored["layers"][layer]["status"], "partial")
        self.assertEqual(authored["layers"]["render"]["status"], "partial")
        self.assertEqual(authored["layers"]["playback"]["status"], "blocked")
        self.assertIn("authored-articulations.engraving.all-ordinary", authored["layers"]["render"]["evidence"])
        self.assertIn("authored-articulations.render-reject", authored["layers"]["render"]["evidence"])
        for sign in ["spiccato", "stress", "unstress"]:
            self.assertIn(sign, authored["layers"]["render"]["detail"])
        self.assertIn("explicit placement", authored["layers"]["render"]["detail"])
        report = n.report(self.catalog, self.ledger, n.check(self.catalog, self.ledger))
        self.assertEqual(report["choice_import_counts"]["authored-articulations"], {"partial": 17})
        self.assertEqual(report["choice_import_counts"]["articulations"], {"blocked": 13, "partial": 4})

    def test_pause_probes_are_rejection_evidence_not_runtime_capability_credit(self):
        authored = next(f for f in self.ledger["features"] if f["id"] == "authored-articulations")
        receipts = [e for e in authored["layers"]["render"]["evidence"] if ".pause-probes." in e]
        self.assertEqual(len(receipts), 5)
        self.assertTrue(all(self.ledger["evidence"][e]["assertion"] == "rejection" for e in receipts))
        self.assertIn("do not enable breath/caesura preview", authored["layers"]["render"]["detail"])
        self.assertIn("entirely withheld", authored["limitations"])
        self.assertEqual(authored["layers"]["render"]["status"], "partial")
        self.assertEqual(authored["layers"]["playback"]["status"], "blocked")

    def test_authored_runtime_rejection_cannot_be_promoted_as_implementation(self):
        for layer in ["render", "playback"]:
            ledger = copy.deepcopy(self.ledger)
            profile = next(f for f in ledger["features"] if f["id"] == "authored-articulations")
            profile["layers"][layer]["status"] = "partial"
            profile["layers"][layer]["evidence"] = ["authored-articulations.render-reject"]
            with self.assertRaisesRegex(ValueError, "positive evidence"):
                n.check(self.catalog, ledger)

    def test_choice_inventory_cannot_omit_duplicate_or_rename_declarations(self):
        for mutation in (lambda entries: entries.pop(),
                         lambda entries: entries.append(copy.deepcopy(entries[0])),
                         lambda entries: entries[0].update(name="unreviewed-alias"),
                         lambda entries: entries[0].update(schema_type="wrong-type")):
            ledger = copy.deepcopy(self.ledger)
            profile = next(f for f in ledger["features"] if f["id"] == "articulations")
            mutation(profile["choice_inventory"]["entries"])
            with self.assertRaises(ValueError):
                n.check(self.catalog, ledger)

    def test_choice_claims_require_scope_reason_and_matching_evidence(self):
        for mutation in (lambda c: c.update(scope_class=""),
                         lambda c: c.update(why_incomplete=""),
                         lambda c: c.update(evidence=["articulations.roundtrip"]),
                         lambda c: c.update(import_status="supported")):
            ledger = copy.deepcopy(self.ledger)
            profile = next(f for f in ledger["features"] if f["id"] == "articulations")
            choice = next(c for c in profile["choice_inventory"]["entries"] if c["import_status"] == "blocked")
            mutation(choice)
            with self.assertRaises(ValueError):
                n.check(self.catalog, ledger)

    def test_seeded_profiles_remain_independently_staged(self):
        by_id = {f["id"]: f for f in self.ledger["features"]}
        for name in ["authored-slurs", "authored-beams", "printed-accidentals", "barline-repeats", "measured-ties"]:
            self.assertIn(name, by_id)
        self.assertEqual(by_id["authored-slurs"]["layers"]["import"]["status"], "partial")
        self.assertEqual(by_id["authored-slurs"]["layers"]["render"]["status"], "blocked")
        self.assertEqual(by_id["authored-slurs"]["layers"]["playback"]["status"], "blocked")
        self.assertTrue(all(layer["status"] == "partial" for layer in by_id["articulations"]["layers"].values()))
        self.assertTrue(all(layer["status"] == "unassessed" for layer in by_id["ornaments"]["layers"].values()))


if __name__ == "__main__":
    unittest.main()
