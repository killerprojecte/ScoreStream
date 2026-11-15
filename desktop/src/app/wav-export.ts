import type {PlaybackPlan} from '../playback/types';
import type {Pcm16Layout} from '../playback/pcm';
import {PCM16_WAV,validatePcm16Plan} from '../playback/pcm.ts';
import {renderPcm16Wav} from '../playback/offline.ts';

/** Native owns the chooser, destination and session. No path is accepted for writing. */
export interface WavExportSession extends Pcm16Layout {
 readonly sessionId:string;
 readonly path:string;
 readonly plan:PlaybackPlan;
}
export interface WavExportBridge {
 begin(source:string):Promise<WavExportSession|null>;
 write(sessionId:string,offset:number,bytes:Uint8Array):Promise<number>;
 finish(sessionId:string):Promise<string>;
 abort(sessionId:string):Promise<void>;
}
export type WavExportPhase='idle'|'choosing'|'rendering'|'writing'|'finishing'|'cancelling'|'succeeded'|'failed';
export interface WavExportState {
 readonly phase:WavExportPhase;
 readonly fileName:string;
 readonly writtenBytes:number;
 readonly totalBytes:number;
 readonly error:string|null;
}
const initial=():WavExportState=>({phase:'idle',fileName:'',writtenBytes:0,totalBytes:0,error:null});
const basename=(path:string)=>path.split(/[\\/]/).pop()||'WAV';
const sameLayout=(a:Pcm16Layout,b:Pcm16Layout)=>a.frameCount===b.frameCount&&a.dataBytes===b.dataBytes&&a.fileBytes===b.fileBytes;
const cancelled=()=>new DOMException('WAV export was cancelled.','AbortError');
const failures={
 choosing:'Could not start WAV export. Check the source diagnostics and the 120-second limit, then choose a new filename in a writable folder.',
 rendering:'Could not render WAV audio. Try a shorter score and make sure this desktop WebView supports offline audio.',
 writing:'Could not write the WAV file. Check free disk space and folder permissions, then try again with a new filename.',
 finishing:'Could not finish the WAV file. Choose a new filename; existing files are never replaced.',
 cleanup:'Could not discard the incomplete WAV export. Restart the desktop app before trying again.',
 layout:'The native WAV plan and output size do not match. Restart the desktop app and try again.',
};

const nativeErrors:Record<string,string>={
 WAV_CLEANUP_FAILED:failures.cleanup,
 WAV_CHOOSER_FAILED:'Could not open or complete the WAV save dialog. Try again, or restart the desktop app if the problem continues.',
 WAV_SOURCE_INVALID:'WAV export needs a valid score. Fix the source diagnostics and try again.',
 WAV_LIMIT_EXCEEDED:'This score exceeds the WAV export limits (120 seconds, 4,096 notes, or 32,768 expression points). Shorten or simplify it and try again.',
 WAV_DESTINATION_EXISTS:'A file already exists at this destination. Export again with a new filename; existing files are never replaced.',
 WAV_BUSY:'Another WAV export is active. Finish or cancel it before trying again.',
 WAV_BAD_REQUEST:'The WAV export request was rejected. Restart the desktop app and try again.',
 WAV_SESSION_NOT_FOUND:'The WAV export session has ended. Export again with a new filename.',
};
function nativeErrorCode(error:unknown):string|null {
 if(!error||typeof error!=='object'||!('code' in error)||typeof error.code!=='string')return null;
 return error.code;
}

/** Independent of editor transactions: captured source is never applied or saved. */
export class WavExportController {
 private state=initial();
 private listeners=new Set<()=>void>();
 private operation:AbortController|null=null;
 private disposed=false;
 private finalizing=false;
 private api:WavExportBridge;
 private render:typeof renderPcm16Wav;
 constructor(api:WavExportBridge,render:typeof renderPcm16Wav=renderPcm16Wav){this.api=api;this.render=render;}
 subscribe=(listener:()=>void)=>{this.listeners.add(listener);return()=>this.listeners.delete(listener);};
 getSnapshot=()=>this.state;
 get busy(){return this.operation!==null;}
 private set(patch:Partial<WavExportState>){if(this.disposed)return;this.state={...this.state,...patch};for(const listener of this.listeners)listener();}
 dismiss(){if(!this.busy)this.set(initial());}
 cancel(){if(!this.operation||this.finalizing)return;this.operation.abort();this.set({phase:'cancelling'});}
 dispose(){this.cancel();this.disposed=true;this.listeners.clear();}
 async start(source:string):Promise<void> {
  if(this.busy||this.disposed)return;
  const operation=new AbortController();
  this.operation=operation;
  this.finalizing=false;
  let session:WavExportSession|null=null,abortAttempted=false,cleanupFailed=false,finished=false;
  let stage:'choosing'|'rendering'|'writing'|'finishing'='choosing';
  let errorMessage:string|undefined;
  const checkCancelled=()=>{if(operation.signal.aborted)throw cancelled();};
  const abortSession=async()=>{
   if(!session||finished||abortAttempted)return;
   abortAttempted=true;
   try{await this.api.abort(session.sessionId);}catch{cleanupFailed=true;throw new Error(failures.cleanup);}
  };
  this.set({...initial(),phase:'choosing'});
  try{
   // This immutable string is the click-time source, not a later editor snapshot.
   session=await this.api.begin(source);
   if(!session){this.set(initial());return;}
   checkCancelled();
   const layout=validatePcm16Plan(session.plan);
   if(!sameLayout(layout,session)){errorMessage=failures.layout;throw new Error(errorMessage);}
   this.set({fileName:basename(session.path),totalBytes:layout.fileBytes,phase:'rendering'});
   stage='rendering';
   let offset=0;
   const write=async(bytes:Uint8Array)=>{
    checkCancelled();
    const next=offset+bytes.byteLength;
    if(!(bytes instanceof Uint8Array)||bytes.byteLength>PCM16_WAV.maxChunkBytes||next>layout.fileBytes)throw new Error('Invalid WAV chunk.');
    stage='writing';
    this.set({phase:'writing'});
    // No Array.from, base64, JSON byte array or full-file buffer. Backpressure
    // is end-to-end: do not request the next chunk until native acknowledges it.
    const accepted=await this.api.write(session!.sessionId,offset,bytes);
    if(accepted!==next)throw new Error('Native WAV byte acknowledgement does not match.');
    offset=next;
    this.set({writtenBytes:offset});
   };
   await this.render(session.plan,{
    writeHeader:async(bytes,headerLayout)=>{
     if(offset!==0||bytes.byteLength!==PCM16_WAV.headerBytes||!sameLayout(layout,headerLayout))throw new Error('Invalid WAV header layout.');
     await write(bytes);
    },
    writeChunk:async(bytes,startFrame,frameCount)=>{
     if(offset!==PCM16_WAV.headerBytes+startFrame*PCM16_WAV.bytesPerFrame||bytes.byteLength!==frameCount*PCM16_WAV.bytesPerFrame)throw new Error('Invalid WAV chunk order.');
     await write(bytes);
    },
    complete:async(completedLayout)=>{
     checkCancelled();
     if(offset!==layout.fileBytes||!sameLayout(layout,completedLayout))throw new Error('Incomplete WAV output.');
     stage='finishing';this.finalizing=true;this.set({phase:'finishing'});
     const path=await this.api.finish(session!.sessionId);
     finished=true;
     this.set({fileName:basename(path)});
    },
    abort:abortSession,
   },{signal:operation.signal});
   if(!finished)throw new Error('WAV renderer did not finalize its output.');
   this.set({phase:'succeeded'});
  }catch(error){
   const code=nativeErrorCode(error);
   const safeMessage=code&&Object.hasOwn(nativeErrors,code)?nativeErrors[code]:undefined;
   try{await abortSession();}catch{/* Preserve a visible cleanup failure even after cancellation. */}
   // Native may already have removed the session before failing to delete its
   // temporary file. A subsequent no-op abort cannot erase that cleanup error.
   if(cleanupFailed||code==='WAV_CLEANUP_FAILED')this.set({phase:'failed',error:failures.cleanup});
   else if((operation.signal.aborted&&!this.finalizing)||code==='WAV_CANCELLED')this.set(initial());
   else this.set({phase:'failed',error:safeMessage??errorMessage??failures[stage]});
  }finally{
   this.operation=null;
   this.finalizing=false;
   // Publish after releasing the guard so subscribers can safely start again.
   if(!this.disposed)this.set({});
  }
 }
}
