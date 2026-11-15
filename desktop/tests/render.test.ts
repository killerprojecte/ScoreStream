// @vitest-environment jsdom
import {describe,it,expect} from 'vitest';
import createVerovioModule from 'verovio/wasm';
import {VerovioToolkit} from 'verovio/esm';
import {Q,rhythm} from '../src/render/exact';
import {project} from '../src/render/project';
import {LatestOnly} from '../src/render/scheduler';
import type {View,RenderRequest,RenderResult} from '../src/types';
const fixture=():View=>({valid:true,diagnostics:[],source:'backend-only',document:{frames:[]},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false,parts:[{id:'p',instrument:'Piano & strings',staves:1}],voices:[{id:'v',part:'p',staff:1}],states:[{time:'0',scope:'p.s1',values:{meter:'4/4',clef:'G2',key:0}}],measures:[{id:'m',part:'p',time:'0',length:'4',label:'1'}],events:[{id:'n1',voice:'v',pitch:'C#4',time:'0',duration:'1',staff:1,chord:null,written_duration:null,dots:0,source_line:5},{id:'n2',voice:'v',pitch:'r',time:'1',duration:'3',staff:1,chord:null,written_duration:null,dots:1,source_line:6}]});
describe('bounded typed MEI projection',()=>{
 it('uses exact rhythm without floating point',()=>{expect(new Q('90071992547409930000000001/3').sub(new Q('90071992547409930000000000/3')).eq(new Q('1/3'))).toBe(true);expect(rhythm(new Q('3/2'))).toEqual({dur:4,dots:1});expect(()=>rhythm(new Q('1/3'))).toThrow();});
 it('escapes labels and keeps stable event identity',()=>{const result=project(fixture());expect(result.mei).toContain('Piano &amp; strings');expect(result.mei).toContain('accid="s"');expect(result.map).toEqual({'ss-e0':'n1','ss-e1':'n2'});});
 it('uses independent escaped part labels without changing routing or events',()=>{
  const v=fixture();v.parts[0].instrument='flute';v.parts[0].display_name='Flute 1 <solo> & "lead"\nnext\tline';
  const result=project(v);
  expect(result.mei).toContain('<label>Flute 1 &lt;solo&gt; &amp; &quot;lead&quot;&#10;next&#9;line</label>');
  expect(result.mei).not.toContain('<solo>');expect(v.parts[0].instrument).toBe('flute');expect(result.map).toEqual({'ss-e0':'n1','ss-e1':'n2'});
 });
 it('distinguishes explicit empty part labels from missing labels',()=>{
  const v=fixture();v.parts[0].display_name='';expect(project(v).mei).toContain('<staffGrp><label></label>');
  delete v.parts[0].display_name;expect(project(v).mei).toContain('<staffGrp><label>Piano &amp; strings</label>');
 });
 it('keeps two labels for parts sharing one instrument',()=>{
  const v=fixture();v.parts[0]={id:'p',instrument:'flute',staves:1,display_name:'Flute 1'};
  v.parts.push({id:'p2',instrument:'flute',staves:1,display_name:'Flute 2'});
  v.voices.push({id:'v2',part:'p2',staff:1});v.measures.push({...v.measures[0],part:'p2',id:'m2'});
  v.states.push({...v.states[0],scope:'p2.s1'});
  v.events.push({...v.events[0],id:'n3',voice:'v2'},{...v.events[1],id:'n4',voice:'v2'});
  const result=project(v);expect(result.mei).toContain('<label>Flute 1</label>');expect(result.mei).toContain('<label>Flute 2</label>');expect(Object.keys(result.map)).toHaveLength(4);
 });
 it('engraves independent labels with the real pinned Verovio renderer',async()=>{
  const toolkit=new VerovioToolkit(await createVerovioModule());
  toolkit.setOptions({pageWidth:2100,pageHeight:2600,scale:40,adjustPageHeight:true,breaks:'auto',header:'none',footer:'none',font:'Bravura',spacingSystem:14,pageMarginTop:40,pageMarginBottom:40,pageMarginLeft:30,pageMarginRight:30,svgViewBox:true});
  const v=fixture();v.parts[0]={id:'p',instrument:'flute',staves:1,display_name:'Flute 1 <script>literal</script> & lead'};
  v.parts.push({id:'p2',instrument:'flute',staves:1,display_name:'Flute 2'});
  v.voices.push({id:'v2',part:'p2',staff:1});v.measures.push({...v.measures[0],part:'p2',id:'m2'});v.states.push({...v.states[0],scope:'p2.s1'});
  v.events.push({...v.events[0],id:'n3',voice:'v2'},{...v.events[1],id:'n4',voice:'v2'});
  expect(toolkit.loadData(project(v).mei)).toBeTruthy();let svg=toolkit.renderToSVG(1).replaceAll('\u00a0',' ');
  expect(svg).toContain('Flute 1 &lt;script&gt;literal&lt;/script&gt; &amp; lead');expect(svg).toContain('Flute 2');expect(svg).not.toContain('<script>');
  v.parts[0].display_name='';delete v.parts[1].display_name;
  expect(toolkit.loadData(project(v).mei)).toBeTruthy();svg=toolkit.renderToSVG(1).replaceAll('\u00a0',' ');
  expect(svg).not.toContain('Flute 1');expect(svg).not.toContain('Flute 2');expect(svg.match(/>flute<\/tspan>/g)).toHaveLength(1);
 },10000);
 it('refuses invalid snapshots and unsupported pitches',()=>{const v=fixture();v.valid=false;expect(()=>project(v)).toThrow('Rust-validated');v.valid=true;v.events[0].pitch='C(+1/3)4';expect(()=>project(v)).toThrow('not supported');});
 it('refuses missing measures instead of dropping events',()=>{const v=fixture();v.events[0].time='4';expect(()=>project(v)).toThrow('outside');});
 it('refuses unequal chord rhythms',()=>{const v=fixture();v.events[0].chord='c';v.events[1]={...v.events[1],pitch:'E4',time:'0',duration:'2',chord:'c'};expect(()=>project(v)).toThrow('Unequal-duration');});
 it('refuses cross-bar duration instead of clipping',()=>{const v=fixture();v.events[1].duration='4';expect(()=>project(v)).toThrow('Cross-bar');});
 it('refuses state changes instead of displaying wrong clef',()=>{const v=fixture();v.states.push({time:'2',scope:'p.s1',values:{clef:'F4'}});expect(()=>project(v)).toThrow('Mid-score');});
});
describe('latest-only renderer scheduling',()=>{
 it('rejects stale results even after a newer invalid draft and bounds queue',()=>{
  const sent:RenderRequest[]=[],published:RenderResult[]=[];
  const worker:{onmessage:((e:MessageEvent<RenderResult>)=>void)|null;postMessage:(x:RenderRequest)=>void}={onmessage:null,postMessage:r=>sent.push(r)};
  const scheduler=new LatestOnly(worker as unknown as Worker,r=>published.push(r));
  const request=(id:number):RenderRequest=>({requestId:id,key:{kind:'draft',session:'s',sequence:id,hash:String(id),baseRevision:'r1',layout:'x'},mei:'',width:600,map:{}});
  const finish=(id:number)=>worker.onmessage!({data:{...request(id),pages:[],elapsed:1}} as unknown as MessageEvent<RenderResult>);
  scheduler.invalidate(1);scheduler.submit(request(1));scheduler.invalidate(2);scheduler.submit(request(2));scheduler.invalidate(3);scheduler.submit(request(3));expect(sent.map(x=>x.requestId)).toEqual([1]);finish(1);expect(sent.map(x=>x.requestId)).toEqual([1,3]);expect(published).toEqual([]);
  scheduler.invalidate(4);finish(3);expect(published).toEqual([]);
  scheduler.submit(request(4));finish(4);expect(published.map(x=>x.requestId)).toEqual([4]);
 });
});

describe('irregular extents and graphic note size',()=>{
 it('keeps written meter while marking a ten-quarter measure metrically irregular',()=>{
  const v=fixture();v.states[0].values.meter='2/4';v.measures[0].length='10';
  v.events=Array.from({length:10},(_,i)=>({...v.events[0],id:`n${i}`,time:String(i),duration:'1',written_duration:'4',attributes:{note_size:'cue'}}));
  const result=project(v);expect(result.mei).toContain('meter.count="2" meter.unit="4"');expect(result.mei).toContain('<measure n="1" metcon="false">');
  expect(result.mei.match(/cue="true"/g)).toHaveLength(10);expect(Object.keys(result.map)).toHaveLength(10);expect(result.warnings.some(w=>w.includes('actual length 10'))).toBe(true);
 });
 it('rejects unimplemented graphic sizes and mixed chord sizes without losing them',()=>{
  for(const size of ['grace-cue','large','tiny']) { const v=fixture();v.events[0].attributes={note_size:size};expect(()=>project(v)).toThrow('graphic note size'); }
  const v=fixture();v.events[0].attributes={note_size:'cue'};v.events[0].chord='c';v.events[1]={...v.events[0],id:'other',pitch:'E4',attributes:{note_size:'full'}};expect(()=>project(v)).toThrow('Mixed graphic sizes');
 });
 it('renders smaller normal-note glyphs with real Verovio and retains all event IDs',async()=>{
  const toolkit=new VerovioToolkit(await createVerovioModule());
  toolkit.setOptions({pageWidth:2100,pageHeight:2600,scale:40,adjustPageHeight:true,header:'none',footer:'none',font:'Bravura'});
  const v=fixture();v.states[0].values.meter='2/4';v.measures[0].length='10';
  v.events=Array.from({length:10},(_,i)=>({...v.events[0],id:`n${i}`,time:String(i),pitch:'D5',duration:'1',written_duration:'4',attributes:i?{note_size:'cue'}:{note_size:'full'}}));
  const projected=project(v);expect(toolkit.loadData(projected.mei)).toBeTruthy();const svg=toolkit.renderToSVG(1);
  const dom=new DOMParser().parseFromString(svg,'image/svg+xml');
  const scale=(id:string)=>dom.querySelector(`[id="${id}"] .notehead use`)?.getAttribute('transform')?.match(/scale\(([^,)]+)/)?.[1];
  const normal=Number(scale('ss-e0')),cue=Number(scale('ss-e1'));
  expect(normal).toBeGreaterThan(0);expect(cue).toBeGreaterThan(0);expect(cue).toBeLessThan(normal);
  for(const id of Object.keys(projected.map))expect(dom.querySelector(`[id="${id}"]`)).not.toBeNull();
 },10000);
});
