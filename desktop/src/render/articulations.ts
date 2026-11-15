import type {View,MarkRecord,ScoreEvent} from '../types';
import {Q} from './exact';
// MEI 5: note-local <artic> values, verified with the pinned Verovio renderer.
// https://music-encoding.org/guidelines/v5/elements/artic.html
const glyphs = {staccato:'stacc',staccatissimo:'stacciss',tenuto:'ten',accent:'acc'} as const;
export type SimpleArticulation = keyof typeof glyphs;
export function projectArticulations(view:View){
 const events = new Map<string,ScoreEvent|null>();
 for(const event of view.events)events.set(event.id,events.has(event.id)?null:event);
 const entries=(view.document.frames??[]).flatMap(f=>f.records
  .filter((r):r is MarkRecord=>r.record==='mark'&&r.kind==='articulation').map(mark=>({time:f.time,mark})));
 const byEvent=new Map<string,{id:string;xml:string;value:SimpleArticulation;handled:boolean}[]>();
 for(const [index,{time,mark}] of entries.entries()){
  const fail:(reason:string)=>never=reason=>{throw Error(`Articulation ${mark.target}: ${reason}. Preview withheld; your source is preserved.`);};
  const a=mark.attributes;
  if(Object.keys(a).some(k=>!['value','placement','id'].includes(k)))fail('unsupported attributes');
  if(a.id!==undefined&&(typeof a.id!=='string'||!a.id))fail('unsupported identity');
  if(typeof a.value!=='string'||!Object.hasOwn(glyphs,a.value))fail('unsupported articulation shape');
  if(a.placement!==undefined&&a.placement!=='above'&&a.placement!=='below')fail('unsupported placement');
  const event=mark.target.startsWith('#')?events.get(mark.target.slice(1)):undefined;
  if(!event)fail('requires one mapped event target');
  if(!new Q(time).eq(new Q(event.time)))fail('attachment must be in its event onset frame');
  if(!/^[A-G](#{1,2}|b{1,2})?-?\d+$/.test(event.pitch)||event.duration==='?')fail('requires a measured pitched note');
  const value=a.value as SimpleArticulation,existing=byEvent.get(event.id)??[];
  if(existing.some(e=>e.value===value))fail('duplicate identical signs');
  const id=`ss-artic${index}`,place=a.placement?` place="${a.placement}"`:'';
  existing.push({id,xml:`<artic xml:id="${id}" artic="${glyphs[value]}"${place}/>`,value,handled:false});
  byEvent.set(event.id,existing);
 }
 return {
  forEvent(eventId:string,map:Record<string,string>){
   return (byEvent.get(eventId)??[]).map(e=>{
    if(e.handled)throw Error('An articulation target was engraved twice. Preview withheld.');
    e.handled=true;map[e.id]=eventId;return e.xml;
   }).join('');
  },
  finish(){if([...byEvent.values()].flat().some(e=>!e.handled))throw Error('An articulation target is outside the engraved measures or staff mapping. Preview withheld.');}
 };
}
