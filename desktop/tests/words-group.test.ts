import {describe,it,expect} from 'vitest';
import {project} from '../src/render/project';
import type {View} from '../src/types';
const fixture=():View=>({valid:true,diagnostics:[],source:'Opaque unchanged source',document:{frames:[]},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false,parts:[{id:'p',instrument:'piano',staves:1}],voices:[{id:'v',part:'p',staff:1}],states:[{time:'0',scope:'p.s1',values:{meter:'4/4',clef:'G2',key:0}}],measures:[{id:'m',part:'p',time:'0',length:'4',label:'1'}],events:[{id:'n',voice:'v',pitch:'C4',time:'0',duration:'4',staff:1,chord:null,written_duration:'1',dots:0,source_line:1}]});
describe('authored words sibling group',()=>{
 it('warns rather than substitutes unavailable engraving or interpreting private-use glyphs',()=>{
  const v=fixture(),baseline=project(v);
  v.document.required_extensions=['scorestream.words@1'];
  v.document.frames=[{time:'0',records:[
   {record:'mark',kind:'scorestream.words',target:'#m',attributes:{part:'p',index:0,text:'sempre ',placement:'below',default_y:'-40',relative_x:'-98/25',relative_y:'-854/25'}},
   {record:'mark',kind:'scorestream.words',target:'#m',attributes:{part:'p',index:0,row:1,text:'\uE522\uE522',placement:'below',font_family:'Leland Text'}}
  ]}];
  const before=JSON.stringify(v),result=project(v);
  expect(result.mei).toBe(baseline.mei);
  expect(result.warnings.filter(w=>w.includes('scorestream.words')&&w.includes('not drawn'))).toHaveLength(2);
  expect(JSON.stringify(v)).toBe(before);
 });
});
