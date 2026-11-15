// @vitest-environment jsdom
import {beforeAll,describe,it,expect} from 'vitest';
import {writeFileSync} from 'node:fs';
import studyJson from './fixtures/articulation-study.json?raw';
import duetJson from './fixtures/articulation-duet.json?raw';
import createVerovioModule from 'verovio/wasm';
import {VerovioToolkit} from 'verovio/esm';
import {project} from '../src/render/project';
import type {View,MarkRecord} from '../src/types';
const fixture=():View=>({valid:true,diagnostics:[],source:'Not parsed by the renderer.',document:{frames:[]},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false,parts:[{id:'p',instrument:'piano',staves:1}],voices:[{id:'v',part:'p',staff:1}],states:[{time:'0',scope:'p.s1',values:{meter:'4/4',clef:'G2',key:0}}],measures:[{id:'m',part:'p',time:'0',length:'4',label:'1'}],events:[{id:'a',voice:'v',pitch:'C4',time:'0',duration:'1',staff:1,chord:null,written_duration:'4',dots:0,source_line:1}]});
const attach=(v:View,attributes:MarkRecord['attributes'],event=v.events[0])=>v.document.frames!.push({time:event.time,records:[{record:'mark',kind:'articulation',target:`#${event.id}`,attributes}]});
const parse=(s:string)=>new DOMParser().parseFromString(s,'application/xml');
const values=[['staccato','stacc'],['staccatissimo','stacciss'],['tenuto','ten'],['accent','acc']] as const;
describe('simple articulation projection',()=>{
 it.each(values)('projects %s with distinct absent/above/below placement and selection identity',(value,meiValue)=>{
  for(const placement of [undefined,'above','below']){
   const v=fixture();attach(v,{value,...(placement?{placement}:{})});const before=JSON.stringify(v),r=project(v),xml=parse(r.mei);
   const mark=xml.querySelector('note > artic')!;expect(mark.getAttribute('artic')).toBe(meiValue);
   expect(mark.getAttribute('place')).toBe(placement??null);expect(r.map['ss-artic0']).toBe('a');
   expect(r.warnings).toEqual([]);expect(JSON.stringify(v)).toBe(before);
  }
 });
 it('does not suppress pitch accidentals when adding articulations',()=>{
  const v=fixture();v.events[0].pitch='C#4';attach(v,{value:'staccato'});
  expect(parse(project(v).mei).querySelector('note')?.getAttribute('accid')).toBe('s');
 });
 it('keeps explicit accidental and articulation children independent',()=>{
  const v=fixture();v.events[0].pitch='C#4';attach(v,{value:'accent'});
  v.document.frames!.push({time:'0',records:[{record:'mark',kind:'accidental',target:'#a',attributes:{value:'sharp'}}]});
  const r=project(v),note=parse(r.mei).querySelector('note')!;expect(note.getAttribute('accid')).toBeNull();
  expect(note.querySelectorAll('accid')).toHaveLength(1);expect(note.querySelectorAll('artic')).toHaveLength(1);
 });
 it('rejects unsupported shapes, attributes, invalid placement and duplicates rather than silently dropping marks',()=>{
  for(const attributes of [{value:'strong-accent'},{value:'breath-mark'},{value:'spiccato'},{value:'staccato',placement:'sideways'},{value:'staccato',default_y:4},{value:'accent',sound:false}] as MarkRecord['attributes'][]){
   const v=fixture();attach(v,attributes);expect(()=>project(v)).toThrow('Preview withheld');
  }
  const v=fixture();attach(v,{value:'staccato'});attach(v,{value:'staccato',placement:'below'});expect(()=>project(v)).toThrow('duplicate');
 });
 it('rejects missing, non-pitched, grace, duplicate and off-frame anchors',()=>{
  for(const pitch of ['r','_']){const v=fixture();v.events[0].pitch=pitch;attach(v,{value:'tenuto'});expect(()=>project(v)).toThrow('measured pitched');}
  const v=fixture();attach(v,{value:'accent'});v.document.frames![0].time='1';expect(()=>project(v)).toThrow('onset');
  v.document.frames![0].time='0';v.events[0].id='other';expect(()=>project(v)).toThrow('one mapped');
  v.events[0].id='a';v.events.push({...v.events[0]});expect(()=>project(v)).toThrow('one mapped');
 });
});
describe('pinned Verovio articulation fidelity',()=>{
 let toolkit:VerovioToolkit;
 beforeAll(async()=>{
  toolkit=new VerovioToolkit(await createVerovioModule());
  expect((toolkit as VerovioToolkit & {getVersion():string}).getVersion()).toMatch(/^5\.6\.0/);
  toolkit.setOptions({pageWidth:2100,pageHeight:2600,scale:40,adjustPageHeight:true,header:'none',footer:'none',font:'Bravura'});
 },10000);
 const render=(v:View)=>{const result=project(v);expect(toolkit.loadData(result.mei)).toBeTruthy();return {result,svg:parse(toolkit.renderToSVG(1))};};
 it('renders all four symbols, both placements, and glyph identities',()=>{
  // Pinned Verovio uses SMuFL wedge-style staccatissimo E4A8/E4A9.
  // No imported glyph/shape override is accepted by this simple-symbol profile.
  const expected={staccato:['E4A2','E4A3'],staccatissimo:['E4A8','E4A9'],tenuto:['E4A4','E4A5'],accent:['E4A0','E4A1']};
  for(const [value] of values)for(const [i,placement] of ['above','below'].entries()){
   const v=fixture();attach(v,{value,placement});const {result,svg}=render(v),artic=svg.querySelector('[id="ss-artic0"]');
   expect(artic).not.toBeNull();expect(artic!.getAttribute('class')).toBe('artic');
   const uses=[...artic!.querySelectorAll('use')].map(e=>e.getAttribute('xlink:href'));
   expect(uses.some(h=>h?.startsWith(`#${expected[value][i]}`)),`${value} ${placement}: ${uses.join(",")}`).toBe(true);
   expect(result.map['ss-artic0']).toBe('a');expect(result.warnings).toEqual([]);
  }
 });
 it.each(['articulation-study','articulation-duet'])('renders the Rust-produced MusicXML → SFS → workspace snapshot %s',name=>{
  const v=JSON.parse(name==='articulation-study'?studyJson:duetJson) as View;
  const before=JSON.stringify(v),{result,svg}=render(v);expect(result.warnings.filter(w=>w.includes('articulation'))).toEqual([]);
  const marks=(v.document.frames??[]).flatMap(f=>f.records).filter((r):r is MarkRecord=>r.record==='mark'&&r.kind==='articulation');
  expect(svg.querySelectorAll('.artic')).toHaveLength(marks.length);
  for(const [i,mark] of marks.entries()){
   expect(result.map[`ss-artic${i}`]).toBe(mark.target.slice(1));
   expect(svg.querySelector(`[id="ss-artic${i}"] use`)).not.toBeNull();
  }
  expect(JSON.stringify(v)).toBe(before);
  if(process.env.ARTICULATION_EVIDENCE_DIR)writeFileSync(`${process.env.ARTICULATION_EVIDENCE_DIR}/${name}.svg`,new XMLSerializer().serializeToString(svg));
 });
 it('renders the alternate duet with chord-head, member and independent lower-staff identities',()=>{
  const v=fixture();v.parts[0].staves=2;v.voices.push({id:'lower',part:'p',staff:2});v.states.push({time:'0',scope:'p.s2',values:{meter:'4/4',clef:'F4',key:0}});
  v.events[0].chord='c';v.events.push({...v.events[0],id:'member',pitch:'E4'},{...v.events[0],id:'bass',voice:'lower',staff:2,pitch:'C3',chord:null});
  attach(v,{value:'staccato',placement:'above'});attach(v,{value:'accent',placement:'above'});attach(v,{value:'tenuto',placement:'below'},v.events[1]);
  const {result,svg}=render(v),mei=parse(result.mei);expect(mei.querySelector('chord')?.querySelectorAll('artic')).toHaveLength(3);
  expect([...mei.querySelectorAll('note')].find(e=>e.getAttribute('xml:id')==='ss-e2')!.querySelectorAll('artic')).toHaveLength(0);
  for(const [id,event] of [['ss-artic0','a'],['ss-artic1','a'],['ss-artic2','member']]){
   expect(svg.querySelector(`[id="${id}"] use`)).not.toBeNull();expect(result.map[id]).toBe(event);
  }
  expect(svg.querySelectorAll('.artic')).toHaveLength(3);expect(result.warnings).toEqual([]);
 });
});
