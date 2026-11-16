import {PlaybackTransport} from '../playback/transport';
import {WavExportController} from './wav-export';
import { backend, native, playbackBackend, wavExportBackend } from '../bridge';
import { clampZoom } from '../layout';
import { project, RENDERER } from '../render/project';
import { LatestOnly } from '../render/scheduler';
import { describeError } from '../errors';
import type { ImportDiagnostic } from '../bridge';
import type { ConfirmUnsaved } from './confirmation';
import type { SourceEditor } from '../editor/source';
import type { View, ScoreEvent, RenderResult, PreviewKey } from '../types';

export type Phase = 'loading'|'pending'|'rendering'|'invalid'|'unsupported'|'ready';
export interface WorkbenchState {
 current:View|null; view:View|null; published:View|null; selected:ScoreEvent|null;
 source:string; fileName:string; fileSource:string|null; initialSource:string;
 sequence:number; renderedSequence:number; phase:Phase; busy:boolean; zoom:number;
 pages:string[]; renderedMap:Record<string,string>; elapsed:number; warnings:string[];
 importReport:ImportDiagnostic[]; importedUnsaved:boolean;
 message:string; messageValues:Record<string,string|number>;
}
const initial=():WorkbenchState=>({current:null,view:null,published:null,selected:null,source:'',fileName:'First light.sfs',fileSource:null,initialSource:'',sequence:0,renderedSequence:0,phase:'loading',busy:false,zoom:1,pages:[],renderedMap:{},elapsed:0,warnings:[],importReport:[],importedUnsaved:false,message:'',messageValues:{}});
/** Rust remains the only score authority. This controller owns async sequencing;
 * React owns every shell node. Monaco and the SVG renderer receive narrow refs. */
export class Workbench {
 private draftOrigin:Pick<WorkbenchState,'fileName'|'fileSource'|'initialSource'|'importReport'|'importedUnsaved'>|null=null;
 private state=initial(); private listeners=new Set<()=>void>();
 private timer:ReturnType<typeof setTimeout>|undefined;private width=600;
 private snapshots=new Map<number,View>();private session=crypto.randomUUID();
 private scheduler:LatestOnly|null=null;private worker:Worker|null=null;
 private confirmUnsaved:ConfirmUnsaved=async()=>false;private openPending=false;private lifecycle=0;
 editor:SourceEditor|null=null;
 readonly playback=new PlaybackTransport(playbackBackend);
 readonly wavExport=new WavExportController(wavExportBackend);
 constructor(private api=backend,private isNative=native){}
 subscribe=(listener:()=>void)=>{this.listeners.add(listener);return()=>this.listeners.delete(listener);};
 getSnapshot=()=>this.state;
 private set(patch:Partial<WorkbenchState>){this.state={...this.state,...patch};for(const listener of this.listeners)listener();}
 get current(){return this.state.current;} get view(){return this.state.view;} get published(){return this.state.published;} get selected(){return this.state.selected;}
 get sequence(){return this.state.sequence;} get renderedSequence(){return this.state.renderedSequence;} get phase(){return this.state.phase;} get busy(){return this.state.busy;} get zoom(){return this.state.zoom;}
 get dirty(){return !!this.current&&this.state.source!==this.current.source;}
 get unsaved(){return this.state.importedUnsaved||this.state.source!==(this.state.fileSource??this.state.initialSource);}
 get canExportWav(){return this.isNative&&!this.busy&&!this.wavExport.busy&&!!this.state.source.trim();}
 exportWav(){if(this.canExportWav)return this.wavExport.start(this.state.source);return Promise.resolve();}
 get canSave(){return !this.busy&&!!this.view?.valid&&this.view.source===this.state.source;}
 attachEditor(editor:SourceEditor){this.editor=editor;if(this.state.source)editor.value=this.state.source;return()=>{if(this.editor===editor)this.editor=null;};}
 attachRenderer(worker:Worker){this.worker=worker;this.scheduler=new LatestOnly(worker,result=>this.publish(result));worker.onerror=e=>this.previewError(`Renderer failed: ${e.message}`);}
 bindConfirmation(confirm:ConfirmUnsaved){this.confirmUnsaved=confirm;return()=>{if(this.confirmUnsaved===confirm){this.confirmUnsaved=async()=>false;this.lifecycle++;}};}
 dispose(){this.wavExport.dispose();this.playback.dispose();this.lifecycle++;clearTimeout(this.timer);this.worker?.terminate();this.worker=null;this.scheduler=null;this.listeners.clear();}
 async start(){if(!this.isNative){this.previewError('Open the native desktop app. This UI intentionally has no JavaScript parser or mock Rust backend.');return;}try{const view=await this.api.current();this.set({initialSource:view.source});this.accept(view);}catch(e){this.previewError(describeError(e));}}
 message(message:string,messageValues:Record<string,string|number>={}){this.set({message,messageValues});}
 private invalidate(){clearTimeout(this.timer);const sequence=this.sequence+1;this.scheduler?.invalidate(sequence);this.snapshots.clear();this.set({sequence,phase:'pending'});return sequence;}
 changed(source=this.editor?.value??this.state.source){this.playback.setSource(source);this.set({source,view:null,selected:null});const sequence=this.invalidate();this.timer=setTimeout(()=>void this.preview(sequence),280);}
 async preview(sequence=this.sequence){const source=this.state.source;try{const view=await this.api.preview(source);if(sequence!==this.sequence||source!==this.state.source)return;this.set({view});this.editor?.diagnostics(view.diagnostics);if(!view.valid){this.set({phase:'invalid'});return;}if(this.isNative)void this.playback.prepareSource();await this.render(view,sequence);}catch(e){if(sequence===this.sequence)this.previewError(describeError(e));}}
 private async render(view:View,sequence:number){try{
  const projected=project(view);const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(view.source)))).map(x=>x.toString(16).padStart(2,'0')).join('');if(sequence!==this.sequence)return;
  const layout=`${this.width}:${RENDERER}`;const key:PreviewKey=view.dirty?{kind:'draft',session:this.session,sequence,hash,baseRevision:this.current!.revision_id,layout}:{kind:'committed',revision:view.revision_id,layout};
  this.snapshots.set(sequence,view);this.set({phase:'rendering',warnings:projected.warnings});this.scheduler?.submit({requestId:sequence,key,mei:projected.mei,width:this.width,map:projected.map});
 }catch(e){if(sequence===this.sequence)this.previewError(describeError(e));}}
 private publish(result:RenderResult){if(result.requestId!==this.sequence)return;if(result.error){this.previewError(result.error);return;}const view=this.snapshots.get(result.requestId);if(!view||view.source!==this.state.source)return;this.set({published:view,renderedMap:result.map,renderedSequence:result.requestId,phase:'ready',pages:result.pages,elapsed:result.elapsed});}
 private previewError(message:string){this.set({phase:'unsupported',message});}
 private accept(view:View,selectedId?:string){this.playback.setSource(view.source);if(this.isNative)void this.playback.prepareSource();this.editor&&(this.editor.value=view.source);this.editor?.diagnostics(view.diagnostics);this.set({current:view,view,source:view.source,selected:view.events.find(e=>e.id===selectedId)??null});const sequence=this.invalidate();void this.render(view,sequence);}
 async apply(){if(this.busy||!this.dirty||!this.canSave)return;await this.transaction(async()=>{this.accept(await this.api.apply(this.state.source,this.current!.revision_id));this.draftOrigin=null;this.message('Draft applied. A new committed revision is ready.');});}
 async discard(){if(!this.current||this.busy)return;await this.transaction(async()=>{await this.api.preview(this.current!.source);this.accept(this.current!);if(this.draftOrigin){this.set(this.draftOrigin);this.draftOrigin=null;}this.message('Draft discarded. Showing the committed score.');});}
 async history(direction:'undo'|'redo'){if(this.dirty||this.busy)return;await this.transaction(async()=>{this.accept(await this.api[direction](),this.selected?.id);this.message(direction==='undo'?'Edit undone.':'Edit restored.');});}
 private async transaction(action:()=>Promise<void>,rethrow=false){this.set({busy:true});if(this.editor)this.editor.readOnly=true;try{await action();}catch(e){this.message(describeError(e));if(rethrow)throw e;}finally{this.set({busy:false});if(this.editor)this.editor.readOnly=false;}}
 async mutate(kind:'pitch'|'duration',value:string){if(!this.selected||this.dirty||this.busy||this.phase!=='ready')return;const id=this.selected.id;if(value===this.selected[kind])return;await this.transaction(async()=>{const view=kind==='pitch'?await this.api.setPitch(id,value,this.current!.revision_id):await this.api.setDuration(id,value,this.current!.revision_id);this.accept(view,id);this.message('Note updated.');},true);}
 async open(){
  if(this.busy||this.openPending)return;
  this.openPending=true;
  const lifecycle=this.lifecycle;
  try{
   if((this.dirty||this.unsaved)&&!await this.confirmUnsaved('replace'))return;
   if(lifecycle!==this.lifecycle||this.busy)return;
   await this.transaction(async()=>{
    const file=await this.api.openFile();
    if(!file||lifecycle!==this.lifecycle)return;
    if(file.imported_view){
     if(!file.import_token)throw new Error("The import is no longer available. Open the file again.");
     const imported=await this.api.activateImport(file.import_token,this.current!.revision_id);
     if(lifecycle!==this.lifecycle)return;
     this.session=crypto.randomUUID();this.draftOrigin=null;
    this.set({fileName:file.path.split(/[\\/]/).pop()!.replace(/\.(musicxml|xml|mxl)$/i,'.sfs'),fileSource:null,initialSource:file.source,importedUnsaved:true,importReport:file.import_report?.diagnostics??[],published:null,pages:[],renderedMap:{},renderedSequence:0,warnings:[],selected:null});
     this.accept(imported);
     this.message('Imported as an unsaved SFS document. The original file is unchanged.');
     return;
    }
    this.draftOrigin??={fileName:this.state.fileName,fileSource:this.state.fileSource,initialSource:this.state.initialSource,importReport:this.state.importReport,importedUnsaved:this.state.importedUnsaved};
    this.set({fileName:file.path.split(/[\\/]/).pop()!,fileSource:file.source,importedUnsaved:false,importReport:[]});
    if(this.editor)this.editor.value=file.source;
    this.changed(file.source);
    this.message('File opened as a draft. Review and apply it to the current session.');
   });
  }catch(e){if(lifecycle===this.lifecycle)this.message(describeError(e));}
  finally{this.openPending=false;}
 }
 async save(){if(!this.canSave){this.message('Wait for Rust validation before saving.');return;}await this.transaction(async()=>{const source=this.state.source,path=await this.api.saveFile(source);if(path){const fileName=path.split(/[\\/]/).pop()!;this.set({fileName,fileSource:source,importedUnsaved:false});this.message('Saved {name}. Session history is not persisted.',{name:fileName});}});}
 select(id:string,locate=false){if(this.phase!=='ready'||!this.published)return;const selected=this.published.events.find(e=>e.id===id)??null;this.set({selected});if(locate&&selected?.source_line)this.editor?.selectLine(selected.source_line);}
 selectSource(line:number){if(this.phase!=='ready'||!this.published)return;this.select(this.published.events.find(e=>e.source_line===line)?.id??'');}
 reveal(){if(this.selected?.source_line)this.editor?.selectLine(this.selected.source_line);this.editor?.focus();}
 setZoom(value:number){this.set({zoom:clampZoom(value)});}
 setWidth(width:number){if(width===this.width)return;this.width=width;const view=this.view;if(view?.valid&&view.source===this.state.source){const sequence=this.invalidate();void this.render(view,sequence);}}
}
