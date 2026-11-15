import type {MarkRecord,Scalar,ScoreEvent,View} from '../types';
import {Q} from './exact';

const namespace='scorestream.articulation';
export const isAuthoredArticulation=(kind:string)=>kind===namespace||kind.startsWith(`${namespace}.`);
export const authoredSigns=['accent','strong-accent','staccato','tenuto','detached-legato','staccatissimo','spiccato','scoop','plop','doit','falloff','breath-mark','caesura','stress','unstress','soft-accent','other-articulation'] as const;
export type AuthoredSign=typeof authoredSigns[number];
type Placement='above'|'below';
const styleFields=['default_x','default_y','relative_x','relative_y','font_family','font_style','font_size_points','font_size_css','font_weight','color'] as const;
type Style=Partial<Record<typeof styleFields[number],string>>;
const lineFields=['line_shape','line_type','line_length','dash_length','space_length'] as const;
type Line=Partial<Record<typeof lineFields[number],string>>;
type SignData={sign:Exclude<AuthoredSign,'strong-accent'|'scoop'|'plop'|'doit'|'falloff'|'breath-mark'|'caesura'|'other-articulation'>}
 |{sign:'strong-accent';orientation?:'up'|'down'}
 |{sign:'scoop'|'plop'|'doit'|'falloff';line:Line}
 |{sign:'breath-mark';shape?:'comma'|'tick'|'upbow'|'salzedo'}
 |{sign:'caesura';shape?:'normal'|'thick'|'short'|'curved'|'single'}
 |{sign:'other-articulation';text:string;smufl?:string};
export type AuthoredItem=SignData&{placement?:Placement;style:Style;id:string;renderIds:string[];hidden:boolean};
export interface AuthoredGroup {sourceId?:string;items:AuthoredItem[]}
export interface AuthoredWrapper {sourceId?:string;printObject?:boolean;groups:AuthoredGroup[]}
export interface AuthoredEvent {eventId:string;notations:AuthoredWrapper[]}
export interface AuthoredProjection {version:'1';events:AuthoredEvent[]}
// An exhaustive family inventory, not a promise of performance interpretation.
export const authoredCapabilities:Record<AuthoredSign,'native'|'explicit-placement'|'unsupported'>={
 accent:'native','strong-accent':'native',staccato:'native',tenuto:'native','detached-legato':'native',
 staccatissimo:'native',spiccato:'explicit-placement',stress:'explicit-placement',unstress:'explicit-placement',
 'soft-accent':'native',scoop:'unsupported',plop:'unsupported',doit:'unsupported',falloff:'unsupported',
 'breath-mark':'unsupported',caesura:'unsupported','other-articulation':'unsupported',
};
type Row={time:string;mark:MarkRecord;path:number[]};
const objectName=(row:Row)=>`${row.mark.target} / ${row.path.map((n,i)=>`${['event','notations','group','item'][i]}[${n}]`).join('/')} ${typeof row.mark.attributes.sign==='string'?`(${row.mark.attributes.sign})`:''}`.trim();
function fail(object:string,reason:string):never {throw Error(`Authored articulation ${object}: ${reason}. Preview withheld; source is unchanged.`);}
const xmlText=(s:string)=>/^[\u0009\u000A\u000D\u0020-\uD7FF\uE000-\uFFFD\u{10000}-\u{10FFFF}]*$/u.test(s);
const indexNames=['event_index','wrapper_index','group_index','item_index'];
const kinds=['event','notations','group','item'];
const sourcePattern=/^[A-Za-z_][A-Za-z0-9_.-]*$/;
const string=(a:Record<string,Scalar>,key:string,object:string):string|undefined=>{
 const value=a[key];if(value!==undefined&&typeof value!=='string')fail(object,`${key} must be a string`);return value as string|undefined;
};
function choice<T extends string>(a:Record<string,Scalar>,key:string,values:readonly T[],object:string):T|undefined {
 const value=string(a,key,object);if(value!==undefined&&!values.includes(value as T))fail(object,`invalid ${key}`);return value as T|undefined;
}
function rational(value:string,object:string,key:string){
 if(value.length>128||!/^[-+]?\d+(?:\/\d+)?$/.test(value))fail(object,`invalid exact ${key}`);
 try {new Q(value);}catch{fail(object,`invalid exact ${key}`);}
}
function decodeItem(row:Row,hidden:boolean):AuthoredItem {
 const object=objectName(row),a=row.mark.attributes,sign=choice(a,'sign',authoredSigns,object);
 if(!sign)fail(object,'missing sign');
 const extra=sign==='strong-accent'?['orientation']:['scoop','plop','doit','falloff'].includes(sign)?lineFields:
  sign==='breath-mark'||sign==='caesura'?['shape']:sign==='other-articulation'?['text','smufl']:[];
 const allowed=[...indexNames,'sign','placement',...styleFields,...extra];
 for(const key of Object.keys(a))if(!allowed.includes(key))fail(object,`unknown or inapplicable field ${key}`);
 const style:Style={};for(const key of styleFields){const value=string(a,key,object);if(value!==undefined)style[key]=value;}
 for(const key of ['default_x','default_y','relative_x','relative_y','font_size_points'] as const)if(style[key]!==undefined)rational(style[key],object,key);
 choice(a,'font_style',['normal','italic'],object);choice(a,'font_weight',['normal','bold'],object);
 choice(a,'font_size_css',['xx-small','x-small','small','medium','large','x-large','xx-large'],object);
 if(style.font_size_points!==undefined&&style.font_size_css!==undefined)fail(object,'conflicting font sizes');
 if(style.color!==undefined&&!/^#(?:[0-9A-F]{6}|[0-9A-F]{8})$/.test(style.color))fail(object,'invalid color');
 if(style.font_family!==undefined&&(!xmlText(style.font_family)||!style.font_family||style.font_family!==style.font_family.split(/[ \t\r\n]+/).filter(Boolean).join(' ')||style.font_family.split(',').some(v=>!v)))fail(object,'invalid font family');
 let data:SignData;
 switch(sign){
  case 'strong-accent':data={sign,orientation:choice(a,'orientation',['up','down'],object)};break;
  case 'scoop':case 'plop':case 'doit':case 'falloff':{
   const line:Line={};for(const key of lineFields){const value=string(a,key,object);if(value!==undefined)line[key]=value;}
   choice(a,'line_shape',['straight','curved'],object);choice(a,'line_type',['solid','dashed','dotted','wavy'],object);choice(a,'line_length',['short','medium','long'],object);
   for(const key of ['dash_length','space_length'] as const)if(line[key]!==undefined)rational(line[key],object,key);
   data={sign,line};break;
  }
  case 'breath-mark':data={sign,shape:choice(a,'shape',['comma','tick','upbow','salzedo'],object)};break;
  case 'caesura':data={sign,shape:choice(a,'shape',['normal','thick','short','curved','single'],object)};break;
  case 'other-articulation':{
   const text=string(a,'text',object),smufl=string(a,'smufl',object);
   if(text===undefined||!xmlText(text))fail(object,'invalid passive text');
   if(smufl!==undefined&&!/^[A-Za-z0-9_.:-]+$/.test(smufl))fail(object,'invalid passive glyph name');
   data={sign,text,smufl};break;
  }
  default:data={sign};
 }
 return {...data,placement:choice(a,'placement',['above','below'],object),style,id:`ss-aa-${row.path.join('-')}`,renderIds:[],hidden};
}

// Consume only the typed scalar Workspace transport, never parse SFS/XML source.
// The Rust validator remains authoritative; these guards also fail closed on
// stale/future/malformed worker inputs instead of partially engraving them.
export function decodeAuthoredArticulations(view:View):AuthoredProjection|null {
 const extensions=view.document.required_extensions;
 if(extensions!==undefined&&(!Array.isArray(extensions)||extensions.some(e=>typeof e!=='string')))fail('capability','required_extensions must be an array of strings');
 const requirements=Array.isArray(extensions)?extensions.filter((e):e is string=>typeof e==='string'&&e.startsWith(`${namespace}@`)):[];
 const all=(view.document.frames??[]).flatMap(f=>f.records.map(record=>({time:f.time,record})));
 const entries=all.filter((e):e is {time:string;record:MarkRecord}=>e.record.record==='mark'&&isAuthoredArticulation(e.record.kind));
 if(!requirements.length&&!entries.length)return null;
 if(requirements.length!==1||requirements[0]!==`${namespace}@1`)fail('capability','requires exactly scorestream.articulation@1');
 if(!entries.length)fail('capability','requires event records');
 if(entries.length>10000)fail('capability','preview is limited to 10000 authored hierarchy records');
 const rows:Row[]=entries.map(({time,record:mark})=>{
  const depth=kinds.indexOf(mark.kind.slice(namespace.length+1));
  if(depth<0)fail(mark.target,'unknown reserved kind');
  const path=indexNames.slice(0,depth+1).map(key=>{const n=mark.attributes[key];if(typeof n!=='number'||!Number.isSafeInteger(n)||n<0||n>=10000)fail(mark.target,`invalid ${key}`);return n;});
  const row={time,mark,path};
  if(depth<3){const allowed=[...indexNames.slice(0,depth+1),...(depth===1?['source_id','print_object']:depth===2?['source_id']:[])];for(const key of Object.keys(mark.attributes))if(!allowed.includes(key))fail(objectName(row),`unknown field ${key}`);}
  return row;
 }).sort((a,b)=>{for(let i=0;i<Math.min(a.path.length,b.path.length);i++){if(a.path[i]!==b.path[i])return a.path[i]-b.path[i];}return a.path.length-b.path.length;});
 const hosts=new Map<string,ScoreEvent|null>();for(const event of view.events)hosts.set(event.id,hosts.has(event.id)?null:event);
 const nativeIds=new Set([...view.parts,...view.voices,...view.measures,...view.events].map(x=>x.id));
 for(const event of view.events)if(event.chord)nativeIds.add(event.chord);
 const incompatible=new Set<string>();
 for(const {record} of all){
  if(record.record==='mark'){
   const a=record.attributes;for(const key of ['id',...(['scorestream.instrument','scorestream.navigation'].includes(record.kind)?['source_id']:[])])if(typeof a[key]==='string')nativeIds.add(a[key]);
   if(record.kind==='articulation'||record.kind==='scorestream.fermata')incompatible.add(record.target);
  }else if(record.record==='relation'){
   if(typeof record.id==='string')nativeIds.add(record.id);
   if(record.kind==='tie'||record.kind==='slur')if(typeof record.anchor==='string')incompatible.add(record.anchor);
  }
 }
 const sourceIds=new Set<string>(),seen=new Set<string>(),eventIds=new Set<string>(),parents=new Map<number,Row>();
 const result:AuthoredProjection={version:'1',events:[]};
 for(const row of rows){
  const {mark,path,time}=row,[e,w,g,i]=path,object=objectName(row),key=path.join('/');
  if(seen.has(key))fail(object,'duplicate hierarchy index');seen.add(key);
  if(path.length===1){
   if(e!==result.events.length)fail(object,'event indices must be contiguous');
   const id=mark.target.startsWith('#')?mark.target.slice(1):'',host=hosts.get(id);
   if(!host||eventIds.has(id))fail(object,'requires one unique event target');
   const voice=view.voices.find(v=>v.id===host.voice),part=view.parts.find(p=>p.id===voice?.part);
   if(!part||host.staff<1||host.staff>part.staves||!Number.isInteger(host.staff)||host.duration==='?'||new Q(host.duration).cmp(new Q('0'))<=0||!/^[A-G](#{1,2}|b{1,2})?-?\d+$/.test(host.pitch)||host.attributes?.measure_rest===true||host.attributes?.kind!==undefined&&host.attributes.kind!=='note')fail(object,'requires an owned measured pitched event');
   if(!new Q(time).eq(new Q(host.time)))fail(object,'attachment must match event onset');
   if(incompatible.has(mark.target))fail(object,'mixed notation families on the same host are unsupported');
   eventIds.add(id);parents.set(e,row);result.events.push({eventId:id,notations:[]});continue;
  }
  const parent=parents.get(e),event=result.events[e];
  if(!parent||!event)fail(object,'missing event parent');
  if(parent.mark.target!==mark.target||!new Q(parent.time).eq(new Q(time)))fail(object,'hierarchy target/onset differs from event');
  let sourceId:string|undefined;
  if(path.length<4){sourceId=string(mark.attributes,'source_id',object);if(sourceId!==undefined){if(!sourcePattern.test(sourceId)||sourceIds.has(sourceId)||nativeIds.has(sourceId))fail(object,'invalid, duplicate or colliding source identity');sourceIds.add(sourceId);}}
  if(path.length===2){
   if(w!==event.notations.length)fail(object,'notations indices must be contiguous');
   const printObject=mark.attributes.print_object;if(printObject!==undefined&&typeof printObject!=='boolean')fail(object,'print_object must be boolean');
   event.notations.push({sourceId,printObject:printObject as boolean|undefined,groups:[]});continue;
  }
  const wrapper=event.notations[w];if(!wrapper)fail(object,'missing notations parent');
  if(path.length===3){if(g!==wrapper.groups.length)fail(object,'group indices must be contiguous');wrapper.groups.push({sourceId,items:[]});continue;}
  const group=wrapper.groups[g];if(!group)fail(object,'missing group parent');
  if(i!==group.items.length)fail(object,'item indices must be contiguous');group.items.push(decodeItem(row,wrapper.printObject===false));
 }
 for(const event of result.events)if(!event.notations.some(w=>w.groups.length))fail(`#${event.eventId}`,'requires at least one group');
 return result;
}

// Closed, independently checked glyph mappings. User-provided glyph names never
// enter MEI. Strong-accent orientation is NOT inferred from vertical placement.
const nativeGlyphs={accent:'acc',staccato:'stacc',tenuto:'ten',staccatissimo:'stacciss','soft-accent':'acc-soft'} as const;
const fixedGlyphs={spiccato:{artic:'spicc',glyph:'articStaccatissimoStroke'},stress:{artic:'stress',glyph:'articStress'},unstress:{artic:'unstress',glyph:'articUnstress'}} as const;
export function projectAuthoredArticulations(view:View){
 const authored=decodeAuthoredArticulations(view),xmlByEvent=new Map<string,{xml:string;event:AuthoredEvent}>(),handled=new Set<string>();
 for(const [e,event] of (authored?.events??[]).entries()){
  let xml='';
  for(const [w,wrapper] of event.notations.entries())for(const [g,group] of wrapper.groups.entries())for(const [i,item] of group.items.entries()){
   const object=`#${event.eventId} / event[${e}]/notations[${w}]/group[${g}]/item[${i}] (${item.sign})`;
   // print-object=no suppresses the complete wrapper. Retain every validated
   // item in the receipt; never send passive text/appearance/SMuFL to Verovio.
   if(item.hidden)continue;
   const styles=Object.keys(item.style);if(styles.length)fail(object,`unsupported appearance/coordinate field ${styles[0]}`);
   const capability=authoredCapabilities[item.sign];
   if(capability==='unsupported')fail(object,item.sign==='other-articulation'?'passive text and custom glyph rendering are unsupported':`unsupported ${item.sign} engraving`);
   if(capability==='explicit-placement'&&!item.placement)fail(object,'exact glyph requires explicit above/below placement in Verovio 5.6.0');
   const place=item.placement?` place="${item.placement}"`:'';
   const artic=(suffix:string,attrs:string)=>{const id=item.id+suffix;item.renderIds.push(id);return `<artic xml:id="${id}"${attrs}${place}/>`;};
   if(item.sign in nativeGlyphs)xml+=artic('',` artic="${nativeGlyphs[item.sign as keyof typeof nativeGlyphs]}"`);
   else if(item.sign==='strong-accent')xml+=artic('',` artic="marc" glyph.auth="smufl" glyph.name="articMarcato${item.orientation==='down'?'Below':'Above'}"`);
   else if(item.sign==='detached-legato')xml+=artic('-stacc',' artic="stacc"')+artic('-ten',' artic="ten"');
   else if(item.sign in fixedGlyphs){const mapping=fixedGlyphs[item.sign as keyof typeof fixedGlyphs];xml+=artic('',` artic="${mapping.artic}" glyph.auth="smufl" glyph.name="${mapping.glyph}${item.placement==='above'?'Above':'Below'}"`);}
  }
  xmlByEvent.set(event.eventId,{xml,event});
 }
 return {authored,
  forEvent(eventId:string,map:Record<string,string>){
   const entry=xmlByEvent.get(eventId);if(!entry)return '';
   if(handled.has(eventId))fail(`#${eventId}`,'host engraved twice');handled.add(eventId);
   for(const wrapper of entry.event.notations)for(const group of wrapper.groups)for(const item of group.items)for(const id of item.renderIds)map[id]=eventId;
   return entry.xml;
  },
  finish(){for(const eventId of xmlByEvent.keys())if(!handled.has(eventId))fail(`#${eventId}`,'host outside engraved measures or staff mapping');},
 };
}
