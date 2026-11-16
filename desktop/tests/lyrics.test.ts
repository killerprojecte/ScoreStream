import {describe,it,expect} from 'vitest';
import {project} from '../src/render/project';
import type {View} from '../src/types';
const fixture=():View=>({valid:true,diagnostics:[],source:'Never parsed by the renderer.',document:{frames:[]},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false,parts:[{id:'p',instrument:'flute',staves:1}],voices:[{id:'v',part:'p',staff:1}],states:[{time:'0',scope:'p.s1',values:{meter:'4/4',clef:'G2',key:0}}],measures:[{id:'m',part:'p',time:'0',length:'4',label:'1'}],events:[{id:'a',voice:'v',pitch:'C4',time:'0',duration:'1/2',staff:1,chord:null,written_duration:'8',dots:0,source_line:1}]});
describe('authored lyrics capability disclosure',()=>{
 it('omits written lyrics explicitly without withholding music or modifying source',()=>{
  const v=fixture();const baseline=project(v);v.document.required_extensions=['scorestream.lyrics@1'];
  v.document.frames=[{time:'0',records:[{record:'mark',kind:'scorestream.lyric',target:'#a',attributes:{index:0,number:'1',syllabic:'single',text:'II–V analysis',color:'#FF0000',default_y:'-30'}}]}];
  const before=JSON.stringify(v);const result=project(v);
  expect(result.mei).toBe(baseline.mei);expect(JSON.stringify(v)).toBe(before);
  expect(result.warnings.filter(w=>w.includes('lyrics')&&w.includes('not drawn'))).toHaveLength(1);
  expect(result.warnings.some(w=>w.includes('unsupported notation'))).toBe(false);
 });
});
