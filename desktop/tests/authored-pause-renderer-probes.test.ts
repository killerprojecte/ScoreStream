// @vitest-environment jsdom
// Renderer characterization ONLY. These fixtures never enable pause projection.
import {beforeAll,describe,it,expect} from 'vitest';
import {writeFileSync} from 'node:fs';
import createVerovioModule from 'verovio/wasm';
import {VerovioToolkit} from 'verovio/esm';
import {project} from '../src/render/project';
import {sanitizeSvg} from '../src/render/sanitize';
import type {View} from '../src/types';

const fixture=():View=>({valid:true,diagnostics:[],source:'Passive source is never converted by these probes.',document:{frames:[]},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false,parts:[{id:'p',instrument:'piano',staves:1}],voices:[{id:'v',part:'p',staff:1}],states:[{time:'0',scope:'p.s1',values:{meter:'4/4',clef:'G2',key:0}}],measures:[{id:'m',part:'p',time:'0',length:'4',label:'1'}],events:[{id:'a',voice:'v',pitch:'C4',time:'0',duration:'1',staff:1,chord:null,written_duration:'4',dots:0,source_line:1},{id:'next',voice:'v',pitch:'D4',time:'1',duration:'1',staff:1,chord:null,written_duration:'4',dots:0,source_line:2}]});
const parse=(text:string)=>new DOMParser().parseFromString(text,'application/xml');
const byId=(doc:Document,id:string)=>doc.querySelector(`[id="${id}"]`)!;
const glyphs=(element:Element)=>[...element.querySelectorAll('use')].map(e=>e.getAttribute('xlink:href')!.match(/^#([0-9A-F]+)-/)![1]);
const position=(element:Element)=>{const m=/translate\(([-\d.]+),\s*([-\d.]+)\) scale\(([-\d.]+)/.exec(element.querySelector('use')!.getAttribute('transform')!)!;return {x:Number(m[1]),y:Number(m[2]),scale:Number(m[3])};};
// Exact pinned data/Bravura.xml metrics, not guessed screen rectangles.
// https://github.com/rism-digital/verovio/blob/version-5.6.0/data/Bravura.xml
const metrics:Record<string,[number,number,number,number]>={E4CE:[1,2,151,249],E4D1:[0,-1,384,533],E4D2:[0,0,663,532],E4D3:[0,0,186,533],E4D4:[0,0,373,530],F42C:[0,-1,224,533],E4D7:[0,0,51,533],E4A0:[0,1,339,244],E4A1:[0,-244,339,244],E4A2:[0,0,84,84],E4A3:[0,-84,84,84],E4A4:[-1,0,339,48],E4A5:[-1,-48,339,48],E0A4:[0,-125,295,250],E0A3:[0,-125,295,250],E0A2:[0,-125,422,250]};
const box=(element:Element)=>{const p=position(element),[x,y,w,h]=metrics[glyphs(element)[0]];return {left:p.x+x*p.scale,right:p.x+(x+w)*p.scale,top:p.y-(y+h)*p.scale,bottom:p.y-y*p.scale};};
const disjoint=(a:ReturnType<typeof box>,b:ReturnType<typeof box>)=>a.right<=b.left||b.right<=a.left||a.bottom<=b.top||b.bottom<=a.top;
const stemBoxes=(svg:Document)=>[...svg.querySelectorAll('.stem path')].map(e=>{const n=e.getAttribute('d')!.match(/-?\d+(?:\.\d+)?/g)!.map(Number),r=Number(e.getAttribute('stroke-width')??0)/2;return {left:Math.min(n[0],n[2])-r,right:Math.max(n[0],n[2])+r,top:Math.min(n[1],n[3])-r,bottom:Math.max(n[1],n[3])+r};});
const head=(svg:Document,id='ss-e0')=>byId(svg,id).querySelector('.notehead')!;
const staffBounds=(note:Element)=>{const ys=[...note.closest('.staff')!.children].filter(e=>e.localName==='path').map(e=>Number(/^M[-\d.]+ ([-\d.]+)/.exec(e.getAttribute('d')!)![1]));return {top:Math.min(...ys),bottom:Math.max(...ys)};};
const breaths={comma:'E4CE',tick:'E4CF',upbow:'E4D0',salzedo:'E4D5'} as const;
const caesuras={normal:'E4D1',thick:'E4D2',short:'E4D3',curved:'E4D4',single:'F42C'} as const;
const control=(tag:'breath'|'caesura',attrs:string,id='pause')=>`<${tag} xml:id="${id}" staff="1" ${attrs}/>`;

describe('pinned pause renderer characterization not runtime support',()=>{
 let toolkit:VerovioToolkit;
 beforeAll(async()=>{toolkit=new VerovioToolkit(await createVerovioModule());expect((toolkit as VerovioToolkit&{getVersion():string}).getVersion()).toBe('5.6.0-85e7620');toolkit.setOptions({pageWidth:2100,pageHeight:2600,scale:40,adjustPageHeight:true,header:'none',footer:'none',font:'Bravura'});},10000);
 const render=(name:string,v:View,controls:string,defs='',extraNoteChildren='')=>{
  let mei=project(v).mei.replace('</scoreDef>',`${defs}</scoreDef>`).replace('</measure>',`${controls}</measure>`);
  if(extraNoteChildren)mei=mei.replace(/(<note xml:id="ss-e0"[^>]*)(\/>)/,`$1>${extraNoteChildren}</note>`);
  expect(toolkit.loadData(mei)).toBeTruthy();const raw=toolkit.renderToSVG(1),svg=parse(sanitizeSvg(raw));expect(svg.querySelector('parsererror')).toBeNull();
  const objects=[...svg.querySelectorAll('.breath,.caesura,.artic')].map(e=>({id:e.id,glyphs:glyphs(e),...(e.querySelector('use')?{position:position(e),bounds:box(e)}:{})}));
  if(process.env.AUTHORED_PAUSE_PROBE_DIR){const dir=process.env.AUTHORED_PAUSE_PROBE_DIR;writeFileSync(`${dir}/${name}.svg`,new XMLSerializer().serializeToString(svg));writeFileSync(`${dir}/${name}.mei`,mei);writeFileSync(`${dir}/${name}.json`,JSON.stringify({purpose:'renderer characterization, not runtime support',view:v,objects,noteheads:[...svg.querySelectorAll('.notehead')].map(e=>({noteId:e.closest('.note')?.id,position:position(e),bounds:box(e)})),stems:stemBoxes(svg),dots:[...svg.querySelectorAll('.dots ellipse')].map(e=>({cx:e.getAttribute('cx'),cy:e.getAttribute('cy'),rx:e.getAttribute('rx'),ry:e.getAttribute('ry')})),barlines:[...svg.querySelectorAll('.barLine path')].map(e=>e.getAttribute('d'))},null,2));}
  return svg;
 };
 it('proves omitted and all four breath forms including ignored overrides missing glyphs and empty alternate symbols',()=>{
  for(const shape of [undefined,...Object.keys(breaths)] as (keyof typeof breaths|undefined)[]){
   const code=breaths[shape??'comma'],v=fixture(),name=shape??'omitted';
   const glyphAttr=shape===undefined?'':` glyph.auth="smufl" glyph.num="U+${code}"`;
   const svg=render(`breath-${name}-override`,v,control('breath',`tstamp="1.5" place="above"${glyphAttr}`));
   expect(glyphs(byId(svg,'pause'))).toEqual(['E4CE']);
   const carrier=render(`breath-${name}-font-availability`,v,control('caesura',`tstamp="1.5" place="above" glyph.auth="smufl" glyph.num="U+${code}"`));
   expect(glyphs(byId(carrier,'pause'))).toEqual([shape===undefined||shape==='comma'?'E4CE':'E4D1']);
   if(shape!==undefined&&shape!=='comma')expect(carrier.querySelector(`[id^="${code}-"]`)).toBeNull();
   const defs=`<symbolTable><symbolDef xml:id="fixed-probe"><symbol glyph.auth="smufl" glyph.num="U+${code}"/></symbolDef></symbolTable>`;
   const alternate=byId(render(`breath-${name}-altsym`,v,control('breath','tstamp="1.5" place="above" altsym="#fixed-probe"'),defs),'pause');
   expect(alternate).not.toBeNull();expect(alternate.querySelectorAll('use,path,text')).toHaveLength(0);
  }
 });
 it('verifies every caesura form and omitted normal actual path independently from placement and stem',()=>{
  for(const shape of [undefined,...Object.keys(caesuras)] as (keyof typeof caesuras|undefined)[])for(const placement of ['above','below'])for(const stem of ['up','down']){
   const v=fixture();v.events[0].attributes={stem};const code=caesuras[shape??'normal'],attrs=shape===undefined?'':`glyph.auth="smufl" glyph.num="U+${code}"`;
   const svg=render(`caesura-${shape??'omitted'}-${placement}-${stem}`,v,control('caesura',`tstamp="1.5" place="${placement}" ${attrs}`)),mark=byId(svg,'pause'),staff=staffBounds(head(svg));
   expect(glyphs(mark)).toEqual([code]);expect(byId(svg,mark.querySelector('use')!.getAttribute('xlink:href')!.slice(1)).querySelector('path')!.getAttribute('d')!.length).toBeGreaterThan(20);
   expect(placement==='below'?box(mark).top>staff.bottom:box(mark).bottom<staff.top).toBe(true);
  }
  const named=render('caesura-single-name-is-different',fixture(),control('caesura','tstamp="1.5" place="above" glyph.auth="smufl" glyph.name="caesuraSingleStroke"'));
  expect(glyphs(byId(named,'pause'))).toEqual(['E4D7']);expect(metrics.E4D7[2]).not.toBe(metrics.F42C[2]);
 });
 it('records native omitted placement separately from explicit placement defaults',()=>{
  for(const tag of ['breath','caesura'] as const){
   const svg=render(`${tag}-omitted-placement`,fixture(),control(tag,'tstamp="1.5"')),mark=byId(svg,'pause'),staff=staffBounds(head(svg));
   expect(glyphs(mark)).toEqual([tag==='breath'?'E4CE':'E4D1']);
   if(tag==='breath')expect(box(mark).bottom).toBeLessThan(staff.top);
   else expect(box(mark).top<staff.top&&box(mark).bottom>staff.top).toBe(true);
  }
 });
 it('demonstrates startid on-head breath ignored horizontal offsets and release timestamp at the next note',()=>{
  const v=fixture(),base=render('breath-startid-on-head',v,control('breath','startid="#ss-e0" place="above"'));
  const positioned=render('breath-startid-ho-ignored',v,control('breath','startid="#ss-e0" ho="2" place="above"'));
  expect(position(byId(positioned,'pause'))).toEqual(position(byId(base,'pause')));
  expect(position(byId(base,'pause')).x).toBeLessThan(box(head(base)).right);
  expect(position(byId(base,'pause')).x).toBeGreaterThan(box(head(base)).left);
  for(const tag of ['breath','caesura'] as const){
   const end=render(`${tag}-release-at-next-note`,v,control(tag,'tstamp="2" place="above"'));
   expect(position(byId(end,'pause')).x).toBe(position(head(end,'ss-e1')).x);
  }
 });
 it('reproduces midpoint failure on a short note and end-relative failure on displaced second chords',()=>{
  const short=fixture();short.events[0].duration=short.events[1].duration='1/4';short.events[0].written_duration=short.events[1].written_duration='16';short.events[1].time='1/4';short.measures[0].length='1/2';
  const midpoint=render('midpoint-sixteenth-overlaps-notehead',short,control('breath','tstamp="1.125" place="above"'));
  expect(position(byId(midpoint,'pause')).x).toBeLessThan(box(head(midpoint)).right);
  const chords=fixture();chords.events[0].chord='ch';chords.events[0].attributes={stem:'up'};chords.events.splice(1,0,{...chords.events[0],id:'second',pitch:'D4'});
  const displaced=render('end-relative-second-chord-overlaps-notehead',chords,control('breath','tstamp="1.5" place="above"'));
  expect(position(byId(displaced,'pause')).x).toBeLessThan(Math.max(box(head(displaced)).right,box(head(displaced,'ss-e1')).right));
 });
 it('records full caesura extents at following-note columns and final barlines in dense passages',()=>{
  const v=fixture();v.events=Array.from({length:4},(_,i)=>({...v.events[0],id:`n${i}`,time:String(i),pitch:i%2?'D4':'C4'}));
  const svg=render('end-relative-dense-thick-caesuras',v,Array.from({length:4},(_,i)=>control('caesura',`tstamp="${i+1.5}" place="above" glyph.auth="smufl" glyph.num="U+E4D2"`,`pause${i}`)).join(''));
  const spans=Array.from({length:3},(_,i)=>({right:box(byId(svg,`pause${i}`)).right,next:box(head(svg,`ss-e${i+1}`)).left}));
  expect(spans.some(s=>s.right>=s.next)).toBe(true);
  const barline=svg.querySelector('.barLine path')!,barlineX=Number(/^M([-\d.]+)/.exec(barline.getAttribute('d')!)![1]);
  expect(box(byId(svg,'pause3')).right).toBeGreaterThan(barlineX);
 });
 it('qualifies only local duplicate placement and compound separation without promoting horizontal acceptance',()=>{
  for(const placement of ['above','below'])for(const stem of ['up','down']){
   const v=fixture();v.events[0].attributes={stem};
   const controls=control('breath',`tstamp="1.5" place="${placement}"`,'b1')+control('caesura',`tstamp="1.5" place="${placement}" glyph.num="U+E4D2"`,'c1')+control('breath',`tstamp="1.5" place="${placement}"`,'b2')+control('caesura',`tstamp="1.5" place="${placement}" glyph.num="U+F42C"`,'c2');
   const compound=`<artic xml:id="dot" artic="stacc" place="${placement}"/><artic xml:id="line" artic="ten" place="${placement}"/><artic xml:id="accent" artic="acc" place="${placement}"/>`;
   const svg=render(`local-stacking-${placement}-${stem}`,v,controls,'',compound),objects=['b1','c1','b2','c2','dot','line','accent'].map(id=>byId(svg,id));
   expect(svg.querySelectorAll('.breath,.caesura,.artic')).toHaveLength(7);
   for(let i=0;i<objects.length;i++)for(let j=i+1;j<objects.length;j++)expect(disjoint(box(objects[i]),box(objects[j]))).toBe(true);
   for(const object of objects)for(const stem of stemBoxes(svg))expect(disjoint(box(object),stem)).toBe(true);
   expect(placement==='above'?position(byId(svg,'dot')).y>position(byId(svg,'line')).y:position(byId(svg,'dot')).y<position(byId(svg,'line')).y).toBe(true);
   expect(glyphs(byId(svg,'c1'))).toEqual(['E4D2']);expect(glyphs(byId(svg,'c2'))).toEqual(['F42C']);
  }
 });
 it('records dotted host bounds and exact chord voice staff associations as local evidence only',()=>{
  const v=fixture();v.events[0].duration='3/2';v.events[0].dots=1;v.events[1].time='3/2';v.measures[0].length='5/2';v.events[0].chord='ch';v.events.splice(1,0,{...v.events[0],id:'member',pitch:'E4'});
  v.parts[0].staves=2;v.voices.push({id:'other',part:'p',staff:1},{id:'bass',part:'p',staff:2});v.states.push({time:'0',scope:'p.s2',values:{meter:'4/4',clef:'F4',key:0}});v.events.push({...v.events[0],id:'voice',voice:'other',pitch:'G3',chord:null},{...v.events[0],id:'bass',voice:'bass',pitch:'C3',staff:2,chord:null});
  // This is a direct native fixture, not an authored projection receipt.
  const base=project(v),ids=Object.fromEntries(Object.entries(base.map).map(([id,event])=>[event,id]));
  const controls=['a','member','voice','bass'].map((event,i)=>`<caesura xml:id="p${i}" corresp="#${ids[event]}" staff="${event==='bass'?2:1}" tstamp="2" place="${i%2?'below':'above'}" glyph.num="U+F42C"/>`).join('');
  const svg=render('local-dotted-polyphonic-associations',v,controls),chord=byId(svg,ids.a).closest('.chord')!,dots=[...chord.querySelectorAll('.dots ellipse')].map(e=>Number(e.getAttribute('cx'))+Number(e.getAttribute('rx')));
  expect(dots.length).toBeGreaterThan(0);expect(svg.querySelectorAll('.caesura')).toHaveLength(4);
  expect(position(byId(svg,'p0')).x).toBeGreaterThan(Math.max(...dots,...[...chord.querySelectorAll('.notehead')].map(e=>box(e).right)));
  for(const [i,event] of ['a','member','voice','bass'].entries()){const mark=byId(svg,`p${i}`),staff=staffBounds(head(svg,ids[event]));expect(i%2?box(mark).top>staff.bottom:box(mark).bottom<staff.top).toBe(true);}
 });
 it('keeps all visible authored pause shapes withheld without source mutation',()=>{
  for(const [sign,shapes] of [['breath-mark',[undefined,...Object.keys(breaths)]],['caesura',[undefined,...Object.keys(caesuras)]]] as const)for(const shape of shapes){
   const v=fixture();v.document.required_extensions=['scorestream.articulation@1'];
   const rows=[['event',{event_index:0}],['notations',{event_index:0,wrapper_index:0}],['group',{event_index:0,wrapper_index:0,group_index:0}],['item',{event_index:0,wrapper_index:0,group_index:0,item_index:0,sign,...shape===undefined?{}:{shape}}]] as const;
   for(const [kind,attributes] of rows)v.document.frames!.push({time:'0',records:[{record:'mark',kind:`scorestream.articulation.${kind}`,target:'#a',attributes}]});
   const before=JSON.stringify(v);expect(()=>project(v)).toThrow(`notations[0]/group[0]/item[0] (${sign}): unsupported ${sign} engraving`);expect(JSON.stringify(v)).toBe(before);
  }
 });
});
