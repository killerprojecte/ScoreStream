import type {PlaybackMeasure,PlaybackPlan} from './types';
export function displayMeasures(plan:PlaybackPlan|null):PlaybackMeasure[] {
 if(!plan?.measures.length)return [];
 const part=plan.measures[0].part;
 return plan.measures.filter(measure=>measure.part===part).sort((a,b)=>a.start_seconds-b.start_seconds||a.id.localeCompare(b.id));
}
export function measureAt(measures:PlaybackMeasure[],position:number):PlaybackMeasure|undefined {
 return measures.find(measure=>position>=measure.start_seconds&&position<measure.end_seconds)
  ??(position===measures.at(-1)?.end_seconds?measures.at(-1):undefined);
}
export function formatTime(seconds:number):string {
 const tenths=Math.floor(Math.max(0,seconds)*10+1e-8);
 return `${Math.floor(tenths/600)}:${String(Math.floor(tenths/10)%60).padStart(2,'0')}.${tenths%10}`;
}
export function parseSeconds(text:string,duration:number):number|null {
 if(!/^\d+(?:\.\d+)?$/.test(text.trim()))return null;
 const value=Number(text);
 return Number.isFinite(value)&&value>=0&&value<=duration?value:null;
}

/** Stable selector key for a performed visit; authored ids still drive notation. */
export function measureKey(measure:PlaybackMeasure):string {
 return measure.occurrence_index===undefined?measure.id:`occ${measure.occurrence_index}:${measure.id}`;
}
