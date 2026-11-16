#!/usr/bin/env python3
"""Validate actual stdio reference payloads and report serialized UTF-8 byte sizes.

Requires the built scorestream-mcp binary and Python jsonschema. No network calls.
"""
import json
from pathlib import Path
import subprocess
import sys

import jsonschema

root = Path(__file__).resolve().parents[1]
binary = root / 'target/debug/scorestream-mcp'
if sys.platform == 'win32':
    binary = binary.with_suffix('.exe')
language = json.loads((root / 'design/spec/scorestream-language.json').read_text())
schema = json.loads((root / 'design/schemas/mcp/score_get_format.output.schema.json').read_text())
input_schema = json.loads((root / 'design/schemas/mcp/score_get_format.input.schema.json').read_text())
topics = input_schema['properties']['topics']['items']['enum']
cases = {
    'empty': {}, 'quick': {'detail': 'quick'}, 'empty_topics': {'topics': []},
    'full': {'detail': 'full'}, 'full_with_topics': {'detail': 'full', 'topics': ['pitch']},
    **{topic: {'topics': [topic]} for topic in topics},
    'combined': {'topics': ['time', 'relations', 'editing']},
    'combined_reordered': {'topics': ['editing', 'relations', 'time']},
    'all_topics': {'topics': topics},
}
requests = [{'jsonrpc': '2.0', 'id': name, 'method': 'tools/call',
             'params': {'name': 'score_get_format', 'arguments': args}} for name, args in cases.items()]
process = subprocess.run([str(binary)], input='\n'.join(map(json.dumps, requests)) + '\n',
                         text=True, capture_output=True, check=True)
responses = {response['id']: response for response in map(json.loads, process.stdout.splitlines())}
assert set(responses) == set(cases)
for name, response in responses.items():
    result = response['result']
    assert result['isError'] is False, name
    selected = result['structuredContent']
    jsonschema.Draft202012Validator(schema).validate(selected)
    assert json.loads(result['content'][0]['text']) == selected, name
    for example in selected['data']['examples']:
        assert example in language['examples'], (name, example['name'])

for same in ['quick', 'empty_topics']:
    assert responses[same]['result'] == responses['empty']['result']
assert responses['full']['result'] == responses['full_with_topics']['result']
assert responses['combined']['result'] == responses['combined_reordered']['result']
expected = dict(language, status=responses['full']['result']['structuredContent']['data']['status'])
assert responses['full']['result']['structuredContent']['data'] == expected
assert 'relations' not in responses['marks']['result']['structuredContent']['data']['vocabulary']
assert 'vocabulary' not in responses['editing']['result']['structuredContent']['data']
assert 'vocabulary' not in responses['empty']['result']['structuredContent']['data']
for topic in ['validation', 'all_topics']:
    assert responses[topic]['result']['structuredContent']['data']['additional_constraints'] == language['additional_constraints']
articulation = next(value for value in language['additional_constraints']
                    if value.startswith('scorestream.articulation@1 preserves'))
for topic in ['marks', 'extensions', 'validation']:
    assert articulation in responses[topic]['result']['structuredContent']['data']['additional_constraints'], topic

# Reconstruct the previous implementation's exact result fields as a byte-size
# baseline. This is not a token count; JSON separators are normalized identically.
reference = (root / 'design/spec/agent-reference.md').read_text()
old_data = json.loads((root / 'design/examples/mcp/valid/score_get_format.full.structured.json').read_text())
old_data['data']['status'] = expected['status']
old_quick = {'content': [{'type': 'text', 'text': reference}], 'structuredContent': old_data, 'isError': False}
old_full = dict(old_quick, content=[{'type': 'text', 'text': reference + '\n\nFull machine-readable format contract:\n' + json.dumps(old_data['data'], ensure_ascii=False, indent=2, sort_keys=True)}])
def size(value):
    return len(json.dumps(value, ensure_ascii=False, separators=(',', ':')).encode('utf-8'))
sizes = {name: {'result_utf8_bytes': size(response['result']),
                'text_utf8_bytes': len(response['result']['content'][0]['text'].encode('utf-8')),
                'structured_utf8_bytes': size(response['result']['structuredContent'])}
         for name, response in responses.items()}
assert sizes['empty']['result_utf8_bytes'] < 18000
assert sizes['empty']['result_utf8_bytes'] < size(old_quick)
assert all(sizes[topic]['result_utf8_bytes'] < sizes['full']['result_utf8_bytes'] for topic in topics)
print(json.dumps({'status': 'PASS', 'schema_validated_cases': len(cases),
                  'previous_result_utf8_bytes': {'quick': size(old_quick), 'full_or_topics': size(old_full)},
                  'current': sizes}, indent=2))
