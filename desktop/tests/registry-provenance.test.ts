import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {describe,it,expect} from 'vitest';
const root=new URL('../',import.meta.url);
const manifest=JSON.parse(readFileSync(new URL('licenses/shadcn-registry-provenance.json',root),'utf8'));
describe('official shadcn registry provenance',()=>{
 for(const entry of manifest.files) it(`${entry.name} is the recorded official source with import adaptations only`,()=>{
  const text=readFileSync(new URL(`src/components/ui/${entry.name}.tsx`,root),'utf8');
  expect(createHash('sha256').update(text).digest('hex')).toBe(entry.local_sha256);
  expect(entry.registry_url).toBe(`https://ui.shadcn.com/r/styles/new-york-v4/${entry.name}.json`);
 });
});
