import {describe,it,expect} from 'vitest';
import {project} from '../src/render/project';
import type {View} from '../src/types';
const fixture=():View=>({valid:true,diagnostics:[],source:'Never parsed by the renderer.',document:{frames:[]},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false,parts:[{id:'p',instrument:'flute',staves:1}],voices:[{id:'v',part:'p',staff:1}],states:[{time:'0',scope:'p.s1',values:{meter:'4/4',clef:'G2',key:0}}],measures:[{id:'m',part:'p',time:'0',length:'4',label:'1'}],events:[{id:'a',voice:'v',pitch:'C4',time:'0',duration:'1/2',staff:1,chord:null,written_duration:'8',dots:0,source_line:1}]});
describe('authored beam fidelity boundary',()=>{
 it('withholds rather than replacing authored runs or hooks with automatic beaming',()=>{
  for(const value of ['begin','continue','end','forward-hook','backward-hook']){
   const v=fixture();v.document.frames=[{time:'0',records:[{record:'mark',kind:'scorestream.beam',target:'#a',attributes:{number:2,value}}]}];
   const before=JSON.stringify(v);expect(()=>project(v)).toThrow('Authored beam roles and hooks');expect(JSON.stringify(v)).toBe(before);
  }
 });
 it('withholds even if an incomplete snapshot omitted the beam marks',()=>{
  const v=fixture();v.document.required_extensions=['scorestream.beam@1'];expect(()=>project(v)).toThrow('Preview withheld');
 });
 it('keeps existing unannotated rendering unchanged',()=>{
  const v=fixture();expect(project(v).mei).toContain('pname="c"');
 });
});
