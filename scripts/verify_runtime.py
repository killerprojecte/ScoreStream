#!/usr/bin/env python3
"""Independent process-level acceptance; Python is test tooling only."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[1]
cli = root / 'target/debug/scorestream'
mcp = root / 'target/debug/scorestream-mcp'
if sys.platform == 'win32':
    cli = cli.with_suffix('.exe')
    mcp = mcp.with_suffix('.exe')

def run(*args):
    return subprocess.run([str(cli), *map(str,args)], capture_output=True, text=True, cwd=root, check=False)

results = []
for name in ['document-metadata', 'ensemble', 'tuplets-and-ties', 'grace-percussion-cross-staff']:
    source=root/'design/examples/sfs'/f'{name}.sfs'
    actual=run('inspect', source)
    assert actual.returncode == 0, (name,actual.stderr)
    parsed=json.loads(actual.stdout)
    expected=json.loads((root/'design/examples/ast'/f'{name}.json').read_text())
    assert parsed == expected, f'AST mismatch: {name}'
    canonical=run('canonical',source)
    assert canonical.returncode == 0, canonical.stderr
    with tempfile.TemporaryDirectory(prefix='scorestream-roundtrip-') as tmp:
        target=Path(tmp)/'canonical.sfs';target.write_text(canonical.stdout)
        second=run('inspect',target)
        assert second.returncode==0, second.stderr
        rewritten=json.loads(second.stdout)
        # Canonical writing deliberately orders measure/state/event records.
        # Compare all records as a multiset, plus event order within each voice.
        a=json.loads(json.dumps(parsed)); b=json.loads(json.dumps(rewritten))
        for left,right in zip(a['frames'],b['frames'],strict=True):
            voices={r['voice'] for r in left['records'] if r['record']=='event'}
            for voice in voices:
                assert [r for r in left['records'] if r.get('voice')==voice] == [r for r in right['records'] if r.get('voice')==voice], 'Voice/chord/grace ordering changed'
            left['records'].sort(key=lambda r:json.dumps(r,sort_keys=True))
            right['records'].sort(key=lambda r:json.dumps(r,sort_keys=True))
        assert a==b, f'Canonical musical data changed: {name}'
        assert run('canonical',target).stdout==canonical.stdout, f'Canonical writer is not idempotent: {name}' 
    results.append({'case':name,'ast_matches_fixture':True,'canonical_roundtrip':True})

manifest=json.loads((root/'design/examples/sfs-invalid/manifest.json').read_text())
for fixture in manifest['cases']:
    outcome=run('validate',root/'design'/fixture['path'])
    assert outcome.returncode==1,(fixture['path'],outcome.stdout,outcome.stderr)
    assert json.loads(outcome.stderr), fixture['path']
results.append({'case':'invalid-corpus','rejected':len(manifest['cases']),'scope':'rejection; exact diagnostic-code checks are in codec unit tests'})

requests=[
    {'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'acceptance','version':'1'}}},
    {'jsonrpc':'2.0','method':'notifications/initialized'},
    {'jsonrpc':'2.0','id':2,'method':'tools/list'},
    {'jsonrpc':'2.0','id':3,'method':'tools/call','params':{'name':'score_get_format','arguments':{}}},
    {'jsonrpc':'2.0','id':4,'method':'resources/read','params':{'uri':'scorestream://spec/1.1/grammar'}},
    {'jsonrpc':'2.0','id':5,'method':'tools/call','params':{'name':'score_apply_edit','arguments':{}}},
    {'jsonrpc':'2.0','id':6,'method':'resources/read','params':{'uri':'file:///etc/passwd'}},
]
p=subprocess.run([str(mcp)],input='\n'.join(map(json.dumps,requests))+'\n',capture_output=True,text=True,cwd=root,check=False)
assert p.returncode==0,p.stderr
answers=[json.loads(line) for line in p.stdout.splitlines()]
assert [a['id'] for a in answers]==[1,2,3,4,5,6]
assert answers[0]['result']['protocolVersion']=='2025-11-25'
assert [t['name'] for t in answers[1]['result']['tools']]==['score_get_format']
assert answers[2]['result']['isError'] is False
assert 'quarter' in answers[2]['result']['content'][0]['text'].lower()
assert 'document =' in answers[3]['result']['contents'][0]['text']
assert answers[4]['error']['code']==-32602
assert answers[5]['error']['code']==-32002
results.append({'case':'mcp-stdio','requests':len(requests),'responses':len(answers),'only_implemented_tool_advertised':True,'unlisted_resource_rejected':True})
print(json.dumps({'status':'PASS','results':results},indent=2))
