import type {Language} from './i18n';
export interface Preferences {version:1;language:Language;theme:'system'|'light'|'dark';material:'glass'|'solid';glassQuality:'auto'|'layered';fontSize:number;wrap:boolean;showTitle:boolean;showInspector:boolean;view:'split'|'score';sourceWidth:number;inspectorWidth:number}
export const PREFERENCES_KEY='scorestream.preferences.v1';
export const defaults=(locale='en'):Preferences=>({version:1,language:locale.toLowerCase().startsWith('zh')?'zh-CN':'en',theme:'system',material:'glass',glassQuality:'auto',fontSize:12,wrap:false,showTitle:true,showInspector:true,view:'split',sourceWidth:360,inspectorWidth:256});
const bounded=(value:unknown,fallback:number,min:number,max:number)=>typeof value==='number'&&Number.isFinite(value)?Math.max(min,Math.min(max,value)):fallback;
export function parsePreferences(raw:string|null,locale='en'):Preferences {
 const base=defaults(locale);try{const p=JSON.parse(raw??'null');if(!p||p.version!==1)return base;
 return {...base,language:p.language==='en'||p.language==='zh-CN'?p.language:base.language,theme:['system','light','dark'].includes(p.theme)?p.theme:base.theme,material:p.material==='solid'?'solid':'glass',glassQuality:p.glassQuality==='layered'?'layered':'auto',fontSize:bounded(p.fontSize,base.fontSize,10,22),wrap:typeof p.wrap==='boolean'?p.wrap:base.wrap,showTitle:typeof p.showTitle==='boolean'?p.showTitle:base.showTitle,showInspector:typeof p.showInspector==='boolean'?p.showInspector:base.showInspector,view:p.view==='score'?'score':'split',sourceWidth:bounded(p.sourceWidth,base.sourceWidth,220,680),inspectorWidth:bounded(p.inspectorWidth,base.inspectorWidth,240,440)};
 }catch{return base;}
}
/** No secrets, paths or score content enter the preference store. */
export class PreferenceStore {
 value:Preferences;onChange:(preferences:Preferences)=>void=()=>{};saved=true;
 constructor(private storage:Pick<Storage,'getItem'|'setItem'>|null,locale='en') {let raw=null;try{raw=storage?.getItem(PREFERENCES_KEY)??null;}catch{}this.value=parsePreferences(raw,locale);}
 update(changes:Partial<Omit<Preferences,'version'>>){this.value=parsePreferences(JSON.stringify({...this.value,...changes}));try{if(!this.storage)throw Error();this.storage.setItem(PREFERENCES_KEY,JSON.stringify(this.value));this.saved=true;}catch{this.saved=false;}this.onChange(this.value);}
}
export function fitPanes(width:number,source:number,inspector:number,sourceVisible=true,inspectorVisible=true){
 const sidebar=inspectorVisible?Math.round(Math.max(240,Math.min(440,inspector,width-320-(sourceVisible?226:6)))):0;
 const editor=sourceVisible?Math.round(Math.max(220,Math.min(680,source,width-sidebar-320-(inspectorVisible?12:6)))):0;
 return {source:editor,inspector:sidebar};
}
