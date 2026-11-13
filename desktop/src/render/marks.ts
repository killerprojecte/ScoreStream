import type {View,MarkRecord,Measure} from '../types';
import {Q} from './exact';
const esc=(s:unknown)=>String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&apos;'}[c]!));
// SMuFL metronome note glyphs, not Unicode text-font approximations.
// https://smufl.formats.music/latest/tables/metronome-marks.html
// https://book.verovio.org/toolkit-reference/output-formats.html
// https://music-encoding.org/guidelines/v5/elements/tempo.html
// https://music-encoding.org/guidelines/v5/elements/dynam.html
const beats:Record<string,string>={w:'\uECA2',h:'\uECA3',q:'\uECA5',e:'\uECA7',s:'\uECA9'};
const dynamics=new Set(['pppp','ppp','pp','p','mp','mf','f','ff','fff','ffff','sfz','fp']);
// MEI tstamp is a decimal, not a rational. Reject non-terminating decimals rather
// than silently round an authored placement. Event targets use exact startid.
function decimal(q:Q):string {
 let n=q.n,d=q.d;const gcd=(a:bigint,b:bigint):bigint=>b?gcd(b,a%b):a;const g=gcd(n<0n?-n:n,d);n/=g;d/=g;
 let test=d;while(test%2n===0n)test/=2n;while(test%5n===0n)test/=5n;
 if(test!==1n)throw Error('non-terminating timestamp requires an event target');
 let result=String(n/d),r=n%d;if(r){result+='.';while(r){r*=10n;result+=String(r/d);r%=d;}}return result;
}
export function projectMarks(view:View,warnings:string[]){
 const staves=view.parts.flatMap(p=>Array.from({length:p.staves},(_,i)=>({part:p.id,staff:i+1})));
 const entries=(view.document.frames??[]).flatMap(f=>f.records.filter((r):r is MarkRecord=>r.record==='mark').map(mark=>({time:f.time,mark,handled:false})));
 if((view.document.frames??[]).some(f=>f.records.some(r=>r.record==='relation')))warnings.push('Relationships are preserved in the source but not drawn in this preview.');
 const warn=(mark:MarkRecord,reason:string)=>warnings.push(`${mark.kind} mark ${mark.attributes.id??mark.target}: ${reason}; preserved in the source but not drawn.`);
 return {
  forMeasure(measure:Measure,map:Record<string,string>){
   let xml='';const start=new Q(measure.time),end=start.add(new Q(measure.length));
   for(const [index,entry] of entries.entries()){
    if(entry.handled)continue;
    const {mark}=entry,a=mark.attributes;
    const event=mark.target.startsWith('#')?view.events.find(e=>e.id===mark.target.slice(1)):undefined;
    const time=new Q(event?.time??entry.time);
    if(time.cmp(start)<0||time.cmp(end)>=0)continue;
    entry.handled=true;
    try {
     if(mark.kind!=='metronome'&&mark.kind!=='dynamic')throw Error('unsupported notation');
     const allowed=new Set(mark.kind==='dynamic'?['value','placement','id']:['beat','bpm','text','id']);
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
      if(a.placement!==undefined&&a.placement!=='above'&&a.placement!=='below')throw Error('unsupported placement');
      xml+=`<dynam ${common} place="${a.placement??'below'}">${esc(a.value)}</dynam>`;
     }else{
      const beat=String(a.beat),base=beat.replace(/\.$/,'');if(!beats[base]||!new RegExp('^[whqes]\\.?$').test(beat))throw Error('unsupported metronome beat');
      const rate=String(a.bpm);if(!/^\d+(\/\d+)?$/.test(rate)||new Q(rate).cmp(new Q('0'))<=0)throw Error('unsupported metronome rate');
      // No midi.bpm: a printed metronome is independent from performance state.
      xml+=`<tempo ${common} place="above">${a.text?`${esc(a.text)} `:''}<rend glyph.auth="smufl">${beats[base]}${beat.endsWith('.')?'\uECB7':''}</rend> = ${esc(rate)}</tempo>`;
     }
     // Only event-anchored controls participate in existing event selection.
     if(event)map[id]=event.id;
    }catch(error){warn(mark,(error as Error).message);}
   }return xml;
  },
  finish(){for(const entry of entries)if(!entry.handled)warn(entry.mark,'outside declared measures');}
 };
}
