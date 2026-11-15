// @vitest-environment jsdom
import {describe,it,expect} from 'vitest';
import createVerovioModule from 'verovio/wasm';
import {VerovioToolkit} from 'verovio/esm';
import {project} from '../src/render/project';
import type {View} from '../src/types';
const fixture=():View=>({valid:true,diagnostics:[],source:'Typed snapshots only; never parse source here.',document:{frames:[]},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false,parts:[{id:'p',instrument:'Flute',staves:1}],voices:[{id:'v',part:'p',staff:1}],states:[{time:'0',scope:'p.s1',values:{meter:'2/4',clef:'G2',key:0}}],measures:[{id:'m',part:'p',time:'0',length:'2',label:'1'}],events:[{id:'original-note',voice:'v',pitch:'C4',time:'0',duration:'1',staff:1,chord:null,written_duration:'4',dots:0,source_line:5}]});
const restFixture=():View=>{const v=fixture();v.events=[{...v.events[0],id:'original-rest',pitch:'r',duration:'2',written_duration:null,attributes:{measure_rest:true}}];return v;};
const chordFixture=():View=>{const v=fixture();v.events=[{...v.events[0],chord:'c',attributes:{stem:'up'}},{...v.events[0],id:'other-note',pitch:'E4',chord:'c',attributes:{stem:'up'}}];return v;};
const parse=(text:string)=>new DOMParser().parseFromString(text,'application/xml');

describe('bounded authored stem projection',()=>{
 it.each(['up','down'])('honors an explicit %s stem on individual notes',stem=>{
  const v=fixture();v.events[0].attributes={stem};const before=JSON.stringify(v);const result=project(v);
  expect(parse(result.mei).querySelector('note')?.getAttribute('stem.dir')).toBe(stem);
  expect(result.map).toEqual({'ss-e0':'original-note'});expect(JSON.stringify(v)).toBe(before);
 });
 it.each(['up','down'])('puts consistent %s stems on the chord without duplicating note stems',stem=>{
  const v=chordFixture();v.events.forEach(e=>e.attributes={stem});const result=project(v);const mei=parse(result.mei);
  expect(mei.querySelector('chord')?.getAttribute('stem.dir')).toBe(stem);
  expect(mei.querySelectorAll('note[stem\\.dir]')).toHaveLength(0);
  expect(result.map).toEqual({'ss-e0':'original-note','ss-e1':'other-note'});
 });
 it('keeps unspecified stems automatic',()=>{const v=chordFixture();v.events.forEach(e=>delete e.attributes);expect(project(v).mei).not.toContain('stem.dir');});
 it.each(['none','double','sideways'])('withholds unsupported stem kind %s instead of ignoring it',stem=>{
  const v=fixture();v.events[0].attributes={stem};expect(()=>project(v)).toThrow('stem kind');
 });
 it.each(['r','_'])('withholds authored stems on %s rather than hiding stemlets',pitch=>{
  for(const stem of ['up','down','none','double']){const v=fixture();v.events[0].pitch=pitch;v.events[0].attributes={stem};expect(()=>project(v)).toThrow('Rest stemlets');}
 });
 it('withholds conflicting and partial chord stem indications',()=>{
  const v=chordFixture();v.events[1].attributes={stem:'down'};expect(()=>project(v)).toThrow('Mixed stem indications');
  delete v.events[1].attributes;expect(()=>project(v)).toThrow('Mixed stem indications');
 });
 it('withholds authored stems on a stemless whole-note duration',()=>{
  const v=fixture();v.measures[0].length='4';v.states[0].values.meter='4/4';v.events[0]={...v.events[0],duration:'4',written_duration:'1',attributes:{stem:'up'}};
  expect(()=>project(v)).toThrow('stemless durations');
 });
 it('does not silently repair an explicit written dot mismatch',()=>{
  const v=fixture();v.events[0].duration='3/2';expect(()=>project(v)).toThrow('Written and actual rhythm differ');
  v.events[0].dots=1;expect(project(v).mei).toContain('dur="4" dots="1"');
 });
});

describe('full-measure-rest projection without rewritten duration',()=>{
 it('uses mRest and the existing event mapping in a non-whole-note measure',()=>{
  const v=restFixture();const before=JSON.stringify(v);const result=project(v);const rest=parse(result.mei).querySelector('mRest');
  expect(rest?.getAttribute('xml:id')).toBe('ss-e0');expect(rest?.hasAttribute('dur')).toBe(false);expect(rest?.hasAttribute('dots')).toBe(false);
  expect(result.mei).toContain('meter.count="2" meter.unit="4"');expect(result.mei).not.toContain('<rest ');
  expect(result.map).toEqual({'ss-e0':'original-rest'});expect(JSON.stringify(v)).toBe(before);
 });
 it('validates exact rational extents, including nonzero measure starts',()=>{
  const v=restFixture();v.measures[0].time='3/2';v.measures[0].length='6/3';v.events[0].time='9/6';v.events[0].duration='4/2';
  expect(project(v).mei).toContain('<mRest xml:id="ss-e0"/>');
 });
 it('keeps false and absent indications distinct in data and renders both as ordinary rests',()=>{
  const absent=restFixture();delete absent.events[0].attributes;const explicit=restFixture();explicit.events[0].attributes={measure_rest:false};
  const before=JSON.stringify(explicit);expect(project(explicit).mei).toBe(project(absent).mei);
  expect(project(explicit).mei).toContain('<rest xml:id="ss-e0" dur="2" dots="0"/>');expect(JSON.stringify(explicit)).toBe(before);
 });
 it('withholds non-boolean indications instead of treating them as true or false',()=>{const v=restFixture();v.events[0].attributes={measure_rest:'false'};expect(()=>project(v)).toThrow('must be a boolean');});
 it('requires a measured rest at the exact start spanning the actual measure',()=>{
  for(const [time,duration] of [['0','1'],['1','1'],['0','3']]){const v=restFixture();Object.assign(v.events[0],{time,duration});expect(()=>project(v)).toThrow('exact measure boundary');}
  const grace=restFixture();grace.events[0].duration='?';expect(()=>project(grace)).toThrow('Grace-note layout is not yet supported');
 });
 it.each(['C4','_'])('withholds full-measure indications on %s',pitch=>{const v=restFixture();v.events[0].pitch=pitch;expect(()=>project(v)).toThrow('cannot be pitched, spaces, or chord members');});
 it('withholds chord-member full-measure rests',()=>{const v=restFixture();v.events[0].chord='c';expect(()=>project(v)).toThrow('chord members');});
 it('withholds explicit written spelling instead of silently dropping it',()=>{
  for(const written_duration of ['1','2'])for(const note_size of ['full','cue']){const v=restFixture();v.events[0].written_duration=written_duration;v.events[0].attributes!.note_size=note_size;expect(()=>project(v)).toThrow('Explicit written spelling');}
  const v=restFixture();v.events[0].dots=1;expect(()=>project(v)).toThrow('Explicit written spelling');
 });
 it('withholds explicit size attributes and stemlets on full-measure rests',()=>{
  const v=restFixture();v.events[0].attributes!.note_size='cue';expect(()=>project(v)).toThrow('graphic size on a full-measure rest');
  v.events[0].attributes!.note_size='full';expect(()=>project(v)).toThrow('graphic size on a full-measure rest');
  v.events[0].attributes!.note_size='large';expect(()=>project(v)).toThrow('graphic note size');
  v.events[0].attributes={measure_rest:true,stem:'up'};expect(()=>project(v)).toThrow('Rest stemlets');
 });
 it('withholds irregular full-measure rests instead of borrowing nominal meter duration',()=>{
  for(const length of ['1','10','5/3']){const v=restFixture();v.measures[0].length=length;v.events[0].duration=length;const before=JSON.stringify(v);expect(()=>project(v)).toThrow('metrically irregular');expect(JSON.stringify(v)).toBe(before);}
 });
 it('withholds mRest when another staff would make the shared measure metrically irregular',()=>{
  const v=restFixture();v.parts[0].staves=2;v.states.push({time:'0',scope:'p.s2',values:{meter:'4/4',clef:'F4',key:0}});
  expect(()=>project(v)).toThrow('metrically irregular');
 });
});

describe('pinned Verovio note-appearance fidelity',()=>{
 it('renders opposite individual and chord stems with visible direction and retained IDs',async()=>{
  const toolkit=new VerovioToolkit(await createVerovioModule());
  expect((toolkit as VerovioToolkit & {getVersion():string}).getVersion()).toMatch(/^5\.6\.0/);
  toolkit.setOptions({pageWidth:2100,pageHeight:2600,scale:40,adjustPageHeight:true,header:'none',footer:'none',font:'Bravura'});
  const v=fixture();v.measures[0].length='4';v.states[0].values.meter='4/4';
  v.events=Array.from({length:4},(_,i)=>({...v.events[0],id:`note-${i}`,time:String(i),attributes:{stem:i%2?'down':'up'}}));
  v.events[2].chord='up-chord';v.events[3].chord='down-chord';
  v.events.push({...v.events[2],id:'up-member',pitch:'E4'},{...v.events[3],id:'down-member',pitch:'E4'});
  const result=project(v);expect(toolkit.loadData(result.mei)).toBeTruthy();const svg=parse(toolkit.renderToSVG(1));
  const dy=(element:Element|null)=>{const d=element?.querySelector('.stem path')?.getAttribute('d');const coords=d?.match(/^M([\d.-]+) ([\d.-]+) L([\d.-]+) ([\d.-]+)$/);expect(coords).not.toBeNull();return Number(coords![4])-Number(coords![2]);};
  expect(dy(svg.querySelector('[id="ss-e0"]'))).toBeLessThan(0);expect(dy(svg.querySelector('[id="ss-e1"]'))).toBeGreaterThan(0);
  expect(dy(svg.querySelector('[id="ss-e2"]')?.closest('.chord')??null)).toBeLessThan(0);
  expect(dy(svg.querySelector('[id="ss-e4"]')?.closest('.chord')??null)).toBeGreaterThan(0);
  for(const id of Object.keys(result.map))expect(svg.querySelector(`[id="${id}"]`)).not.toBeNull();
 },10000);
 it('renders true non-whole-note measure rests with retained identity and exact measure extent',async()=>{
  const toolkit=new VerovioToolkit(await createVerovioModule());
  toolkit.setOptions({pageWidth:2100,pageHeight:2600,scale:40,adjustPageHeight:true,header:'none',footer:'none',font:'Bravura'});
  const v=restFixture();v.states[0].values.meter='3/4';v.measures[0].length='3';v.events[0].duration='3';
  v.measures.push({...v.measures[0],id:'m2',time:'3',label:'2'});v.events.push({...v.events[0],id:'second-rest',time:'3',attributes:{measure_rest:true}});
  const result=project(v);expect(toolkit.loadData(result.mei)).toBeTruthy();const svg=parse(toolkit.renderToSVG(1));
  const scale=(id:string)=>Number(svg.querySelector(`[id="${id}"] use`)?.getAttribute('transform')?.match(/scale\(([^,)]+)/)?.[1]);
  for(const id of Object.keys(result.map))expect(svg.querySelector(`[id="${id}"]`)?.getAttribute('class')).toBe('mRest');
  expect(scale('ss-e0')).toBeGreaterThan(0);expect(scale('ss-e1')).toBeGreaterThan(0);expect(scale('ss-e1')).toBe(scale('ss-e0'));
  expect(svg.querySelector('[id="ss-e0"] use')?.getAttribute('xlink:href')).toMatch(/^#E4E3-/);
  const timemap=(toolkit as VerovioToolkit & {renderToTimemap(options:Record<string,boolean>):Array<{qstamp:number;restsOn?:string[];restsOff?:string[]}>}).renderToTimemap({includeMeasures:true,includeRests:true});
  expect(timemap.find(t=>t.restsOn?.includes('ss-e0'))?.qstamp).toBe(0);expect(timemap.find(t=>t.restsOff?.includes('ss-e0'))?.qstamp).toBe(3);
  expect(timemap.find(t=>t.restsOn?.includes('ss-e1'))?.qstamp).toBe(3);expect(timemap.find(t=>t.restsOff?.includes('ss-e1'))?.qstamp).toBe(6);
  expect(result.map).toEqual({'ss-e0':'original-rest','ss-e1':'second-rest'});
 },10000);
});
