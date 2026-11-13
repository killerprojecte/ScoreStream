import type {Workbench} from './main';
import {backend} from './bridge';
import {invoke} from '@tauri-apps/api/core';
const delay=(ms:number)=>new Promise(r=>setTimeout(r,ms));
async function until(predicate:()=>boolean,timeout=30000){const start=Date.now();while(!predicate()){if(Date.now()-start>timeout)throw Error('Timed out waiting for native UI state');await delay(50);}}
export async function run(app:Workbench){const checks:string[]=[];const assert=(condition:unknown,name:string)=>{if(!condition)throw Error(name);checks.push(name);};try{
 await until(()=>app.phase==='ready');assert(document.querySelectorAll('#notation g.note').length>0,'Native WebKit rendered Verovio SVG notes');
 const original=app.current!;const pitchEvent=original.events.find(e=>/^[A-G]/.test(e.pitch))!;
 app.select(pitchEvent.id,true);assert(!document.getElementById('selection')!.hidden,'SVG identity resolves selected note');assert(app.editor.editor.getPosition()?.lineNumber===pitchEvent.source_line,'Notation selection maps to Rust source line');
 const replacement=pitchEvent.pitch==='D4'?'E4':'D4';const draft=original.source.replace(`${pitchEvent.pitch}:${pitchEvent.duration}`,`${replacement}:${pitchEvent.duration}`);
 app.editor.value=draft;app.changed();await until(()=>app.phase==='ready'&&app.dirty);assert(app.published?.dirty,'Valid uncommitted source gets automatic live preview');assert((await backend.current()).revision_id===original.revision_id,'Live preview does not commit a revision');
 app.select(pitchEvent.id);assert((document.getElementById('set-pitch') as HTMLButtonElement).disabled,'Notation mutation disabled while draft is dirty');
 // Selection highlighting is intentionally cleared on edit; retain the actual SVG node.
 const lastSvg=document.querySelector('#notation svg');assert(lastSvg,'Last good SVG exists before invalid draft');app.editor.value=draft+'\n@';app.changed();await until(()=>app.phase==='invalid');assert(document.querySelector('#notation svg')===lastSvg,'Invalid draft retains the last good SVG');assert(!document.getElementById('stale-banner')!.hidden,'Invalid draft visibly marks stale preview');
 app.editor.value=draft;app.changed();app.editor.value=draft+'\n@';app.changed();await until(()=>app.phase==='invalid');await delay(500);assert(app.phase==='invalid'&&document.querySelector('#notation svg')===lastSvg,'Obsolete pending draft cannot replace a newer invalid draft');
 app.editor.value=draft;app.changed();await until(()=>app.phase==='ready');await app.apply();await until(()=>app.phase==='ready');assert(app.current!.revision_id!==original.revision_id&&!app.dirty,'Apply creates committed revision and clears dirty state');
 await app.history('undo');await until(()=>app.phase==='ready');assert(app.current!.source===original.source,'Committed undo restores original source');await app.history('redo');await until(()=>app.phase==='ready');assert(app.current!.source===draft,'Committed redo restores edited source');
 try{await backend.apply(original.source,original.revision_id);throw Error('Conflict was not rejected');}catch(e){assert(String(e).includes('REVISION_CONFLICT'),'Stale expected revision is rejected');}
 app.select(pitchEvent.id);(document.getElementById('pitch-input') as HTMLInputElement).value='F4';await app.mutate('pitch');await until(()=>app.phase==='ready');assert(app.current!.events.some(e=>e.id===pitchEvent.id&&e.pitch==='F4'),'Inspector commits typed pitch edit through Rust');
 await app.history('undo');await until(()=>app.phase==='ready');await app.history('undo');await until(()=>app.phase==='ready');app.select(pitchEvent.id,true);
 await invoke('report_smoke_test',{report:JSON.stringify({passed:true,checks,renderer:'Verovio 5.6.0',host:'Tauri / Linux WebKitGTK',notes:document.querySelectorAll('#notation g.note').length},null,2)});
 }catch(e){await invoke('report_smoke_test',{report:JSON.stringify({passed:false,checks,error:String(e)},null,2)});}}
