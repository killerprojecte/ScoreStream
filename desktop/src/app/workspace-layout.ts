import type {Preferences} from '../preferences-model';
/** Legacy preferences remain pixels; only the official group's layout API uses percentages. */
export function workspaceLayout(width:number,p:Pick<Preferences,'view'|'showInspector'|'sourceWidth'|'inspectorWidth'>){
 const hasSource=p.view==='split',hasInspector=p.showInspector;
 const available=Math.max(1,width-Number(hasSource)-Number(hasInspector));
 const inspector=hasInspector?Math.max(240,Math.min(440,p.inspectorWidth,available-320-(hasSource?220:0))):0;
 const source=hasSource?Math.max(220,Math.min(680,p.sourceWidth,available-inspector-320)):0;
 return {...hasSource?{source:source/available*100}:{},score:(available-source-inspector)/available*100,...hasInspector?{'inspector-panel':inspector/available*100}:{}};
}
export function resizePreferences(previous:Pick<Preferences,'sourceWidth'|'inspectorWidth'>,source:number|undefined,inspector:number|undefined){
 const patch:Partial<Pick<Preferences,'sourceWidth'|'inspectorWidth'>>={};
 if(source!==undefined&&Number.isFinite(source)){const value=Math.max(220,Math.min(680,Math.round(source)));if(value!==previous.sourceWidth)patch.sourceWidth=value;}
 if(inspector!==undefined&&Number.isFinite(inspector)){const value=Math.max(240,Math.min(440,Math.round(inspector)));if(value!==previous.inspectorWidth)patch.inspectorWidth=value;}
 return patch;
}
