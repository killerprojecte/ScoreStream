import {describe,it,expect} from 'vitest';
import {project} from '../src/render/project';
import type {View} from '../src/types';
import family from './fixtures/authored-articulation-family.json';
import mixed from './fixtures/mixed-authored-notations.json';
const fixture=():View=>structuredClone(family) as unknown as View;
describe('ordered mixed-family preview boundary',()=>{
 it('withholds actual Rust mixed import snapshot without deleting source objects',()=>{
  const v=mixed as unknown as View;const before=JSON.stringify(v);
  expect(v.document.required_extensions).toContain('scorestream.notations@1');
  expect(()=>project(v)).toThrow('Ordered mixed-family');expect(JSON.stringify(v)).toBe(before);
 });
 for(const version of ['1','2'])it(`refuses declared v${version} before any partial projection`,()=>{
  const v=fixture();v.document.required_extensions=[`scorestream.notations@${version}`];
  const before=JSON.stringify(v);expect(()=>project(v)).toThrow('Ordered mixed-family');expect(JSON.stringify(v)).toBe(before);
 });
 for(const kind of ['scorestream.notations','scorestream.notations.event','scorestream.notations.child','scorestream.notations.future'])it(`refuses ${kind} even without capability`,()=>{
  const v=fixture();v.document.required_extensions=[];
  v.document.frames=[{time:'0',records:[{record:'mark',kind,target:'#a',attributes:{}}]}];
  const before=JSON.stringify(v);expect(()=>project(v)).toThrow('Ordered mixed-family');expect(JSON.stringify(v)).toBe(before);
 });
});
