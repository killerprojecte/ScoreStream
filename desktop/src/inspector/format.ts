import type {ScoreEvent, View} from '../types';
import {Q} from '../render/exact';

export function performanceTempo(view:View):{rate:string;unit:string}|null {
  const value=view.states.find(s=>s.time==='0'&&s.scope==='score'&&s.values.tempo!==undefined)?.values.tempo;
  const match=/^([whqes])(\.)?:(\d+(?:\/\d+)?)$/.exec(String(value??''));
  if(!match)return null;
  const units:Record<string,string>={w:'Whole note',h:'Half note',q:'Quarter note',e:'Eighth note',s:'Sixteenth note'};
  return {rate:match[3],unit:`${match[2]?'Dotted ':''}${units[match[1]].toLowerCase()}`};
}
function fraction(q:Q):string {
  const gcd=(a:bigint,b:bigint):bigint=>b?gcd(b,a%b):a;
  const divisor=gcd(q.n<0n?-q.n:q.n,q.d),n=q.n/divisor,d=q.d/divisor;
  return d===1n?String(n):`${n}/${d}`;
}
export function noteLocation(event:ScoreEvent,view:View){
  const voice=view.voices.find(v=>v.id===event.voice),part=view.parts.find(p=>p.id===voice?.part);
  const time=new Q(event.time);
  const measure=view.measures.find(m=>m.part===part?.id&&time.cmp(new Q(m.time))>=0&&time.cmp(new Q(m.time).add(new Q(m.length)))<0);
  const states:Record<string,unknown>={};
  Object.assign(states,...view.states.filter(s=>s.time==='0'&&s.scope===`${part?.id}.s${event.staff}`).map(s=>s.values));
  const meter=/^\d+\/(\d+)$/.exec(String(states.meter??''));
  const offset=measure?time.sub(new Q(measure.time)):null;
  const beat=offset&&meter?fraction(new Q(offset.n*BigInt(meter[1]),offset.d*4n).add(new Q('1'))):null;
  return {part:part?.instrument??'',measure:measure?.label??measure?.id??'Outside measures',beat:beat??`${event.time} q`,staff:String(event.staff),voice:event.voice,line:event.source_line?.toString()??'Unavailable'};
}
