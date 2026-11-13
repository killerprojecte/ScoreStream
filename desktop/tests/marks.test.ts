import {describe,it,expect} from 'vitest';
import createVerovioModule from 'verovio/wasm';
import {VerovioToolkit} from 'verovio/esm';
import {project} from '../src/render/project';
import type {View,MarkRecord} from '../src/types';
const mark=(kind:string,target:string,attributes:MarkRecord['attributes']):MarkRecord=>({record:'mark',kind,target,attributes});
const fixture=(marks:MarkRecord[]=[],time='0'):View=>({valid:true,diagnostics:[],source:'Never parse this SFS source',document:{frames:[{time,records:marks}]},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false,parts:[{id:'p',instrument:'Piano',staves:2}],voices:[{id:'rh',part:'p',staff:1},{id:'lh',part:'p',staff:2}],states:[{time:'0',scope:'score',values:{tempo:'q:104'}},{time:'0',scope:'p.s1',values:{meter:'4/4',clef:'G2',key:0}},{time:'0',scope:'p.s2',values:{meter:'4/4',clef:'F4',key:0}}],measures:[{id:'m',part:'p',time:'0',length:'4',label:'1'}],events:[{id:'n1',voice:'rh',pitch:'C4',time:'0',duration:'4',staff:1,chord:null,written_duration:null,dots:0,source_line:5},{id:'b1',voice:'lh',pitch:'C3',time:'0',duration:'4',staff:2,chord:null,written_duration:null,dots:0,source_line:6}]});
describe('typed expressive marks',()=>{
 it('does not invent a printed metronome from scalar performance tempo',()=>{expect(project(fixture()).mei).not.toContain('<tempo');});
 it('engraves independent printed tempo, exact fractional rate, escaped text and dot',()=>{const r=project(fixture([mark('metronome','score',{beat:'q.',bpm:'209/2',text:'Calmo & dolce'})]));expect(r.mei).toContain('Calmo &amp; dolce');expect(r.mei).toContain('\uECA5\uECB7');expect(r.mei).toContain(' = 209/2</tempo>');expect(r.mei).not.toContain('midi.bpm');expect(r.mei.match(/<tempo/g)).toHaveLength(1);expect(r.warnings).toEqual([]);});
 it('maps event dynamics to actual staff including cross-staff notation',()=>{const v=fixture([mark('dynamic','#n1',{value:'sfz',placement:'above'})]);v.events[0].staff=2;const r=project(v);expect(r.mei).toMatch(/<dynam xml:id="ss-mark0" staff="2" startid="#ss-e\d+" place="above">sfz<\/dynam>/);expect(r.map['ss-mark0']).toBe('n1');});
 it('maps scope marks to correct staff and exact metrical timestamp',()=>{const v=fixture([mark('dynamic','p.s2',{value:'pp'})],'3/2');v.states[2].values.meter='6/8';expect(project(v).mei).toContain('staff="2" tstamp="4" place="below">pp');});
 it('uses exact staff state without order-dependent score or part inheritance',()=>{
  const v=fixture([mark('dynamic','p.s2',{value:'pp'})],'3/2');
  v.states[2].values.meter='6/8';
  // Defensive synthetic fixture: current Rust validation already forbids these
  // out-of-scope fields. They must never acquire inheritance semantics in JS.
  v.states.push({time:'0',scope:'score',values:{meter:'2/2',clef:'C3',key:3}},{time:'0',scope:'p',values:{meter:'3/4'}});
  const result=project(v);
  expect(result.mei).toContain('staff="2" tstamp="4" place="below">pp');
  expect(result.mei).toContain('clef.shape="F" clef.line="4" meter.count="6" meter.unit="8" key.sig="0"');
  v.states.reverse();expect(project(v).mei).toBe(result.mei);
  v.states=v.states.filter(s=>s.scope!=='p.s2');
  const missing=project(v);expect(missing.warnings.join()).toContain('p.s2: meter is unset');
  expect(missing.mei).toContain('staff="2" tstamp="2.5" place="below">pp');
 });
 it('does not round unsupported scope timestamps or drop unsupported authored marks silently',()=>{const v=fixture([mark('dynamic','p',{value:'f'}),mark('words','score',{text:'cresc.'})],'1/3');const r=project(v);expect(r.warnings).toHaveLength(2);expect(r.warnings.join(' ')).toContain('non-terminating');expect(r.warnings.join(' ')).toContain('words');expect(v.document.frames![0].records).toHaveLength(2);});
 it('warns for marks outside measures and unknown targets',()=>{expect(project(fixture([mark('dynamic','p',{value:'f'})],'4')).warnings.join()).toContain('outside declared');expect(project(fixture([mark('dynamic','#missing',{value:'f'})])).warnings.join()).toContain('unmapped');});
 it('warns instead of discarding unknown mark attributes',()=>{const r=project(fixture([mark('dynamic','p',{value:'mf','x-custom':'authored'})]));expect(r.warnings.join()).toContain('unsupported mark attributes');expect(r.mei).not.toContain('<dynam');});
 it('groups grand staff once with brace and through barlines',()=>{const mei=project(fixture()).mei;expect(mei).toContain('staffGrp label="Piano" symbol="brace" bar.thru="true"');expect(mei.match(/label="Piano"/g)).toHaveLength(1);});
 it('renders genuine Bravura metronome font and dynamics paths in installed Verovio SVG',async()=>{
  const module=await createVerovioModule();const toolkit=new VerovioToolkit(module);
  toolkit.setOptions({font:'Bravura',pageWidth:1800,pageHeight:1500,scale:40,adjustPageHeight:true,header:'none',footer:'none',svgViewBox:true});
  const r=project(fixture([mark('metronome','score',{beat:'q.',bpm:'209/2',text:'Moderato'}),mark('dynamic','#n1',{value:'mp',placement:'below'}),mark('dynamic','p.s2',{value:'sfz',placement:'above'})]));
  expect(toolkit.loadData(r.mei)).toBeTruthy();const svg=toolkit.renderToSVG(1);
  expect(svg).toContain('class="tempo"');expect(svg).toContain('class="dynam"');expect(svg).toContain('Moderato');expect(svg).toContain('209/2');expect(svg).toContain('\uECA5\uECB7');expect(svg).toContain('font-family="Bravura"');expect(svg).toContain("format('woff2')");expect(svg).toMatch(/(?:href|xlink:href)="#E52C/);expect(svg).toMatch(/(?:href|xlink:href)="#E539/);
 },20000);
});
