import {describe,it,expect} from 'vitest';
import {project} from '../src/render/project';
import type {View} from '../src/types';
const fixture=():View=>({valid:true,diagnostics:[],source:'Opaque unchanged source',document:{frames:[]},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false,parts:[{id:'p',instrument:'piano',staves:1}],voices:[{id:'v',part:'p',staff:1}],states:[{time:'0',scope:'p.s1',values:{meter:'4/4',clef:'G2',key:0}}],measures:[{id:'m',part:'p',time:'0',length:'4',label:'1'}],events:[{id:'n',voice:'v',pitch:'C4',time:'0',duration:'4',staff:1,chord:null,written_duration:'1',dots:0,source_line:1}]});
describe('authored note visibility preview boundary',()=>{
 for(const pitch of ['C4','r'])it(`warns without changing pitches, timing or source for hidden ${pitch}`,()=>{
  const v=fixture();v.events[0].pitch=pitch;const baseline=project(v);
  v.events[0].attributes={visible:false};const before=JSON.stringify(v),hidden=project(v);
  expect(hidden.mei).toBe(baseline.mei);expect(hidden.map).toEqual(baseline.map);
  expect(hidden.warnings).toContain('Authored hidden notes and rests are retained but are shown in this preview. Print-object visibility is not reproduced; sounding pitches and timing are unchanged.');
  expect(JSON.stringify(v)).toBe(before);
 });
 it('does not warn for explicit true or absence and keeps mixed chord owners distinct',()=>{
  const v=fixture();v.events[0].attributes={visible:true};expect(project(v).warnings.some(w=>w.includes('Print-object'))).toBe(false);
  v.events[0].chord='c';v.events.push({...v.events[0],id:'c',pitch:'E4',attributes:{visible:false}});
  const result=project(v);expect(Object.values(result.map)).toEqual(['n','c']);expect(result.mei).toContain('<chord');expect(result.warnings.filter(w=>w.includes('Print-object'))).toHaveLength(1);
 });
});

describe('authored notehead preview boundary',()=>{
 it('warns for none or a different shape without hiding the whole note',()=>{
  const v=fixture(),baseline=project(v);
  for(const notehead of ['none','diamond']){
   v.events[0].attributes={notehead};const result=project(v);
   expect(result.mei).toBe(baseline.mei);expect(result.map).toEqual(baseline.map);
   expect(result.warnings).toContain('Authored notehead shapes are retained but this preview displays standard noteheads. Sounding pitches and timing are unchanged.');
  }
  v.events[0].attributes={notehead:'normal'};expect(project(v).warnings).toEqual(baseline.warnings);
 });
});
