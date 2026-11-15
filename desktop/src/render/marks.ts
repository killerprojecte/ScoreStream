import type {View,MarkRecord,Measure,ScoreEvent} from '../types';
import {Q} from './exact';
const esc=(s:unknown)=>String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&apos;'}[c]!));
// SMuFL metronome note glyphs, not Unicode text-font approximations.
// https://smufl.formats.music/latest/tables/metronome-marks.html
// https://book.verovio.org/toolkit-reference/output-formats.html
// https://music-encoding.org/guidelines/v5/elements/tempo.html
// https://music-encoding.org/guidelines/v5/elements/dynam.html
const beats:Record<string,string>={w:'\uECA2',h:'\uECA3',q:'\uECA5',e:'\uECA7',s:'\uECA9'};
const dynamics=new Set(['pppp','ppp','pp','p','mp','mf','f','ff','fff','ffff','sf','sfz','fp']);
// MEI tstamp is a decimal, not a rational. Reject non-terminating decimals rather
// than silently round an authored placement. Event targets use exact startid.
function decimal(q:Q):string {
 let n=q.n,d=q.d;const gcd=(a:bigint,b:bigint):bigint=>b?gcd(b,a%b):a;const g=gcd(n<0n?-n:n,d);n/=g;d/=g;
 let test=d;while(test%2n===0n)test/=2n;while(test%5n===0n)test/=5n;
 if(test!==1n)throw Error('non-terminating timestamp requires an event target');
 let result=String(n/d),r=n%d;if(r){result+='.';while(r){r*=10n;result+=String(r/d);r%=d;}}return result;
}
export function projectMarks(view:View,warnings:string[]){
 if(Array.isArray(view.document.required_extensions)&&view.document.required_extensions.includes('scorestream.slur@1'))throw Error('Authored slur endpoints, orientation and Bézier geometry are preserved but not yet supported by the preview. Preview withheld; source is unchanged.');
 if((Array.isArray(view.document.required_extensions)&&view.document.required_extensions.includes('scorestream.beam@1'))||(view.document.frames??[]).some(f=>f.records.some(r=>r.record==='mark'&&r.kind==='scorestream.beam')))throw Error('Authored beam roles and hooks are preserved but not yet supported by the preview. Preview withheld to avoid replacing them with automatic grouping.');
 const staves=view.parts.flatMap(p=>Array.from({length:p.staves},(_,i)=>({part:p.id,staff:i+1})));
 const entries=(view.document.frames??[]).flatMap(f=>f.records.filter((r):r is MarkRecord=>r.record==='mark').map(mark=>({time:f.time,mark,handled:false})));
 if((view.document.frames??[]).some(f=>f.records.some(r=>r.record==='relation')))warnings.push('Relationships are preserved in the source but not drawn in this preview.');
 const warn=(mark:MarkRecord,reason:string)=>warnings.push(`${mark.kind} mark ${mark.attributes.id??mark.target}: ${reason}; preserved in the source but not drawn.`);
 // An authored accidental is part of its note, not an optional control mark.
 // Fail closed before engraving: a pitch-derived accidental is no substitute.
 const accidentals=new Map<string,{entry:typeof entries[number];id:string;xml:string}>();
 const eventsById=new Map<string,ScoreEvent|null>();
 for(const event of view.events)eventsById.set(event.id,eventsById.has(event.id)?null:event);
 const glyphs=new Map([['natural',{mei:'n',pitch:''}],['sharp',{mei:'s',pitch:'#'}],['flat',{mei:'f',pitch:'b'}]]);
 const accidentalFields=new Set(['value','cautionary','parentheses','id']);
 for(const [index,entry] of entries.entries()){
  const {mark}=entry;if(mark.kind!=='accidental')continue;
  const a=mark.attributes;
  const fail:(reason:string)=>never=reason=>{throw Error(`Accidental mark ${mark.target}: ${reason}. Preview withheld; your source is preserved.`);};
  if(Object.keys(a).some(key=>!accidentalFields.has(key)))fail('unsupported accidental attributes');
  if(a.id!==undefined&&(typeof a.id!=='string'||!a.id))fail('unsupported accidental identity');
  for(const flag of ['cautionary','parentheses'])if(a[flag]!==undefined&&typeof a[flag]!=='boolean')fail(`${flag} must be a boolean`);
  const glyph=typeof a.value==='string'?glyphs.get(a.value):undefined;
  if(!glyph)fail('unsupported accidental value');
  const event=mark.target.startsWith('#')?eventsById.get(mark.target.slice(1)):undefined;
  if(!event)fail('requires one mapped event target');
  if(!new Q(entry.time).eq(new Q(event.time)))fail('attachment must be in its event onset frame');
  const pitch=/^[A-G](#|b)?-?\d+$/.exec(event.pitch);
  if(!pitch)fail('requires a supported pitched note');
  if((pitch[1]??'')!==glyph.pitch)fail('printed glyph conflicts with written pitch');
  if(accidentals.has(event.id))fail('duplicate accidental marks for one event');
  // MEI 5 keeps function and enclosure independent. Verovio 5.6 renders
  // func="caution" without adding parentheses. False/absent cautionary both
  // mean no caution function; absent/false parentheses stay distinct in MEI.
  // https://music-encoding.org/guidelines/v5/elements/accid.html
  const id=`ss-mark${index}`;
  const func=a.cautionary===true?' func="caution"':'';
  const enclosure=a.parentheses===undefined?'':` enclose="${a.parentheses?'paren':'none'}"`;
  accidentals.set(event.id,{entry,id,xml:`<accid xml:id="${id}" accid="${glyph.mei}"${func}${enclosure}/>`});
 }
 return {
  forEvent(eventId:string,map:Record<string,string>){
   const accidental=accidentals.get(eventId);if(!accidental)return '';
   if(accidental.entry.handled)throw Error('An accidental target was engraved more than once. Preview withheld.');
   accidental.entry.handled=true;map[accidental.id]=eventId;return accidental.xml;
  },
  forMeasure(measure:Measure,map:Record<string,string>){
   let xml='';const start=new Q(measure.time),end=start.add(new Q(measure.length));
   for(const [index,entry] of entries.entries()){
    if(entry.handled||entry.mark.kind==='accidental')continue;
    const {mark}=entry,a=mark.attributes;
    const event=mark.target.startsWith('#')?view.events.find(e=>e.id===mark.target.slice(1)):undefined;
    const time=new Q(event?.time??entry.time);
    if(time.cmp(start)<0||time.cmp(end)>=0)continue;
    entry.handled=true;
    try {
     if(mark.kind!=='metronome'&&mark.kind!=='dynamic')throw Error('unsupported notation');
     const allowed=new Set(mark.kind==='dynamic'?['value','placement','sound','id']:['beat','bpm','text','id']);
     if(Object.keys(a).some(key=>!allowed.has(key)))throw Error('unsupported mark attributes');
     const voice=event?view.voices.find(v=>v.id===event.voice):view.voices.find(v=>v.id===mark.target);
     const staffIndex=event?staves.findIndex(s=>s.part===voice?.part&&s.staff===event.staff):
      mark.target==='score'?0:staves.findIndex(s=>mark.target===`${s.part}.s${s.staff}`||mark.target===s.part||!!voice&&s.part===voice.part&&s.staff===voice.staff);
     if(staffIndex<0)throw Error('unmapped target');
     const staff=staves[staffIndex];let anchor='';
     if(event){const id=Object.keys(map).find(k=>map[k]===event.id);if(!id)throw Error('event target is not engraved');anchor=`startid="#${id}"`;}
     else {
      const state=Object.assign({},...view.states.filter(s=>s.time==='0'&&s.scope===`${staff.part}.s${staff.staff}`).map(s=>s.values));
      const meter=state.meter??'4/4';
      const unit=String(meter).split('/')[1];const offset=time.sub(start);anchor=`tstamp="${decimal(new Q(offset.n*BigInt(unit),offset.d*4n).add(new Q('1')))}"`;
     }
     const id=`ss-mark${index}`;const common=`xml:id="${id}" staff="${staffIndex+1}" ${anchor}`;
     if(mark.kind==='dynamic'){
      if(!dynamics.has(String(a.value)))throw Error('unsupported dynamic value');
      if(a.sound!==undefined&&typeof a.sound!=='boolean')throw Error('unsupported sound flag');
      if(a.placement!==undefined&&a.placement!=='above'&&a.placement!=='below')throw Error('unsupported placement');
      xml+=`<dynam ${common} place="${a.placement??'below'}">${esc(a.value)}</dynam>`;
     }else{
      const beat=String(a.beat),base=beat.replace(/\.$/,'');if(!beats[base]||!new RegExp('^[whqes]\\.?$').test(beat))throw Error('unsupported metronome beat');
      const rate=String(a.bpm);if(!/^\d+(\/\d+)?$/.test(rate)||new Q(rate).cmp(new Q('0'))<=0)throw Error('unsupported metronome rate');
      // No midi.bpm: a printed metronome is independent from performance state.
      // SMuFL text defaults to a full staff-sized glyph in Verovio (720 versus
      // 405 for tempo text). Reduce it at the MEI boundary, before layout, so
      // the beat and words share a readable scale and collision bounds. Keep
      // the music font regular rather than inheriting the tempo's bold weight.
      xml+=`<tempo ${common} place="above">${a.text?`${esc(a.text)} `:''}<rend glyph.auth="smufl" fontsize="60%" fontweight="normal">${beats[base]}${beat.endsWith('.')?'\uECB7':''}</rend> = ${esc(rate)}</tempo>`;
     }
     // Only event-anchored controls participate in existing event selection.
     if(event)map[id]=event.id;
    }catch(error){warn(mark,(error as Error).message);}
   }return xml;
  },
  finish(){for(const entry of entries)if(!entry.handled){
   if(entry.mark.kind==='accidental')throw Error('An accidental target is outside the engraved measures or staff mapping. Preview withheld; your source is preserved.');
   warn(entry.mark,'outside declared measures');
  }}
 };
}
