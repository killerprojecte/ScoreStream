import {describe,it,expect} from 'vitest';
import {project} from '../src/render/project';
import type {View} from '../src/types';
import family from './fixtures/authored-articulation-family.json';
const fixture=():View=>({valid:true,diagnostics:[],source:'Never parsed by the renderer.',document:{frames:[]},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false,parts:[{id:'p',instrument:'flute',staves:1}],voices:[{id:'v',part:'p',staff:1}],states:[{time:'0',scope:'p.s1',values:{meter:'4/4',clef:'G2',key:0}}],measures:[{id:'m',part:'p',time:'0',length:'4',label:'1'}],events:[{id:'a',voice:'v',pitch:'C4',time:'0',duration:'1/2',staff:1,chord:null,written_duration:'8',dots:0,source_line:1}]});
describe('authored articulation fidelity boundary',()=>{
 it('withholds the actual Rust-imported family snapshot with all seventeen signs intact',()=>{
  const v=family as unknown as View;
  const marks=(v.document.frames??[]).flatMap(f=>f.records).filter(r=>r.record==='mark'&&r.kind==='scorestream.articulation.item');
  expect(marks).toHaveLength(17);
  const before=JSON.stringify(v);expect(()=>project(v)).toThrow('Authored articulation signs');expect(JSON.stringify(v)).toBe(before);
 });
 for(const version of ['1','2'])it(`withholds a header-only v${version} snapshot without modifying source`,()=>{
  const v=fixture();v.document.required_extensions=[`scorestream.articulation@${version}`];
  const before=JSON.stringify(v);expect(()=>project(v)).toThrow('Authored articulation signs');expect(JSON.stringify(v)).toBe(before);
 });
 for(const kind of ['event','notations','group','item','future'])it(`withholds actual ${kind} marks even without the header`,()=>{
  const v=fixture();v.document.frames=[{time:'0',records:[{record:'mark',kind:`scorestream.articulation.${kind}`,target:'#a',attributes:{event_index:0}}]}];
  const before=JSON.stringify(v);expect(()=>project(v)).toThrow('Preview withheld');expect(JSON.stringify(v)).toBe(before);
 });
 for(const value of ['staccato','tenuto','accent','staccatissimo'])it(`keeps legacy ${value} preview unchanged`,()=>{
  const v=fixture();v.document.frames=[{time:'0',records:[{record:'mark',kind:'articulation',target:'#a',attributes:{value}}]}];
  expect(project(v).mei).toContain('<artic');
 });
});
