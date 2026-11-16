"""Offline contract regression: python3 -m unittest discover -s design/tools -p test_notation_order_contract.py."""
import copy
import json
from pathlib import Path
import unittest

import jsonschema
from notation_order_contract import install_descriptor, install_schema

ROOT = Path(__file__).resolve().parents[1]


class MixedNotationContractTests(unittest.TestCase):
    def test_bundled_contracts_match_durable_installer(self):
        for filename, installer in [('spec/scorestream-language.json', install_descriptor),
                                    ('schemas/scorestream-ast.schema.json', install_schema)]:
            with self.subTest(filename=filename):
                bundled = json.loads((ROOT / filename).read_text())
                generated = copy.deepcopy(bundled)
                installer(generated)
                self.assertEqual(generated, bundled)

    def test_closed_child_shapes(self):
        schema = json.loads((ROOT / 'schemas/scorestream-ast.schema.json').read_text())
        jsonschema.Draft202012Validator.check_schema(schema)
        validator = jsonschema.Draft202012Validator({'$defs': schema['$defs'], '$ref': '#/$defs/Mark'})
        base = {'record': 'mark', 'kind': 'scorestream.notations.child', 'target': '#e',
                'attributes': {'event_index': 0, 'wrapper_index': 0, 'child_index': 0}}
        for kind, fields in [('articulations', {'group_index': 0}), ('tied', {'phase': 'stop'}),
                             ('slur', {'phase': 'start', 'relation': 's'}), ('fermata', {})]:
            with self.subTest(kind=kind):
                mark = {**base, 'attributes': {**base['attributes'], 'kind': kind, **fields}}
                validator.validate(mark)
                for key in fields:
                    missing = copy.deepcopy(mark)
                    del missing['attributes'][key]
                    self.assertTrue(list(validator.iter_errors(missing)))
                for key, value in [('group_index', 1), ('phase', 'start'), ('relation', 's'), ('foreign', True)]:
                    if key not in fields:
                        extra = copy.deepcopy(mark)
                        extra['attributes'][key] = value
                        self.assertTrue(list(validator.iter_errors(extra)))

class PinnedPrerequisiteTests(unittest.TestCase):
    def test_descriptor_refuses_stale_ordinals_and_exact_prerequisite_drift_without_mutation(self):
        from notation_order_contract import CONSTRAINT
        canonical = json.loads((ROOT / 'spec/scorestream-language.json').read_text())
        self.assertEqual(canonical['additional_constraints'][11], CONSTRAINT)
        baseline = copy.deepcopy(canonical)
        baseline['additional_constraints'].pop()
        del baseline['extensions']['registered']['scorestream.notations@1']
        for kind in ['scorestream.notations.event', 'scorestream.notations.child']:
            for field in ['marks', 'mark_fields']:
                del baseline['vocabulary'][field][kind]
        install_descriptor(baseline)
        self.assertEqual(baseline, canonical)
        cases = []
        stale = copy.deepcopy(canonical); stale['additional_constraints'] = stale['additional_constraints'][:9]; cases.append(stale)
        reordered = copy.deepcopy(canonical); reordered['additional_constraints'][9:11] = reversed(reordered['additional_constraints'][9:11]); cases.append(reordered)
        changed = copy.deepcopy(canonical); changed['additional_constraints'][10] += ' changed'; cases.append(changed)
        missing = copy.deepcopy(canonical); del missing['extensions']['registered']['scorestream.slur@1']; cases.append(missing)
        changed = copy.deepcopy(canonical); changed['vocabulary']['mark_fields']['scorestream.articulation.group']['required'] = []; cases.append(changed)
        for candidate in cases:
            before = copy.deepcopy(candidate)
            with self.assertRaises(ValueError): install_descriptor(candidate)
            self.assertEqual(candidate, before)

    def test_schema_refuses_missing_or_drifted_prerequisites_without_mutation(self):
        canonical = json.loads((ROOT / 'schemas/scorestream-ast.schema.json').read_text())
        missing = copy.deepcopy(canonical); missing['$defs']['Mark']['allOf'] = []
        changed = copy.deepcopy(canonical); changed['$defs']['Mark']['allOf'][3]['then']['description'] += ' changed'
        for candidate in [missing, changed]:
            before = copy.deepcopy(candidate)
            with self.assertRaises(ValueError): install_schema(candidate)
            self.assertEqual(candidate, before)


if __name__ == '__main__':
    unittest.main()
