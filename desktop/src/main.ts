import './style.css';
import { shell } from './shell';
import { clampZoom, engravingWidth, MIN_ZOOM, MAX_ZOOM } from './layout';
import { backend,native } from './bridge';
import { SourceEditor } from './editor/source';
import { project,RENDERER } from './render/project';
import { LatestOnly } from './render/scheduler';
import { sanitizeSvg } from './render/sanitize';
import RenderWorker from './render/worker?worker';
import type {View,RenderResult,PreviewKey,ScoreEvent} from './types';
const $=<T extends HTMLElement=HTMLElement>(id:string)=>document.getElementById(id) as T;
$('app').innerHTML = shell;

export class Workbench {
 editor:SourceEditor; current:View|null=null; view:View|null=null; published:View|null=null; selected:ScoreEvent|null=null;
 sequence=0;renderedSequence=0;phase='loading';busy=false;zoom=1;fileName='First light.sfs';
 private timer=0;private snapshotByRequest=new Map<number,View>();private renderedMap:Record<string,string>={};
 private session=crypto.randomUUID();private scheduler:LatestOnly;
 private resizeTimer=0;private layoutWidth=0;
 constructor(){
  this.editor=new SourceEditor($('editor'),()=>this.changed(),line=>this.selectSource(line));
  const worker=new RenderWorker();this.scheduler=new LatestOnly(worker,result=>this.publish(result));
  worker.onerror=e=>this.previewError(`Renderer failed: ${e.message}`);
  $('apply').onclick=()=>void this.apply();$('discard').onclick=()=>void this.discard();$('undo').onclick=()=>void this.history('undo');$('redo').onclick=()=>void this.history('redo');
  $('open').onclick=()=>void this.open();$('save').onclick=()=>void this.save();
  $('set-pitch').onclick=()=>void this.mutate('pitch');$('set-duration').onclick=()=>void this.mutate('duration');
  $('jump').onclick=()=>{this.setLayout('split');if(this.selected?.source_line)this.editor.selectLine(this.selected.source_line);this.editor.focus();};
  $('notation').onclick=e=>{let el=e.target as Element|null;while(el&&el!==$('notation')){if(el.id&&this.renderedMap[el.id]){this.select(this.renderedMap[el.id],true);break;}el=el.parentElement;}};
  $('zoom-in').onclick=()=>this.setZoom(this.zoom+.1);$('zoom-out').onclick=()=>this.setZoom(this.zoom-.1);$('fit').onclick=()=>this.setZoom(1);
  $('view-split').onclick=()=>this.setLayout('split');$('view-score').onclick=()=>this.setLayout('score');
  $('toggle-inspector').onclick=()=>{const hidden=$('workspace').classList.toggle('inspector-hidden');$('toggle-inspector').setAttribute('aria-pressed',String(!hidden));};
  $('toggle-title').onclick=()=>{const heading=$('score-heading');heading.hidden=!heading.hidden;$('toggle-title').setAttribute('aria-pressed',String(!heading.hidden));};
  new ResizeObserver(()=>{const width=this.measureWidth();if(width===this.layoutWidth)return;this.layoutWidth=width;clearTimeout(this.resizeTimer);this.resizeTimer=window.setTimeout(()=>this.relayout(),120);}).observe($('canvas-scroll'));
  $('about').onclick=()=>$<HTMLDialogElement>('about-dialog').showModal();$('close-about').onclick=()=>$<HTMLDialogElement>('about-dialog').close();
  document.addEventListener('keydown',e=>{if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='s'){e.preventDefault();void this.save();}if((e.ctrlKey||e.metaKey)&&e.key==='Enter'){e.preventDefault();void this.apply();}});
  window.addEventListener('beforeunload',e=>{if(this.dirty){e.preventDefault();e.returnValue='';}});
 }
 get dirty(){return !!this.current&&this.editor.value!==this.current.source;}
 async start(){if(!native){this.previewError('Open the native desktop app. This UI intentionally has no JavaScript parser or mock Rust backend.');$('connection').textContent='Native Tauri host required';return;}try{this.accept(await backend.current());$('connection').textContent='Ready';}catch(e){this.error(e);}}
 private message(text:string){$('global-message').textContent=text;}
 private error(e:unknown){this.message(String(e));console.error(e);}
 private invalidate(){clearTimeout(this.timer);this.sequence++;this.scheduler.invalidate(this.sequence);this.snapshotByRequest.clear();this.phase='pending';this.update();return this.sequence;}
 changed(){this.invalidate();this.view=null;this.selected=null;this.updateSelection();this.timer=window.setTimeout(()=>void this.preview(this.sequence),280);}
 async preview(sequence=this.sequence){const source=this.editor.value;try{const view=await backend.preview(source);if(sequence!==this.sequence||source!==this.editor.value)return;this.view=view;this.editor.diagnostics(view.diagnostics);this.showDiagnostics(view);if(!view.valid){this.phase='invalid';this.update();return;}await this.render(view,sequence);}catch(e){if(sequence===this.sequence)this.previewError(String(e));}}
 private async render(view:View,sequence:number){
  try{const projected=project(view);const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(view.source)))).map(x=>x.toString(16).padStart(2,'0')).join('');if(sequence!==this.sequence)return;
   const width=this.measureWidth();this.layoutWidth=width;const layout=`${width}:${RENDERER}`;
   const key:PreviewKey=view.dirty?{kind:'draft',session:this.session,sequence,hash,baseRevision:this.current!.revision_id,layout}:{kind:'committed',revision:view.revision_id,layout};
   this.snapshotByRequest.set(sequence,view);this.phase='rendering';this.update();const warning=$('projection-warning');warning.hidden=!projected.warnings.length;warning.textContent=projected.warnings.join(' ');
   this.scheduler.submit({requestId:sequence,key,mei:projected.mei,width,map:projected.map});
  }catch(e){if(sequence===this.sequence)this.previewError(String(e));}
 }
 private publish(result:RenderResult){if(result.requestId!==this.sequence)return;if(result.error){this.previewError(result.error);return;}const view=this.snapshotByRequest.get(result.requestId);if(!view||view.source!==this.editor.value)return;this.published=view;this.renderedMap=result.map;this.renderedSequence=result.requestId;this.phase='ready';$('notation').innerHTML=result.pages.map(svg=>`<div class="notation-page">${sanitizeSvg(svg)}</div>`).join('');$('page-count').textContent=`${result.pages.length} page${result.pages.length===1?'':'s'}`;$('render-time').textContent=`${Math.round(result.elapsed)} ms · Verovio`;this.update();this.updateSelection();}
 private previewError(message:string){this.phase='unsupported';this.message(message);if(!this.published)$('notation').innerHTML='<div class="empty-state"><h3>Preview unavailable</h3><p id="preview-error-text"></p></div>';$('preview-error-text')?.append(document.createTextNode(message));this.update();}
 private accept(view:View){this.current=view;this.view=view;this.editor.value=view.source;this.editor.diagnostics(view.diagnostics);const seq=this.invalidate();this.showDiagnostics(view);void this.render(view,seq);this.selected=null;this.updateSelection();this.update();}
 async apply(){if(this.busy||!this.dirty||!this.view?.valid||this.view.source!==this.editor.value)return;await this.transaction(async()=>{const view=await backend.apply(this.editor.value,this.current!.revision_id);this.accept(view);this.message('Draft applied. A new committed revision is ready.');});}
 async discard(){if(!this.current||this.busy)return;await this.transaction(async()=>{await backend.preview(this.current!.source);this.accept(this.current!);this.message('Draft discarded. Showing the committed score.');});}
 async history(direction:'undo'|'redo'){if(this.dirty||this.busy)return;await this.transaction(async()=>{this.accept(await backend[direction]());this.message(`${direction==='undo'?'Undo':'Redo'} committed as a new revision.`);});}
 private async transaction(action:()=>Promise<void>){this.busy=true;this.editor.readOnly=true;this.update();try{await action();}catch(e){this.error(e);}finally{this.busy=false;this.editor.readOnly=false;this.update();}}
 async mutate(kind:'pitch'|'duration'){if(!this.selected||this.dirty||this.busy||this.phase!=='ready')return;const id=this.selected.id;const value=$<HTMLInputElement>(`${kind}-input`).value;await this.transaction(async()=>{const view=kind==='pitch'?await backend.setPitch(id,value,this.current!.revision_id):await backend.setDuration(id,value,this.current!.revision_id);this.accept(view);this.message('Note property committed through the Rust engine.');});}
 async open(){if(this.busy)return;if(this.dirty&&!confirm('Replace the current uncommitted draft?'))return;await this.transaction(async()=>{const file=await backend.openFile();if(!file)return;this.fileName=file.path.split(/[\\/]/).pop()!;this.editor.value=file.source;this.changed();this.message('File opened as a draft. Review and apply it to the current session.');});}
 async save(){if(this.busy||!this.view?.valid||this.view.source!==this.editor.value){this.message('Wait for Rust validation before saving.');return;}await this.transaction(async()=>{const path=await backend.saveFile(this.editor.value);if(path){this.fileName=path.split(/[\\/]/).pop()!;this.message(`Saved ${this.fileName}. Session history is not persisted.`);this.update();}});}
 private showDiagnostics(view:View){const host=$('diagnostics');host.replaceChildren();if(!view.diagnostics.length){host.innerHTML='<span class="valid-mark">✓</span> No validation issues';return;}for(const d of view.diagnostics.slice(0,3)){const button=document.createElement('button');button.className='diagnostic';button.textContent=`L${d.line} · ${d.message}`;button.onclick=()=>this.editor.selectLine(d.line);host.append(button);}}
 private update(){const dirty=this.dirty;const stale=this.phase!=='ready';$('filename').textContent=this.fileName;const title=this.fileName.replace(/\.sfs$/i,'').replace(/[-_]/g,' ');$('score-title').textContent=title;$('document-title').textContent=title;$('dirty-dot').hidden=!dirty;$('source-state').textContent=dirty?'Draft':'Committed';$('source-state').className=dirty?'draft-label':'';$('line-count').textContent=`${this.editor.value.split('\n').length} lines`;$('revision').textContent=this.current?.revision_id.slice(0,13)??'connecting';
  $<HTMLButtonElement>('apply').disabled=this.busy||!dirty||!this.view?.valid||this.view.source!==this.editor.value;$<HTMLButtonElement>('discard').disabled=this.busy||!dirty;$<HTMLButtonElement>('undo').disabled=this.busy||dirty||!this.current?.can_undo;$<HTMLButtonElement>('redo').disabled=this.busy||dirty||!this.current?.can_redo;$<HTMLButtonElement>('open').disabled=this.busy;$<HTMLButtonElement>('save').disabled=this.busy||!this.view?.valid||this.view.source!==this.editor.value;
  const status=$('preview-status');status.className=`status-pill ${stale?'pending':dirty?'draft':'valid'}`;status.textContent={loading:'Preparing',pending:'Updating',rendering:'Engraving',invalid:'Out of date',unsupported:'Limited preview',ready:dirty?'Live draft':'In sync'}[this.phase]??this.phase;
  const banner=$('stale-banner');banner.hidden=!stale||!this.published;banner.textContent=this.phase==='invalid'?'Source needs attention. Showing the last valid preview.':this.phase==='unsupported'?'This score exceeds the current preview scope. Last good notation retained.':'Updating the preview. The score below shows the previous snapshot.';
  $('preview-kind').textContent=dirty?`Private draft · based on ${this.current?.revision_id.slice(0,5)}`:'Committed score';$('paper').classList.toggle('stale',stale&&!!this.published);
  const doc=this.view?.valid?this.view:this.current;if(doc){$('document-summary').textContent=`${doc.parts.length} part${doc.parts.length===1?'':'s'} · ${new Set(doc.measures.map(m=>m.time)).size} measures`;$('event-count').textContent=`${doc.events.length} events`;const tempo=doc.states.find(s=>s.time==='0'&&s.scope==='score'&&s.values.tempo)?.values.tempo;$('tempo-text').textContent=tempo?String(tempo).replace('q:','♩ = '):'Tempo not set';$('instrument-names').textContent=doc.parts.map(p=>p.instrument).join(' / ');$('parts').replaceChildren(...doc.parts.map(p=>{const el=document.createElement('div');el.className='part-row';el.innerHTML='<span class="part-dot"></span><span></span><small></small>';el.children[1].textContent=p.instrument;el.children[2].textContent=`${p.staves} staff${p.staves===1?'':'s'}`;return el;}));}
  this.updateSelection();
 }
 select(id:string,locate=false){if(this.phase!=='ready'||!this.published)return;this.selected=this.published.events.find(e=>e.id===id)??null;if(locate&&this.selected?.source_line)this.editor.selectLine(this.selected.source_line);this.updateSelection();}
 private selectSource(line:number){if(this.phase!=='ready'||!this.published)return;const event=this.published.events.find(e=>e.source_line===line);if(event)this.select(event.id);}
 private updateSelection(){const e=this.selected;$('selection').hidden=!e;$('selection-empty').hidden=!!e;document.querySelectorAll('.selected-event').forEach(el=>el.classList.remove('selected-event'));if(!e)return;
  $('event-pitch').textContent=e.pitch;$('event-id').textContent=e.id;$('event-voice').textContent=e.voice;$('event-time').textContent=`${e.time} q`;$('event-duration').textContent=`${e.duration} q`;$('event-staff').textContent=String(e.staff);
  const pitch=$<HTMLInputElement>('pitch-input'),duration=$<HTMLInputElement>('duration-input');if(document.activeElement!==pitch)pitch.value=e.pitch;if(document.activeElement!==duration)duration.value=e.duration;const disabled=this.busy||this.dirty||this.phase!=='ready';pitch.disabled=disabled;duration.disabled=disabled;$<HTMLButtonElement>('set-pitch').disabled=disabled;$<HTMLButtonElement>('set-duration').disabled=disabled;$('mutation-hint').textContent=this.dirty?'Apply or discard the draft before editing notes.':'Changes are added to undo history.';
  for(const [svgId,eventId]of Object.entries(this.renderedMap))if(eventId===e.id)document.getElementById(svgId)?.classList.add('selected-event');
 }
 setLayout(mode:'split'|'score'){
  $('workspace').classList.toggle('score-only',mode==='score');
  $('view-split').setAttribute('aria-pressed',String(mode==='split'));
  $('view-score').setAttribute('aria-pressed',String(mode==='score'));
 }
 private measureWidth(){
  const canvas=$('canvas-scroll'),paper=$('paper');
  const canvasStyle=getComputedStyle(canvas),paperStyle=getComputedStyle(paper);
  const insets=(style:CSSStyleDeclaration)=>parseFloat(style.paddingLeft)+parseFloat(style.paddingRight)+parseFloat(style.borderLeftWidth)+parseFloat(style.borderRightWidth);
  return engravingWidth(canvas.clientWidth,insets(canvasStyle),insets(paperStyle));
 }
 private relayout(){
  const view=this.view;
  if(!view?.valid||view.source!==this.editor.value)return;
  // New layout receives a fresh sequence, so an old width cannot overwrite it.
  this.setZoom(this.zoom);const sequence=this.invalidate();void this.render(view,sequence);
 }
 private setZoom(value:number){
  this.zoom=clampZoom(value);
  const style=getComputedStyle($('paper'));
  const insets=parseFloat(style.paddingLeft)+parseFloat(style.paddingRight)+parseFloat(style.borderLeftWidth)+parseFloat(style.borderRightWidth);
  // Page padding stays fixed; compensate so 200% really doubles the notation.
  $('paper').style.width=`calc(${this.zoom*100}% - ${insets*(this.zoom-1)}px)`;
  $('zoom-level').textContent=`${Math.round(this.zoom*100)}%`;
  $<HTMLButtonElement>('zoom-out').disabled=this.zoom===MIN_ZOOM;
  $<HTMLButtonElement>('zoom-in').disabled=this.zoom===MAX_ZOOM;
 }
}
const workbench=new Workbench();void workbench.start();
if((window as unknown as {__SCORESTREAM_SMOKE__?:boolean}).__SCORESTREAM_SMOKE__)void import('./smoke').then(m=>m.run(workbench));
