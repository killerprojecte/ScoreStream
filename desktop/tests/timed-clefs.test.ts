import {describe,it,expect} from 'vitest';
import {project} from '../src/render/project';
import type {View} from '../src/types';
const fixture=():View=>({valid:true,diagnostics:[],source:'Never parsed by the renderer.',document:{frames:[]},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false,parts:[{id:'p',instrument:'flute',staves:1}],voices:[{id:'v',part:'p',staff:1}],states:[{time:'0',scope:'p.s1',values:{meter:'4/4',clef:'G2',key:0}}],measures:[{id:'m',part:'p',time:'0',length:'4',label:'1'}],events:[{id:'a',voice:'v',pitch:'C4',time:'0',duration:'1/2',staff:1,chord:null,written_duration:'8',dots:0,source_line:1}]});
describe('interior clef preview boundary',()=>{
 it('discloses withheld engraving without mutating source or timing',()=>{const v=fixture();v.states.push({time:'1/3',scope:'p.s1',values:{clef:'F4'}});const before=JSON.stringify(v);expect(()=>project(v)).toThrow('Mid-measure clef changes are retained');expect(JSON.stringify(v)).toBe(before);});
 it('retains the distinct unsupported measure-boundary diagnostic',()=>{const v=fixture();v.measures.push({id:'m2',part:'p',time:'4',length:'4',label:'2'});v.states.push({time:'4',scope:'p.s1',values:{clef:'F4'}});expect(()=>project(v)).toThrow('Mid-score state changes');});
});
