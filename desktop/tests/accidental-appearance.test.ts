// @vitest-environment jsdom
import {beforeAll,describe,it,expect} from 'vitest';
import createVerovioModule from 'verovio/wasm';
import {VerovioToolkit} from 'verovio/esm';
import {project} from '../src/render/project';
import type {View,MarkRecord} from '../src/types';

const fixture=():View=>({valid:true,diagnostics:[],source:'Typed snapshots only; never parse source here.',document:{frames:[]},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false,parts:[{id:'p',instrument:'Flute',staves:1}],voices:[{id:'v',part:'p',staff:1}],states:[{time:'0',scope:'p.s1',values:{meter:'4/4',clef:'G2',key:0}}],measures:[{id:'m',part:'p',time:'0',length:'4',label:'1'}],events:[{id:'original-note',voice:'v',pitch:'C4',time:'0',duration:'1',staff:1,chord:null,written_duration:'4',dots:0,source_line:5}]});
const mark=(attributes:MarkRecord['attributes'],target='#original-note'):MarkRecord=>({record:'mark',kind:'accidental',target,attributes});
const attach=(v:View,attributes:MarkRecord['attributes'],event=v.events[0])=>v.document.frames!.push({time:event.time,records:[mark(attributes,`#${event.id}`)]});
const parse=(text:string)=>new DOMParser().parseFromString(text,'application/xml');
const flagValues=[undefined,false,true] as const;
const flagCases=flagValues.flatMap(cautionary=>flagValues.map(parentheses=>({
 label:`cautionary=${String(cautionary)}, parentheses=${String(parentheses)}`,
 cautionary,parentheses,
 flags:{...(cautionary===undefined?{}:{cautionary}),...(parentheses===undefined?{}:{parentheses})},
})));
const glyphCases=[{value:'natural',pitch:'C4',mei:'n',smufl:'#E261'},{value:'sharp',pitch:'C#4',mei:'s',smufl:'#E262'},{value:'flat',pitch:'Cb4',mei:'f',smufl:'#E260'}];

describe('bounded authored accidental projection',()=>{
 for(const {value,pitch,mei} of glyphCases)it.each(flagCases)(`${value}: $label`,({flags,cautionary,parentheses})=>{
  const v=fixture();v.events[0].pitch=pitch;attach(v,{value,...flags});const before=JSON.stringify(v);
  const result=project(v),doc=parse(result.mei),note=doc.querySelector('note')!,accid=note.querySelector('accid')!;
  expect(doc.querySelector('parsererror')).toBeNull();expect(doc.querySelectorAll('accid')).toHaveLength(1);
  expect(note.hasAttribute('accid')).toBe(false);expect(note.getAttribute('pname')).toBe('c');expect(note.getAttribute('oct')).toBe('4');
  expect(accid.getAttribute('accid')).toBe(mei);expect(accid.getAttribute('func')).toBe(cautionary===true?'caution':null);
  expect(accid.getAttribute('enclose')).toBe(parentheses===undefined?null:parentheses?'paren':'none');
  expect(result.map).toEqual({'ss-e0':'original-note','ss-mark0':'original-note'});expect(result.warnings).toEqual([]);
  expect(JSON.stringify(v)).toBe(before);
 });
 it('keeps the existing derived pitch projection only when no explicit accidental exists',()=>{
  const v=fixture();expect(parse(project(v).mei).querySelector('note')?.hasAttribute('accid')).toBe(false);
  for(const [pitch,accid] of [['C#4','s'],['Cb4','f'],['C##4','ss'],['Cbb4','ff']]){
   v.events[0].pitch=pitch;const result=project(v),doc=parse(result.mei);
   expect(doc.querySelector('note')?.getAttribute('accid')).toBe(accid);expect(doc.querySelector('accid')).toBeNull();
   expect(result.map).toEqual({'ss-e0':'original-note'});
  }
 });
 it('uses generated IDs instead of embedding source mark identity as markup',()=>{
  const v=fixture();attach(v,{value:'natural',id:'authored-accidental'});const result=project(v);
  expect(result.mei).toContain('<accid xml:id="ss-mark0" accid="n"/>');expect(result.mei).not.toContain('authored-accidental');
  const unsafe=fixture();attach(unsafe,{value:'natural',id:'"><script>alert(1)</script>'});
  expect(project(unsafe).mei).toBe(result.mei);expect(JSON.stringify(unsafe)).toContain('script');
 });
 it('consumes accidentals once without changing existing legacy mark warnings or mark IDs',()=>{
  const v=fixture();v.document.frames=[{time:'0',records:[
   {record:'mark',kind:'words',target:'score',attributes:{text:'retained legacy warning'}},
   mark({value:'natural'}),
   {record:'mark',kind:'dynamic',target:'#original-note',attributes:{value:'p'}},
  ]}];
  const result=project(v);expect(result.warnings).toHaveLength(1);expect(result.warnings[0]).toContain('words mark');
  expect(result.mei.match(/<accid /g)).toHaveLength(1);expect(result.mei).toContain('<dynam xml:id="ss-mark2"');
  expect(result.map).toEqual({'ss-e0':'original-note','ss-mark1':'original-note','ss-mark2':'original-note'});
 });
 it('uses exact rational frame matching',()=>{
  const v=fixture();Object.assign(v.events[0],{time:'1/2',duration:'1/2',written_duration:'8'});attach(v,{value:'natural'});v.document.frames![0].time='2/4';
  expect(project(v).warnings).toEqual([]);
  v.document.frames![0].time='0';expect(()=>project(v)).toThrow('event onset frame');
 });
 it('consumes cross-staff accidental marks only in their note, retaining selection identity',()=>{
  const v=fixture();v.parts[0].staves=2;v.events[0].staff=2;
  v.states.push({time:'0',scope:'p.s2',values:{meter:'4/4',clef:'F4',key:0}});attach(v,{value:'natural'});
  const result=project(v),doc=parse(result.mei);
  expect(doc.querySelectorAll('accid')).toHaveLength(1);expect(doc.querySelector('accid')?.closest('staff')?.getAttribute('n')).toBe('2');
  expect(result.map).toEqual({'ss-e0':'original-note','ss-mark0':'original-note'});expect(result.warnings).toEqual([]);
 });
 it.each(['score','p','p.s1','v','#missing','original-note','#'])('withholds unsupported accidental target %s',target=>{
  const v=fixture();v.document.frames=[{time:'0',records:[mark({value:'natural'},target)]}];
  expect(()=>project(v)).toThrow('requires one mapped event target');
 });
 it.each(['r','_','C(+1/2)4'])('withholds accidental targets that are not supported pitched notes: %s',pitch=>{
  const v=fixture();v.events[0].pitch=pitch;attach(v,{value:'natural'});expect(()=>project(v)).toThrow('supported pitched note');
 });
 it('does not silently lose an accidental attached to a full-measure rest',()=>{
  const v=fixture();Object.assign(v.events[0],{pitch:'r',duration:'4',written_duration:null,attributes:{measure_rest:true}});
  attach(v,{value:'natural'});expect(()=>project(v)).toThrow('supported pitched note');
 });
 it.each(['double-sharp','flat-flat','quarter-sharp','quarter-flat','natural-sharp','other','__proto__','<script/>',''])('withholds unimplemented or unknown glyph %s',value=>{
  const v=fixture();attach(v,{value});expect(()=>project(v)).toThrow('unsupported accidental value');
 });
 it('withholds missing or non-string accidental values',()=>{
  const cases:MarkRecord['attributes'][]=[{},{value:false},{value:1}];
  for(const attributes of cases){const v=fixture();attach(v,attributes);expect(()=>project(v)).toThrow('unsupported accidental value');}
 });
 it.each(['visible','editorial','color','placement','glyph','glyph.num','x-unknown'])('withholds unimplemented accidental field %s instead of ignoring it',field=>{
  for(const value of [false,true,'authored']){const v=fixture();attach(v,{value:'natural',[field]:value});expect(()=>project(v)).toThrow('unsupported accidental attributes');}
 });
 it.each(['cautionary','parentheses'])('withholds malformed %s values',flag=>{
  for(const value of ['yes','no','true','false','',0,1]){const v=fixture();attach(v,{value:'natural',[flag]:value});expect(()=>project(v)).toThrow(`${flag} must be a boolean`);}
 });
 it.each(['',false,1])('withholds malformed accidental identity %s',id=>{
  const v=fixture();attach(v,{value:'natural',id});expect(()=>project(v)).toThrow('unsupported accidental identity');
 });
 it.each([['C#4','natural'],['Cb4','sharp'],['C4','flat']])('withholds conflicting written pitch %s and printed %s', (pitch,value)=>{
  const v=fixture();v.events[0].pitch=pitch;attach(v,{value});expect(()=>project(v)).toThrow('conflicts with written pitch');
 });
 it('withholds duplicate accidentals even when their values agree',()=>{
  const v=fixture();attach(v,{value:'natural'});attach(v,{value:'natural'});
  expect(()=>project(v)).toThrow('duplicate accidental marks');
 });
 it('withholds ambiguous duplicated event IDs',()=>{
  const v=fixture();v.events.push({...v.events[0],time:'1'});attach(v,{value:'natural'});
  expect(()=>project(v)).toThrow('requires one mapped event target');
 });
 it('withholds accidental events outside measures or staff mapping instead of returning a warning-only preview',()=>{
  for(const changes of [{time:'4'},{staff:2},{voice:'unknown'}]){
   const v=fixture();Object.assign(v.events[0],changes);attach(v,{value:'natural'});
   expect(()=>project(v)).toThrow('outside the engraved measures or staff mapping');
  }
 });
 it('retains the existing grace-layout guard despite valid explicit accidental flags',()=>{
  const v=fixture();v.events[0].duration='?';v.events[0].attributes={kind:'grace',anchor:'#principal',order:-1,slash:true};
  attach(v,{value:'natural',cautionary:true,parentheses:false});const before=JSON.stringify(v);
  expect(()=>project(v)).toThrow('Grace-note layout is not yet supported');expect(JSON.stringify(v)).toBe(before);
 });
});

describe('pinned Verovio explicit accidental glyph fidelity',()=>{
 let toolkit:VerovioToolkit;
 beforeAll(async()=>{
  toolkit=new VerovioToolkit(await createVerovioModule());
  expect((toolkit as VerovioToolkit & {getVersion():string}).getVersion()).toMatch(/^5\.6\.0/);
  toolkit.setOptions({pageWidth:2100,pageHeight:2600,scale:40,adjustPageHeight:true,header:'none',footer:'none',font:'Bravura'});
 },10000);
 const glyphs=(element:Element|null)=>[...(element?.querySelectorAll('use')??[])].map(use=>use.getAttribute('xlink:href')?.split('-')[0]);
 const render=(v:View)=>{
  const result=project(v);expect(toolkit.loadData(result.mei)).toBeTruthy();const svg=parse(toolkit.renderToSVG(1));
  expect(svg.querySelector('parsererror')).toBeNull();expect(result.warnings).toEqual([]);
  for(const id of Object.keys(result.map))expect(svg.querySelector(`[id="${id}"]`)).not.toBeNull();
  return {svg,result};
 };
 const contexts=[
  ...glyphCases.map(c=>({...c,key:0})),
  {value:'natural',pitch:'F4',smufl:'#E261',key:1},
  {value:'natural',pitch:'B4',smufl:'#E261',key:-1},
  {value:'sharp',pitch:'F#4',smufl:'#E262',key:1},
  {value:'flat',pitch:'Bb4',smufl:'#E260',key:-1},
 ];
 it.each(contexts)('renders repeated explicit $value on $pitch in key $key, for all nine independent flag combinations',({value,pitch,smufl,key})=>{
  for(const {flags,parentheses} of flagCases){
   const v=fixture();v.states[0].values.key=key;
   v.events=Array.from({length:3},(_,i)=>({...v.events[0],id:`repeated-${i}`,pitch,time:String(i)}));
   v.events.forEach(event=>attach(v,{value,...flags},event));const before=JSON.stringify(v);
   const {svg,result}=render(v);expect(svg.querySelectorAll('.note .accid')).toHaveLength(3);
   for(let i=0;i<3;i++){
    const note=svg.querySelector(`[id="ss-e${i}"]`)!,accid=note.querySelector(`[id="ss-mark${i}"]`);
    expect(note.querySelectorAll('.accid')).toHaveLength(1);
    expect(glyphs(accid)).toEqual(parentheses===true?['#E26A',smufl,'#E26B']:[smufl]);
    expect(result.map[`ss-e${i}`]).toBe(`repeated-${i}`);expect(result.map[`ss-mark${i}`]).toBe(`repeated-${i}`);
   }
   expect(JSON.stringify(v)).toBe(before);
  }
 },10000);
 it.each(['up','down'])('renders independent chord-member glyphs and parentheses with %s stems',stem=>{
  const v=fixture();v.states[0].values.key=1;
  v.events=[{...v.events[0],pitch:'F4',chord:'ch',attributes:{stem}},
   {...v.events[0],id:'sharp-member',pitch:'A#4',chord:'ch',attributes:{stem}},
   {...v.events[0],id:'flat-member',pitch:'Bb4',chord:'ch',attributes:{stem}}];
  attach(v,{value:'natural',cautionary:true,parentheses:false},v.events[0]);
  attach(v,{value:'sharp',cautionary:false,parentheses:true},v.events[1]);
  attach(v,{value:'flat'},v.events[2]);
  const {svg,result}=render(v),mei=parse(result.mei);
  expect(mei.querySelector('chord')?.getAttribute('stem.dir')).toBe(stem);
  expect(mei.querySelectorAll('note[accid]')).toHaveLength(0);expect(svg.querySelectorAll('.chord .accid')).toHaveLength(3);
  expect(glyphs(svg.querySelector('[id="ss-mark0"]'))).toEqual(['#E261']);
  expect(glyphs(svg.querySelector('[id="ss-mark1"]'))).toEqual(['#E26A','#E262','#E26B']);
  expect(glyphs(svg.querySelector('[id="ss-mark2"]'))).toEqual(['#E260']);
  const coords=svg.querySelector('.chord .stem path')?.getAttribute('d')?.match(/^M([\d.-]+) ([\d.-]+) L([\d.-]+) ([\d.-]+)$/);
  expect(coords).not.toBeNull();const delta=Number(coords![4])-Number(coords![2]);
  if(stem==='up')expect(delta).toBeLessThan(0);else expect(delta).toBeGreaterThan(0);
 },10000);
 it('engraves the written C-sharp for a B-flat transposing part, without substituting sounding B3',()=>{
  const v=fixture();v.parts[0].instrument='clarinet';v.events[0].pitch='C#4';
  v.states.push({time:'0',scope:'p',values:{transpose_chromatic:-2,transpose_diatonic:-1,transpose_octaves:0}});
  attach(v,{value:'sharp',cautionary:true,parentheses:false});const before=JSON.stringify(v);
  const {svg,result}=render(v),note=parse(result.mei).querySelector('note')!;
  expect(note.getAttribute('pname')).toBe('c');expect(note.getAttribute('oct')).toBe('4');
  expect(glyphs(svg.querySelector('[id="ss-mark0"]'))).toEqual(['#E262']);
  expect(JSON.stringify(v)).toBe(before);
 });
 it('scales explicit cue accidentals and their parentheses with the authored cue notes',()=>{
  const v=fixture();v.events[0].pitch='C#4';attach(v,{value:'sharp',parentheses:true});
  v.events.push({...v.events[0],id:'cue-note',time:'1',attributes:{note_size:'cue'}});
  attach(v,{value:'sharp',parentheses:true},v.events[1]);const {svg}=render(v);
  const scale=(id:string)=>Number(svg.querySelector(`[id="${id}"] use`)?.getAttribute('transform')?.match(/scale\(([^,)]+)/)?.[1]);
  expect(glyphs(svg.querySelector('[id="ss-mark0"]'))).toEqual(['#E26A','#E262','#E26B']);
  expect(glyphs(svg.querySelector('[id="ss-mark1"]'))).toEqual(['#E26A','#E262','#E26B']);
  expect(scale('ss-mark1')).toBeGreaterThan(0);expect(scale('ss-mark1')).toBeLessThan(scale('ss-mark0'));
 });
 it('retains explicit natural, sharp and flat through a following full-measure-rest measure',()=>{
  const v=fixture();v.events=glyphCases.map(({pitch},i)=>({...v.events[0],id:`note-${i}`,pitch,time:String(i)}));
  v.events.forEach((event,i)=>attach(v,{value:glyphCases[i].value,cautionary:true,parentheses:false},event));
  v.measures.push({...v.measures[0],id:'rest-measure',time:'4',label:'2'});
  v.events.push({...v.events[0],id:'full-rest',time:'4',pitch:'r',duration:'4',written_duration:null,attributes:{measure_rest:true}});
  const {svg,result}=render(v);expect(svg.querySelectorAll('.note .accid')).toHaveLength(3);
  expect(svg.querySelector('[id="ss-e3"]')?.getAttribute('class')).toBe('mRest');expect(result.map['ss-e3']).toBe('full-rest');
  expect(glyphs(svg.querySelector('[id="ss-e3"]'))).toEqual(['#E4E3']);
 });
});
