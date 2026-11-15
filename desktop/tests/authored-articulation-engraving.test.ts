// @vitest-environment jsdom
import {beforeAll,describe,it,expect} from 'vitest';
import {writeFileSync} from 'node:fs';
import createVerovioModule from 'verovio/wasm';
import {VerovioToolkit} from 'verovio/esm';
import {project} from '../src/render/project';
import {authoredCapabilities,authoredSigns,decodeAuthoredArticulations} from '../src/render/authored-articulations';
import type {MarkRecord,ScoreEvent,View} from '../src/types';
import family from './fixtures/authored-articulation-family.json';

type Attributes=MarkRecord['attributes'];
const fixture=():View=>({valid:true,diagnostics:[],source:'Passive source: never parsed by this adapter.',document:{frames:[],required_extensions:['scorestream.articulation@1']},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false,parts:[{id:'p',instrument:'piano',staves:1}],voices:[{id:'v',part:'p',staff:1}],states:[{time:'0',scope:'p.s1',values:{meter:'4/4',clef:'G2',key:0}}],measures:[{id:'m',part:'p',time:'0',length:'4',label:'1'}],events:[{id:'a',voice:'v',pitch:'C4',time:'0',duration:'1',staff:1,chord:null,written_duration:'4',dots:0,source_line:1}]});
function record(v:View,kind:string,attributes:Attributes,event:ScoreEvent=v.events[0]){
 const mark:MarkRecord={record:'mark',kind:`scorestream.articulation.${kind}`,target:`#${event.id}`,attributes};
 v.document.frames!.push({time:event.time,records:[mark]});return mark;
}
function attach(v:View,groups:Attributes[][],eventIndex=0,wrapperIndex=0,wrapper:Attributes={}){
 const event=v.events[eventIndex];
 if(wrapperIndex===0)record(v,'event',{event_index:eventIndex},event);
 record(v,'notations',{event_index:eventIndex,wrapper_index:wrapperIndex,...wrapper},event);
 groups.forEach((items,g)=>{const indices={event_index:eventIndex,wrapper_index:wrapperIndex,group_index:g};record(v,'group',indices,event);items.forEach((item,i)=>record(v,'item',{...indices,item_index:i,...item},event));});
}
const parse=(text:string)=>new DOMParser().parseFromString(text,'application/xml');
const byId=(doc:Document,id:string)=>doc.querySelector(`[id="${id}"]`)!;
const glyphs=(element:Element)=>[...element.querySelectorAll('use')].map(e=>e.getAttribute('xlink:href')!.match(/^#([0-9A-F]+)-/)![1]);
const position=(element:Element)=>{const transform=element.querySelector('use')!.getAttribute('transform')!;const m=/translate\(([-\d.]+),\s*([-\d.]+)\)/.exec(transform)!;return {x:Number(m[1]),y:Number(m[2])};};
const expected:Record<string,[string[],string[]]>={
 accent:[['E4A0'],['E4A1']],staccato:[['E4A2'],['E4A3']],tenuto:[['E4A4'],['E4A5']],
 staccatissimo:[['E4A8'],['E4A9']],'strong-accent':[['E4AC'],['E4AC']],
 'soft-accent':[['ED40'],['ED41']],spiccato:[['E4AA'],['E4AB']],stress:[['E4B6'],['E4B7']],unstress:[['E4B8'],['E4B9']],
 'detached-legato':[['E4A2','E4A4'],['E4A3','E4A5']],
};
const marked=(sign:string,placement?:string)=>{const v=fixture();attach(v,[[{sign,...(placement?{placement}:{})}]]);return v;};

describe('bounded authored family transport and receipts',()=>{
 it('enumerates all seventeen family capabilities explicitly',()=>{
  expect(Object.keys(authoredCapabilities).sort()).toEqual([...authoredSigns].sort());
  expect(Object.values(authoredCapabilities).filter(x=>x==='native')).toHaveLength(7);
  expect(Object.values(authoredCapabilities).filter(x=>x==='explicit-placement')).toHaveLength(3);
  expect(Object.values(authoredCapabilities).filter(x=>x==='unsupported')).toHaveLength(7);
 });
 it('preserves ordered empty/repeated wrappers, groups, signs and source identities without reusing source IDs as SVG IDs',()=>{
  const v=fixture();attach(v,[[],[{sign:'staccato',placement:'above'},{sign:'staccato',placement:'above'}]],0,0,{source_id:'wrapper',print_object:true});
  attach(v,[],0,1,{});attach(v,[[{sign:'tenuto',placement:'below'}]],0,2,{print_object:false});
  const rows=v.document.frames!.flatMap(f=>f.records) as MarkRecord[];
  rows.find(r=>r.kind.endsWith('.group')&&r.attributes.group_index===0)!.attributes.source_id='empty';
  rows.find(r=>r.kind.endsWith('.group')&&r.attributes.group_index===1)!.attributes.source_id='repeated';
  const before=JSON.stringify(v),result=project(v),authored=result.authoredArticulations!;
  expect(authored.events[0].notations.map(w=>[w.sourceId,w.printObject,w.groups.length])).toEqual([['wrapper',true,2],[undefined,undefined,0],[undefined,false,1]]);
  expect(authored.events[0].notations[0].groups.map(g=>[g.sourceId,g.items.length])).toEqual([['empty',0],['repeated',2]]);
  expect(authored.events[0].notations[2].groups[0].items[0]).toMatchObject({hidden:true,renderIds:[],sign:'tenuto',placement:'below'});
  expect(Object.keys(result.map).filter(x=>x.startsWith('ss-aa'))).toEqual(['ss-aa-0-0-1-0','ss-aa-0-0-1-1']);
  expect(result.mei).not.toContain('wrapper');expect(result.mei).not.toContain('repeated');expect(result.warnings).toEqual([]);expect(JSON.stringify(v)).toBe(before);
 });
 it('uses indices, not record/frame traversal order, and never merges two sources',()=>{
  const v=fixture();v.events.push({...v.events[0],id:'b',time:'1'});
  attach(v,[[{sign:'accent'}]]);attach(v,[[{sign:'tenuto'}]],1);
  const first=project(v);v.document.frames!.reverse();const second=project(v);
  expect(second.mei).toBe(first.mei);expect(second.authoredArticulations).toEqual(first.authoredArticulations);
  expect(second.map['ss-aa-1-0-0-0']).toBe('b');
 });
 it('distinguishes omitted, yes and no print flags with no source mutation',()=>{
  for(const printObject of [undefined,true,false]){
   const v=fixture();attach(v,[[{sign:'accent'}]],0,0,printObject===undefined?{}:{print_object:printObject});
   const before=JSON.stringify(v),result=project(v),wrapper=result.authoredArticulations!.events[0].notations[0];
   expect(wrapper.printObject).toBe(printObject);expect(parse(result.mei).querySelectorAll('artic')).toHaveLength(printObject===false?0:1);expect(JSON.stringify(v)).toBe(before);
  }
 });
 it('suppresses hidden passive text and glyph names safely without evaluating markup or losing text',()=>{
  const v=fixture(),text=' \t<svg onload="alert(1)"><script>&x;</script></svg>\n';
  attach(v,[[{sign:'other-articulation',text,smufl:'articAccentAbove'}]],0,0,{print_object:false});
  const before=JSON.stringify(v),result=project(v),item=result.authoredArticulations!.events[0].notations[0].groups[0].items[0];
  expect(item).toMatchObject({sign:'other-articulation',text,smufl:'articAccentAbove',hidden:true,renderIds:[]});
  expect(result.mei).not.toContain('script');expect(result.mei).not.toContain('articAccentAbove');expect(result.mei).not.toContain('<artic');expect(JSON.stringify(v)).toBe(before);
 });
 it('keeps the complete all-seventeen Rust snapshot unchanged and refuses its first unsupported object',()=>{
  const v=family as unknown as View,before=JSON.stringify(v),decoded=decodeAuthoredArticulations(v)!;
  expect(decoded.events.flatMap(e=>e.notations.flatMap(w=>w.groups.flatMap(g=>g.items)))).toHaveLength(17);
  expect(()=>project(v)).toThrow(/Authored articulation #host.*notations\[0\].*group\[1\].*item\[6\].*spiccato.*explicit above\/below/);
  expect(JSON.stringify(v)).toBe(before);
 });
 it('honors hidden print wrappers even with known unrenderable signs while preserving all seventeen data objects',()=>{
  const v=structuredClone(family) as unknown as View;
  for(const frame of v.document.frames??[])for(const r of frame.records)if(r.record==='mark'&&r.kind==='scorestream.articulation.notations')r.attributes.print_object=false;
  const before=JSON.stringify(v),result=project(v),items=result.authoredArticulations!.events.flatMap(e=>e.notations.flatMap(w=>w.groups.flatMap(g=>g.items)));
  expect(items).toHaveLength(17);expect(items.every(i=>i.hidden&&i.renderIds.length===0)).toBe(true);expect(parse(result.mei).querySelectorAll('artic')).toHaveLength(0);expect(JSON.stringify(v)).toBe(before);
 });
 for(const sign of authoredSigns.filter(s=>authoredCapabilities[s]==='unsupported'))it(`refuses visible ${sign} at its exact object`,()=>{
  const v=marked(sign,'above');if(sign==='other-articulation')(v.document.frames!.at(-1)!.records[0] as MarkRecord).attributes.text='<script>passive</script>';
  const before=JSON.stringify(v);expect(()=>project(v)).toThrow(`notations[0]/group[0]/item[0] (${sign})`);expect(()=>project(v)).toThrow('Preview withheld');expect(JSON.stringify(v)).toBe(before);
 });
 for(const sign of ['stress','unstress','spiccato'])it(`refuses unresolved automatic ${sign} glyph orientation`,()=>{expect(()=>project(marked(sign))).toThrow('requires explicit above/below');});
 for(const [key,value] of Object.entries({default_x:'1/3',default_y:'0',relative_x:'-2',relative_y:'3',font_family:'Bravura',font_style:'normal',font_weight:'bold',font_size_points:'12',font_size_css:'small',color:'#AA112233'}))it(`refuses visible appearance ${key} without rounding/substitution`,()=>{
  const v=fixture();attach(v,[[{sign:'accent',[key]:value}]]);expect(()=>project(v)).toThrow(`unsupported appearance/coordinate field ${key}`);
 });
 it('does not let hidden wrappers bypass unknown fields, types, signs or unsafe glyph/text syntax',()=>{
  for(const item of ([{sign:'accent',orientation:'down'},{sign:'future'},{sign:'accent',placement:'sideways'},{sign:'accent',font_style:123},{sign:'other-articulation',text:'\u0000'},{sign:'other-articulation',text:'ok',smufl:'x"/><script>'}] as Attributes[])){
   const v=fixture();attach(v,[[item]],0,0,{print_object:false});expect(()=>project(v)).toThrow('Preview withheld');
  }
 });
 it('rejects duplicate/gapped/missing hierarchy, changed anchors/onsets and mixed families',()=>{
  const corruptions:((v:View)=>void)[]=[
   v=>v.document.frames!.push(structuredClone(v.document.frames![3])),
   v=>(v.document.frames![3].records[0] as MarkRecord).attributes.item_index=2,
   v=>v.document.frames!.splice(2,1),v=>v.document.frames![3].time='1',
   v=>(v.document.frames![3].records[0] as MarkRecord).target='#other',
   v=>v.document.frames!.push({time:'0',records:[{record:'mark',kind:'articulation',target:'#a',attributes:{value:'staccato'}}]}),
   v=>v.document.frames!.push({time:'0',records:[{record:'relation',kind:'tie',id:'t',anchor:'#a'}]}),
   v=>(v.document.frames![1].records[0] as MarkRecord).attributes.source_id='a',
   v=>(v.document.frames![1].records[0] as MarkRecord).attributes.print_object='no',
   v=>v.events.push({...v.events[0]}),v=>v.events[0].pitch='r',v=>v.events[0].duration='?',
  ];
  for(const corrupt of corruptions){const v=marked('accent');corrupt(v);expect(()=>project(v)).toThrow('Preview withheld');}
 });
 it('rejects unsupported capability versions, missing requirements, header-only and future records',()=>{
  for(const ext of [[],['scorestream.articulation@2'],['scorestream.articulation@1','scorestream.articulation@2']]){const v=marked('accent');v.document.required_extensions=ext;expect(()=>project(v)).toThrow('Preview withheld');}
  for(const malformed of ['scorestream.articulation@1',null,{},[1],['scorestream.articulation@1',null]]){const v=fixture();v.document.required_extensions=malformed;expect(()=>project(v)).toThrow('required_extensions must be an array of strings');}
  expect(()=>project(fixture())).toThrow('requires event records');const v=marked('accent');record(v,'future',{event_index:0});expect(()=>project(v)).toThrow('unknown reserved kind');
 });
 it('refuses excessive hierarchy and validates hidden hosts against actual rendered staff/measure coverage',()=>{
  const v=marked('accent');while(v.document.frames!.length<=10000)v.document.frames!.push(v.document.frames![0]);expect(()=>project(v)).toThrow('limited to 10000');
  const outside=fixture();attach(outside,[[{sign:'accent'}]],0,0,{print_object:false});outside.events[0].time='8';for(const f of outside.document.frames!)f.time='8';expect(()=>project(outside)).toThrow('host outside engraved');
 });
});

describe('real pinned Verovio authored articulation SVG',()=>{
 let toolkit:VerovioToolkit;
 beforeAll(async()=>{toolkit=new VerovioToolkit(await createVerovioModule());expect((toolkit as VerovioToolkit&{getVersion():string}).getVersion()).toMatch(/^5\.6\.0/);toolkit.setOptions({pageWidth:2100,pageHeight:2600,scale:40,adjustPageHeight:true,header:'none',footer:'none',font:'Bravura'});},10000);
 const render=(v:View)=>{const result=project(v);expect(toolkit.loadData(result.mei)).toBeTruthy();const svg=parse(toolkit.renderToSVG(1));expect(svg.querySelector('parsererror')).toBeNull();return {result,svg};};
 for(const sign of authoredSigns.filter(s=>authoredCapabilities[s]!=='unsupported'))it(`renders exact ${sign} glyphs above and below with every component mapped`,()=>{
  for(const [side,placement] of ['above','below'].entries()){
   const v=marked(sign,placement),before=JSON.stringify(v),{result,svg}=render(v),item=result.authoredArticulations!.events[0].notations[0].groups[0].items[0];
   expect(svg.querySelectorAll('.artic')).toHaveLength(item.renderIds.length);
   expect(item.renderIds.flatMap(id=>glyphs(byId(svg,id)))).toEqual(expected[sign][side]);
   for(const id of item.renderIds){const node=byId(svg,id);expect(node.getAttribute('class')).toBe('artic');expect(result.map[id]).toBe('a');const y=position(node).y,noteY=position(byId(svg,'ss-e0').querySelector('.notehead')!).y;expect(placement==='above'?y<noteY:y>noteY).toBe(true);}
   for(const id of item.renderIds)for(const use of byId(svg,id).querySelectorAll('use'))expect(byId(svg,use.getAttribute('xlink:href')!.slice(1)).querySelector('path')).not.toBeNull();
   expect(result.warnings).toEqual([]);expect(JSON.stringify(v)).toBe(before);
  }
 });
 it('keeps strong-accent up/down/omitted orientation independent from both placements and stem directions',()=>{
  for(const orientation of [undefined,'up','down'])for(const placement of ['above','below'])for(const stem of ['up','down']){
   const v=fixture();v.events[0].attributes={stem};attach(v,[[{sign:'strong-accent',placement,...(orientation?{orientation}:{})}]]);
   const {result,svg}=render(v);expect(glyphs(byId(svg,'ss-aa-0-0-0-0'))).toEqual([orientation==='down'?'E4AD':'E4AC']);
   expect(result.authoredArticulations!.events[0].notations[0].groups[0].items[0]).toMatchObject({orientation});
  }
 });
 it('lets native signs and compound components follow real automatic upper/lower-voice placement',()=>{
  for(const sign of authoredSigns.filter(s=>authoredCapabilities[s]==='native')){
   const v=fixture();v.events[0].pitch='C5';v.voices.push({id:'lower',part:'p',staff:1});v.events.push({...v.events[0],id:'b',pitch:'E4',voice:'lower'});attach(v,[[{sign}]]);attach(v,[[{sign}]],1);
   const {result,svg}=render(v);
   for(const [e,event] of result.authoredArticulations!.events.entries()){
    const item=event.notations[0].groups[0].items[0];expect(item.placement).toBeUndefined();expect(item.renderIds.flatMap(id=>glyphs(byId(svg,id)))).toEqual(expected[sign][e]);
    for(const id of item.renderIds){const y=position(byId(svg,id)).y,noteY=position(byId(svg,`ss-e${e}`).querySelector('.notehead')!).y;expect(e===0?y<noteY:y>noteY).toBe(true);expect(result.map[id]).toBe(event.eventId);}
   }
  }
 });
 it('retains repeated marks and authored mixed stacking order across repeated and empty groups/wrappers',()=>{
  for(const placement of ['above','below'])for(const order of [['staccato','accent','tenuto'],['tenuto','accent','staccato'],['staccato','staccato','staccato']]){
   const v=fixture();attach(v,[[],order.slice(0,2).map(sign=>({sign,placement}))],0,0,{source_id:'first'});attach(v,[[],[{sign:order[2],placement}]],0,1,{source_id:'second'});
   const {result,svg}=render(v),items=result.authoredArticulations!.events[0].notations.flatMap(w=>w.groups.flatMap(g=>g.items));
   const positions=items.map(item=>position(byId(svg,item.renderIds[0])).y);
   expect(positions[0]!==positions[1]&&positions[1]!==positions[2]).toBe(true);
   expect(positions.every((y,i)=>i===0||(placement==='above'?y<positions[i-1]:y>positions[i-1]))).toBe(true);
   expect(svg.querySelectorAll('.artic')).toHaveLength(3);expect(items.map(item=>glyphs(byId(svg,item.renderIds[0])))).toEqual(order.map(s=>expected[s][placement==='above'?0:1]));
  }
 });
 it('engraves the complete supported ordinary family together without dropping or substituting any leaf',()=>{
  for(const placement of ['above','below']){
   const v=fixture(),signs=authoredSigns.filter(s=>authoredCapabilities[s]!=='unsupported');
   attach(v,[signs.map(sign=>({sign,placement}))]);
   const {result,svg}=render(v),items=result.authoredArticulations!.events[0].notations[0].groups[0].items;
   expect(items).toHaveLength(10);expect(svg.querySelectorAll('.artic')).toHaveLength(11);
   for(const item of items){expect(item.renderIds.flatMap(id=>glyphs(byId(svg,id)))).toEqual(expected[item.sign][placement==='above'?0:1]);for(const id of item.renderIds)expect(result.map[id]).toBe('a');}
   expect(new Set(items.flatMap(i=>i.renderIds))).toHaveProperty('size',11);
   if(process.env.AUTHORED_ARTICULATION_EVIDENCE_DIR)writeFileSync(`${process.env.AUTHORED_ARTICULATION_EVIDENCE_DIR}/ordinary-family-${placement}.svg`,new XMLSerializer().serializeToString(svg));
  }
 });
 it('keeps each repeated detached-legato dot nearer than its line without merging compound identities',()=>{
  for(const placement of ['above','below'])for(const stem of ['up','down']){
   const v=fixture();v.events[0].attributes={stem};attach(v,[[{sign:'detached-legato',placement},{sign:'accent',placement},{sign:'detached-legato',placement}]]);
   const {result,svg}=render(v),items=result.authoredArticulations!.events[0].notations[0].groups[0].items;
   expect(svg.querySelectorAll('.artic')).toHaveLength(5);
   for(const item of items.filter(i=>i.sign==='detached-legato')){
    expect(item.renderIds).toEqual([`${item.id}-stacc`,`${item.id}-ten`]);
    const [dot,line]=item.renderIds.map(id=>position(byId(svg,id)).y);
    expect(placement==='above'?dot>line:dot<line).toBe(true);
    for(const id of item.renderIds)expect(result.map[id]).toBe('a');
   }
   expect(new Set(items.flatMap(i=>i.renderIds))).toHaveProperty('size',5);
  }
 });
 it('renders independent chord members, simultaneous voices and lower staff without spreading marks to neighbors',()=>{
  const v=fixture();v.parts[0].staves=2;v.voices.push({id:'other',part:'p',staff:1},{id:'bass',part:'p',staff:2});v.states.push({time:'0',scope:'p.s2',values:{meter:'4/4',clef:'F4',key:0}});
  v.events[0].chord='chord';v.events.push({...v.events[0],id:'member',pitch:'E4'},{...v.events[0],id:'voice',voice:'other',pitch:'G3',chord:null},{...v.events[0],id:'bass',voice:'bass',pitch:'C3',chord:null,staff:2},{...v.events[0],id:'unmarked',time:'1',chord:null});
  attach(v,[[{sign:'strong-accent',orientation:'down',placement:'above'}]]);attach(v,[[{sign:'detached-legato',placement:'below'}]],1);attach(v,[[{sign:'stress',placement:'below'}]],2);attach(v,[[{sign:'spiccato',placement:'above'}]],3);
  const before=JSON.stringify(v),{result,svg}=render(v),mei=parse(result.mei);
  expect(mei.querySelector('chord')!.querySelectorAll('artic')).toHaveLength(3);expect(svg.querySelectorAll('.artic')).toHaveLength(5);
  const byEvent=new Map(result.authoredArticulations!.events.map(e=>[e.eventId,e.notations[0].groups[0].items[0]]));
  for(const [eventId,item] of byEvent){
   const noteId=Object.keys(result.map).find(id=>id.startsWith('ss-e')&&result.map[id]===eventId)!;
   const note=byId(svg,noteId),noteY=position(note.querySelector('.notehead')!).y;
   expect(item.renderIds.flatMap(id=>glyphs(byId(svg,id)))).toEqual(eventId==='a'?['E4AD']:expected[item.sign][item.placement==='above'?0:1]);
   for(const id of item.renderIds){expect(result.map[id]).toBe(eventId);expect(byId(svg,id).closest('.note')!.id).toBe(noteId);const y=position(byId(svg,id)).y;expect(item.placement==='above'?y<noteY:y>noteY).toBe(true);}
  }
  const unmarked=Object.keys(result.map).find(id=>id.startsWith('ss-e')&&result.map[id]==='unmarked')!;expect(byId(svg,unmarked).querySelectorAll('.artic')).toHaveLength(0);expect(JSON.stringify(v)).toBe(before);expect(result.warnings).toEqual([]);
  if(process.env.AUTHORED_ARTICULATION_EVIDENCE_DIR){writeFileSync(`${process.env.AUTHORED_ARTICULATION_EVIDENCE_DIR}/polyphonic-authored-articulations.svg`,new XMLSerializer().serializeToString(svg));writeFileSync(`${process.env.AUTHORED_ARTICULATION_EVIDENCE_DIR}/polyphonic-authored-articulations.json`,JSON.stringify({view:v,projection:result},null,2));}
 });
 it('renders hidden supported symbols as zero visible marks and consumes their host without phantom map IDs',()=>{
  const v=fixture();attach(v,[[{sign:'accent',placement:'above'}]],0,0,{print_object:false});const {result,svg}=render(v);expect(svg.querySelectorAll('.artic')).toHaveLength(0);expect(Object.keys(result.map)).toEqual(['ss-e0']);
 });
});
