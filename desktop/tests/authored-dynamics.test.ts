import {describe,it,expect} from 'vitest';
import {project} from '../src/render/project';
import type {View} from '../src/types';
const fixture=():View=>({valid:true,diagnostics:[],source:'Opaque unchanged source',document:{frames:[]},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false,parts:[{id:'p',instrument:'piano',staves:1}],voices:[{id:'v',part:'p',staff:1}],states:[{time:'0',scope:'p.s1',values:{meter:'4/4',clef:'G2',key:0}}],measures:[{id:'m',part:'p',time:'0',length:'4',label:'1'}],events:[{id:'n',voice:'v',pitch:'C4',time:'0',duration:'4',staff:1,chord:null,written_duration:'1',dots:0,source_line:1}]});
describe('authored boundary dynamics',()=>{
 it('withholds visual substitutes for the independent source-owned lane',()=>{
  const v=fixture();v.document.required_extensions=['scorestream.authored_dynamics@1'];
  const before=JSON.stringify(v);expect(()=>project(v)).toThrow(/boundary dynamic.*explicit performance/);expect(JSON.stringify(v)).toBe(before);
 });
});
