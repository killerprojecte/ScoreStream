import type {View,ScoreEvent} from '../types';
import {Q,rhythm} from './exact';
import {projectMarks} from './marks';
import {projectArticulations} from './articulations';
import {projectAuthoredArticulations,type AuthoredProjection} from './authored-articulations';
import {readMetadata} from '../app/metadata';
const esc=(s:unknown)=>String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&apos;'}[c]!));
// Preserve authored label whitespace through XML text normalization.
const labelEsc=(s:string)=>esc(s).replace(/[\t\n\r]/g,c=>`&#${c.charCodeAt(0)};`);
export const RENDERER='Verovio 5.6.0 · Bravura · projection 0.2';
export function project(view:View):{mei:string;map:Record<string,string>;warnings:string[];authoredArticulations:AuthoredProjection|null}{
 if(!view.valid)throw Error('Only Rust-validated snapshots may be projected.');
 if(!view.events.length)throw Error('No measured events to engrave yet.');
 const warnings:string[]=[];const map:Record<string,string>={};
 const authoredArticulations=projectAuthoredArticulations(view);
 const marks=projectMarks(view,warnings);
 const articulations=projectArticulations(view);
 if(view.states.some(s=>s.time!=='0'))throw Error('Mid-score state changes are not yet supported by this preview. Your source is preserved.');
 const staffDefs=view.parts.flatMap(p=>Array.from({length:p.staves},(_,i)=>({part:p.id,staff:i+1,instrument:p.instrument})));
 const firstPart=view.parts[0]?.id;
 const measures=view.measures.filter(m=>m.part===firstPart).sort((a,b)=>new Q(a.time).cmp(new Q(b.time)));
 if(!measures.length)throw Error('Add explicit measure records to render this score.');
 for(const part of view.parts){const own=view.measures.filter(m=>m.part===part.id);if(own.length!==measures.length||own.some((m,i)=>!new Q(m.time).eq(new Q(measures[i].time))||!new Q(m.length).eq(new Q(measures[i].length))))throw Error('This preview requires aligned measure boundaries across parts.');}
 const used=new Set<string>();let next=0;
 const stemXml=(e:ScoreEvent)=>{
   const stem=e.attributes?.stem;if(stem===undefined)return '';
   if(e.pitch==='r'||e.pitch==='_')throw Error('Rest stemlets and space stems are preserved but not yet supported by the preview.');
   if(stem!=='up'&&stem!=='down')throw Error('This stem kind is preserved but not yet supported by the preview.');
   if(rhythm(new Q(e.duration)).dur===1)throw Error('Stem directions on stemless durations are preserved but not yet supported by the preview.');
   return ` stem.dir="${stem}"`;
 };
 const eventXml=(e:ScoreEvent,withRhythm=true)=>{
   if(!e.id)throw Error('Preview requires stable event IDs.');
   if(e.duration==='?')throw Error('Grace-note layout is not yet supported.');
   if(e.attributes?.tm)throw Error('Tuplet layout is not yet supported.');
   const noteSize=e.attributes?.note_size;
   if(noteSize!==undefined&&noteSize!=='full'&&noteSize!=='cue')throw Error('This graphic note size is preserved but not yet supported by the preview.');
   if(noteSize!==undefined&&e.pitch==='_')throw Error('Graphic note size on a space cannot be engraved faithfully.');
   const size=noteSize==='cue'?' cue="true"':'';
   const stem=stemXml(e);
   const measureRest=e.attributes?.measure_rest;
   if(measureRest!==undefined&&typeof measureRest!=='boolean')throw Error('Full-measure-rest indication must be a boolean in a validated snapshot.');
   if(measureRest===true){
     if(e.pitch!=='r'||e.chord)throw Error('Full-measure rests cannot be pitched, spaces, or chord members.');
     if(e.written_duration!==null||e.dots!==0||noteSize!==undefined)throw Error('Explicit written spelling or graphic size on a full-measure rest is preserved but not yet supported by the preview.');
     const id=`ss-e${next++}`;map[id]=e.id;used.add(e.id);
     return `<mRest xml:id="${id}"/>`;
   }
   const r=rhythm(new Q(e.duration));
   if(e.written_duration && (Number(e.written_duration)!==r.dur||e.dots!==r.dots))throw Error('Written and actual rhythm differ; this preview cannot engrave that rhythm faithfully yet.');
   const id=`ss-e${next++}`;map[id]=e.id;used.add(e.id);
   const dur=withRhythm?` dur="${r.dur}" dots="${r.dots}"`:'';
   if(e.pitch==='r'||e.pitch==='_')return `<${e.pitch==='r'?'rest':'space'} xml:id="${id}"${dur}${size}/>`;
   const p=/^([A-G])(#{1,2}|b{1,2})?(-?\d+)$/.exec(e.pitch);
   if(!p)throw Error(`Pitch ${e.pitch} is preserved but not supported by this preview.`);
   const acc:Record<string,string>={'#':'s','##':'ss',b:'f',bb:'ff'};
   const accidental=marks.forEvent(e.id,map);
   const children=accidental+articulations.forEvent(e.id,map)+authoredArticulations.forEvent(e.id,map);
   return `<note xml:id="${id}" pname="${p[1].toLowerCase()}" oct="${p[3]}"${!accidental&&p[2]?` accid="${acc[p[2]]}"`:''}${dur}${size}${withRhythm?stem:''}${children?`>${children}</note>`:'/>'}`;
 };
 const nominalLengths:Q[]=[];
 const staffDefXml=staffDefs.map((s,i)=>{
   const attrs=Object.assign({},...view.states.filter(x=>x.time==='0'&&x.scope===`${s.part}.s${s.staff}`).map(x=>x.values));
   for(const field of ['clef','meter','key'])if(attrs[field]===undefined)warnings.push(`${s.part}.s${s.staff}: ${field} is unset; preview uses ${field==='clef'?(s.staff===2?'F4':'G2'):field==='meter'?'4/4':'C major'} as a display default.`);
   const clef=/^([GFC])(\d)$/.exec(String(attrs.clef??(s.staff===2?'F4':'G2')));
   if(!clef)throw Error('Unsupported clef in preview.');
   const meter=/^(\d+)\/(\d+)$/.exec(String(attrs.meter??'4/4'));
   if(!meter)throw Error('Unsupported meter in preview.');
   nominalLengths.push(new Q(4n*BigInt(meter[1]),BigInt(meter[2])));
   const key=Number(attrs.key??0);if(!Number.isInteger(key)||Math.abs(key)>7)throw Error('Unsupported key signature in preview.');
   return `<staffDef n="${i+1}" lines="5" clef.shape="${clef[1]}" clef.line="${clef[2]}" meter.count="${meter[1]}" meter.unit="${meter[2]}" key.sig="${Math.abs(key)}${key<0?'f':key>0?'s':''}"/>`;
 });
 let staffOffset=0;
 const groupedStaffDefs=view.parts.map(p=>{const defs=staffDefXml.slice(staffOffset,staffOffset+p.staves).join('');staffOffset+=p.staves;return `<staffGrp${p.staves>1?' symbol="brace" bar.thru="true"':''}><label>${labelEsc(p.display_name??p.instrument)}</label>${defs}</staffGrp>`;}).join('');
 const measureXml=measures.map((m,mi)=>{
   const start=new Q(m.time),end=start.add(new Q(m.length));
   const irregular=nominalLengths.some(n=>!n.eq(new Q(m.length)));
   if(irregular)warnings.push(`Measure ${m.label??mi+1}: actual length ${m.length} quarters differs from the written meter; engraving preserves both.`);
   return `<measure n="${esc(m.label??mi+1)}"${irregular?' metcon="false"':''}>${staffDefs.map((s,si)=>{
     const voices=view.voices.filter(v=>v.part===s.part);
     const relevant=view.events.filter(e=>e.staff===s.staff&&voices.some(v=>v.id===e.voice)&&new Q(e.time).cmp(start)>=0&&new Q(e.time).cmp(end)<0);
     const active=voices.filter(v=>v.staff===s.staff||relevant.some(e=>e.voice===v.id));
     if(!active.length)return `<staff n="${si+1}"><layer n="1"><mSpace/></layer></staff>`;
     return `<staff n="${si+1}">${active.map((v,vi)=>{
       let cursor=start;const events=relevant.filter(e=>e.voice===v.id).sort((a,b)=>new Q(a.time).cmp(new Q(b.time)));let content='';
       for(let j=0;j<events.length;j++){
         const e=events[j],time=new Q(e.time);
         if(e.duration==='?')throw Error('Grace-note layout is not yet supported.');
         if(e.attributes?.measure_rest===true&&(!time.eq(start)||!new Q(e.duration).eq(new Q(m.length))))throw Error('A full-measure rest must start at the exact measure boundary and span its actual length.');
         // Pinned Verovio infers mRest duration from meter; irregular rest-only measures can collapse to zero.
         if(e.attributes?.measure_rest===true&&irregular)throw Error('Full-measure rests in metrically irregular measures are preserved but not yet supported by the preview.');
         const finish=time.add(new Q(e.duration));if(finish.cmp(end)>0)throw Error('Cross-bar events require a tie projection, which is not yet supported.');
         if(time.cmp(cursor)<0)throw Error('Overlapping ungrouped events cannot be engraved by this preview.');
         if(time.cmp(cursor)>0){const r=rhythm(time.sub(cursor));content+=`<space dur="${r.dur}" dots="${r.dots}"/>`;}
         const chord=[e];if(e.chord)while(j+1<events.length&&events[j+1].chord===e.chord&&events[j+1].time===e.time)chord.push(events[++j]);
         if(chord.some(c=>!new Q(c.duration).eq(new Q(e.duration))))throw Error('Unequal-duration chord members are not yet supported by the preview.');
         if(chord.some(c=>(c.attributes?.note_size??'full')!==(e.attributes?.note_size??'full')))throw Error('Mixed graphic sizes within a chord are preserved but not yet supported by the preview.');
         if(chord.some(c=>c.attributes?.stem!==e.attributes?.stem))throw Error('Mixed stem indications within a chord are preserved but not yet supported by the preview.');
         if(chord.length>1){const r=rhythm(new Q(e.duration));content+=`<chord dur="${r.dur}" dots="${r.dots}"${e.attributes?.note_size==='cue'?' cue="true"':''}${stemXml(e)}>${chord.map(c=>eventXml(c,false)).join('')}</chord>`;}else content+=eventXml(e);
         cursor=finish;
       }
       if(!events.length)content='<mSpace/>';
       else if(cursor.cmp(end)<0){const r=rhythm(end.sub(cursor));content+=`<space dur="${r.dur}" dots="${r.dots}"/>`;}
       return `<layer n="${vi+1}">${content}</layer>`;
     }).join('')}</staff>`;
   }).join('')}${marks.forMeasure(m,map)}</measure>`;
 }).join('');
 marks.finish();
 articulations.finish();
 authoredArticulations.finish();
 if(used.size!==view.events.length)throw Error('Some events are outside the declared measures or staff mapping. Preview withheld to avoid hiding music.');
 const metadata=readMetadata(view.document.metadata);
 const catalogTitle=metadata.title?`<title>${esc(metadata.title)}</title>`:'<title/>';
 return {mei:`<?xml version="1.0" encoding="UTF-8"?><mei xmlns="http://www.music-encoding.org/ns/mei" meiversion="5.0"><meiHead><fileDesc><titleStmt>${catalogTitle}</titleStmt><pubStmt/></fileDesc></meiHead><music><body><mdiv><score><scoreDef><staffGrp>${groupedStaffDefs}</staffGrp></scoreDef><section>${measureXml}</section></score></mdiv></body></music></mei>`,map,warnings,authoredArticulations:authoredArticulations.authored};
}
