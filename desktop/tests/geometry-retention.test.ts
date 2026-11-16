import {describe,it,expect} from 'vitest';
import {project} from '../src/render/project';
import type {View} from '../src/types';
const fixture=():View=>({valid:true,diagnostics:[],source:'Never parsed by the renderer.',document:{frames:[]},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false,parts:[{id:'p',instrument:'flute',staves:1}],voices:[{id:'v',part:'p',staff:1}],states:[{time:'0',scope:'p.s1',values:{meter:'4/4',clef:'G2',key:0}}],measures:[{id:'m',part:'p',time:'0',length:'4',label:'1'}],events:[{id:'a',voice:'v',pitch:'C4',time:'0',duration:'1/2',staff:1,chord:null,written_duration:'8',dots:0,source_line:1}]});
describe('authored coordinate preservation',()=>{
 it('reflows preview with an explicit warning, without changing source',()=>{
  const v=fixture(); const baseline=project(v);
  v.document.required_extensions=['scorestream.geometry@1'];
  v.document.frames=[{time:'0',records:[{record:'mark',kind:'scorestream.geometry.event',target:'#a',attributes:{default_x:'0',default_y:'-81/8'}},{record:'mark',kind:'scorestream.geometry.measure',target:'#m',attributes:{part:'p',width:'100'}}]}];
  const before=JSON.stringify(v);const result=project(v);
  expect(result.mei).toBe(baseline.mei);
  expect(result.warnings.filter(w=>w.includes('reflows notation independently'))).toHaveLength(1);
  expect(JSON.stringify(v)).toBe(before);
 });
 it('retains dot coordinates without applying them or changing authored dot count',()=>{
  const v=fixture();v.events[0].duration='3';v.events[0].written_duration='2';v.events[0].dots=1;
  const baseline=project(v);v.document.required_extensions=['scorestream.geometry@1'];
  v.document.frames=[{time:'0',records:[{record:'mark',kind:'scorestream.geometry.dot',target:'#a',attributes:{index:0,default_x:'0',default_y:'-1/8'}}]}];
  const before=JSON.stringify(v),result=project(v);
  expect(result.mei).toBe(baseline.mei);
  expect(result.warnings).toContain('Original note, augmentation-dot and measure coordinates are retained in the score. This preview reflows notation independently and does not reproduce those authored positions.');
  expect(JSON.stringify(v)).toBe(before);
 });
});
