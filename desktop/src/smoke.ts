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
 // Layout controls operate in the real WebKit window, not a browser mock.
 const get=(id:string)=>document.getElementById(id)!;
 const click=(id:string)=>get(id).click();
 const initialRevision=app.current!.revision_id;
 const initialSource=app.current!.source;
 const initialSvg=document.querySelector('#notation svg');
 click('toggle-title');assert(get('score-heading').hidden&&get('toggle-title').getAttribute('aria-pressed')==='false','Page title can be hidden without hiding tempo or instrument');
 assert(!get('tempo-text').hidden&&!get('instrument-names').hidden&&document.querySelector('#notation svg')===initialSvg,'Title visibility leaves musical notation and metadata intact');
 click('toggle-title');assert(!get('score-heading').hidden,'Page title is restored on repeated toggle');await delay(300);
 const splitWidth=get('canvas-scroll').clientWidth;
 let sequence=app.sequence;click('view-score');await until(()=>app.sequence>sequence&&app.phase==='ready');
 assert(get('canvas-scroll').clientWidth>splitWidth&&get('view-score').getAttribute('aria-pressed')==='true','Score focus expands and re-engraves the actual native pane');
 sequence=app.sequence;click('view-split');await until(()=>app.sequence>sequence&&app.phase==='ready');
 assert(get('canvas-scroll').clientWidth===splitWidth,'Split view restores the original pane width');
 sequence=app.sequence;click('toggle-inspector');await until(()=>app.sequence>sequence&&app.phase==='ready');
 assert(get('workspace').classList.contains('inspector-hidden'),'Inspector can be hidden for more score space');
 sequence=app.sequence;click('toggle-inspector');await until(()=>app.sequence>sequence&&app.phase==='ready');
 assert(get('canvas-scroll').clientWidth===splitWidth&&!get('workspace').classList.contains('inspector-hidden'),'Inspector restores without losing score layout');
 const noteBeforeZoom=document.querySelector('#notation g.note')!.getBoundingClientRect().width;
 const zoomSequence=app.sequence;
 for(let i=0;i<14;i++)click('zoom-in');
 assert(app.zoom===2&&(get('zoom-in') as HTMLButtonElement).disabled,'Repeated zoom-in stops at 200 percent');
 await delay(500);
 const noteAfterZoom=document.querySelector('#notation g.note')!.getBoundingClientRect().width;
 assert(Math.abs(noteAfterZoom/noteBeforeZoom-2)<.04,'200 percent zoom doubles the rendered note width');
 assert(get('canvas-scroll').scrollWidth>get('canvas-scroll').clientWidth,'Magnified notation remains horizontally scrollable');
 assert(app.sequence===zoomSequence&&app.phase==='ready','Zoom settles without scrollbar-triggered re-engraving');
 click('fit');assert(app.zoom===1&&get('zoom-level').textContent==='100%','Fit restores a stable 100 percent baseline');
 for(let i=0;i<10;i++)click('zoom-out');
 assert(app.zoom===.6&&(get('zoom-out') as HTMLButtonElement).disabled,'Repeated zoom-out stops at 60 percent');click('fit');
 click('about');assert((get('about-dialog') as HTMLDialogElement).open,'About dialog opens');click('close-about');assert(!(get('about-dialog') as HTMLDialogElement).open,'About dialog closes and returns to the editor');
 assert(app.current!.revision_id===initialRevision&&app.current!.source===initialSource,'View, title, zoom and inspector controls do not mutate the score');
 app.select(pitchEvent.id);(get('duration-input') as HTMLInputElement).value='2';await app.mutate('duration');await until(()=>app.phase==='ready');
 assert(app.current!.events.some(e=>e.id===pitchEvent.id&&e.duration==='2'),'Inspector commits typed duration edit through Rust');
 await app.history('undo');await until(()=>app.phase==='ready');assert(app.current!.source===initialSource,'Undo restores source after duration edit');
 app.select('');app.editor.selectLine(1);
 const footer=document.querySelector('.statusbar')!.getBoundingClientRect();
 assert(footer.bottom<=innerHeight+1&&get('diagnostics').getBoundingClientRect().bottom<=footer.top+1,'Status and diagnostics stay inside native viewport without overlaying content');
 const firstNote=document.querySelector('#notation g.note')!.getBoundingClientRect();
 const layout={viewport:[innerWidth,innerHeight],scorePaneWidth:splitWidth,firstNoteTop:Math.round(firstNote.top),firstNoteHeight:Math.round(firstNote.height),titleVisible:!get('score-heading').hidden};
 await invoke('report_smoke_test',{report:JSON.stringify({passed:true,checks,layout,renderer:'Verovio 5.6.0',host:'Tauri / Linux WebKitGTK',notes:document.querySelectorAll('#notation g.note').length},null,2)});

 }catch(e){await invoke('report_smoke_test',{report:JSON.stringify({passed:false,checks,error:String(e)},null,2)});}}
