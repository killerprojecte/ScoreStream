"""Durable closed native contracts for scorestream.notations@1.

Targeted installer for the pinned current canonical contracts only. The legacy
full build_format_reference.py is stale and is deliberately NOT wired to this
module. Refuse prerequisite drift before mutating either contract.
"""
from copy import deepcopy
import hashlib
import json

EXTENSION = 'scorestream.notations@1'
EVENT = 'scorestream.notations.event'
CHILD = 'scorestream.notations.child'
INDEX = {'type': 'nonnegative_integer'}
FIELDS = {
    EVENT: {'event_index': INDEX},
    CHILD: {'event_index': INDEX, 'wrapper_index': INDEX, 'child_index': INDEX,
            'kind': {'type': 'enum', 'values': ['articulations', 'tied', 'slur', 'fermata']},
            'group_index': INDEX, 'phase': {'type': 'enum', 'values': ['start', 'stop']},
            'relation': {'type': 'identifier'}},
}
REQUIRED = {EVENT: ['event_index'], CHILD: ['event_index', 'wrapper_index', 'child_index', 'kind']}
BEHAVIOR = ('Requires scorestream.articulation@1 and at least one mixed measured pitched host. '
            'Explicit ordered child references retain wrapper boundaries, empty wrappers, and group order. '
            'Each group and every ordinary tied phase, authored slur endpoint and single fermata must occur exactly once. '
            'No legacy articulation overlap, dangling or duplicate reference, gapped index, foreign field or new tie semantics. '
            'Tied stop precedes start. Mixed ties require native endpoints without sound/print overrides; '
            'graphic-only, sound-only, continue engraving, let-ring and repeat-conditioned XML ties remain unsupported. '
            'All optical and performance paths fail closed until separately qualified.')

CONSTRAINT = 'scorestream.notations@1 requires authored articulation hosts and exact ordered wrapper/group/endpoint bijections. Existing ordinary ties, authored slurs and single fermatas retain their own semantics. No new sound or engraving policy is inferred; mixed preview and performance reject.'


BASE_CONSTRAINT_COUNT = 11
PREREQUISITE_EXTENSIONS = ['scorestream.beam@1', 'scorestream.slur@1', 'scorestream.articulation@1']
PREREQUISITE_MARKS = ['scorestream.beam', 'scorestream.articulation.event',
                      'scorestream.articulation.notations', 'scorestream.articulation.group',
                      'scorestream.articulation.item']

def _digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()).hexdigest()

def _descriptor_prerequisites(guide):
    return {'constraints': guide['additional_constraints'][:BASE_CONSTRAINT_COUNT],
            'extensions': {key: guide['extensions']['registered'][key] for key in PREREQUISITE_EXTENSIONS},
            'marks': {kind: {field: guide['vocabulary'][field][kind] for field in ['marks', 'mark_fields']}
                      for kind in PREREQUISITE_MARKS},
            'relations': {key: guide['vocabulary']['relations'][key] for key in ['tie', 'slur']}}

def _schema_prerequisites(schema):
    clauses = schema['$defs']['Mark']['allOf']
    return {kind: next(c for c in clauses if c.get('if', {}).get('properties', {}).get('kind', {}).get('const') == kind)
            for kind in PREREQUISITE_MARKS}

# Pinned after the authored-articulation discovery correction. These guards do
# not certify the historical full generator. Update deliberately with tests if
# the prerequisite contracts change; never append a new ordinal to stale output.
DESCRIPTOR_PREREQUISITES_SHA256 = '337748abf923bae7a701e922227e0fba52fa330b9213f8962cfd3d608a04c3ec'
SCHEMA_PREREQUISITES_SHA256 = 'b94bcf4d7c69c49678311b1a167619ae1021a0e563cd9c0b9dd10bfcc9ae54d3'

def _check_descriptor(guide):
    try:
        constraints = guide['additional_constraints']
        valid_tail = len(constraints) == BASE_CONSTRAINT_COUNT or (
            len(constraints) == BASE_CONSTRAINT_COUNT + 1 and constraints[-1] == CONSTRAINT)
        valid = valid_tail and _digest(_descriptor_prerequisites(guide)) == DESCRIPTOR_PREREQUISITES_SHA256
    except (KeyError, TypeError):
        valid = False
    if not valid:
        raise ValueError('Mixed notation installer requires the pinned canonical 11-constraint beam/slur/articulation baseline; refusing stale or drifted descriptor')

def _check_schema(schema):
    try:
        valid = _digest(_schema_prerequisites(schema)) == SCHEMA_PREREQUISITES_SHA256
    except (KeyError, TypeError, StopIteration):
        valid = False
    if not valid:
        raise ValueError('Mixed notation installer requires the pinned canonical beam/articulation schema; refusing stale or drifted schema')

def install_descriptor(guide):
    _check_descriptor(guide)
    if CONSTRAINT not in guide['additional_constraints']:
        guide['additional_constraints'].append(CONSTRAINT)
    vocabulary = guide['vocabulary']
    for kind, fields in FIELDS.items():
        vocabulary['marks'][kind] = list(fields)
        vocabulary['mark_fields'][kind] = {'targets': ['event'], 'required': REQUIRED[kind],
            'fields': deepcopy(fields), 'required_extension': EXTENSION,
            'meaning': 'Typed mixed-notation topology at the exact authored host onset. ' + BEHAVIOR}
    guide['extensions'].setdefault('registered', {})[EXTENSION] = {
        'status': 'bounded mixed-family authored interchange; rendering and performance unavailable',
        'mark_kinds': [EVENT, CHILD], 'required_behavior': BEHAVIOR}


def install_schema(schema):
    _check_schema(schema)
    clauses = schema['$defs']['Mark'].setdefault('allOf', [])
    # Idempotent targeted update preserves all other capability contracts.
    clauses[:] = [c for c in clauses if c.get('if', {}).get('properties', {}).get('kind', {}).get('const') not in FIELDS]
    for kind, fields in FIELDS.items():
        properties = {}
        for name, field in fields.items():
            properties[name] = ({'type': 'integer', 'minimum': 0} if field['type'] == 'nonnegative_integer'
                                else {'enum': field['values']} if field['type'] == 'enum'
                                else {'$ref': '#/$defs/Identifier'})
        attributes = {'type': 'object', 'properties': properties,
                      'required': REQUIRED[kind], 'additionalProperties': False}
        if kind == CHILD:
            variants = []
            for child, extra in [('articulations', ['group_index']), ('tied', ['phase']),
                                 ('slur', ['phase', 'relation']), ('fermata', [])]:
                forbidden = set(['group_index', 'phase', 'relation']) - set(extra)
                variants.append({'properties': {'kind': {'const': child}},
                                 'required': extra,
                                 'not': {'anyOf': [{'required': [name]} for name in sorted(forbidden)]}})
            attributes['oneOf'] = variants
        clauses.append({'if': {'properties': {'kind': {'const': kind}}},
                        'then': {'properties': {'target': {'$ref': '#/$defs/Reference'}, 'attributes': attributes},
                                 'description': BEHAVIOR}})
