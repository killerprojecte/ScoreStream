import {describeError} from '../errors';
import {WebAudioOutput,type AudioOutput} from './audio';
import type {PlaybackPlan} from './types';

export interface PlaybackState {
 status:'stopped'|'loading'|'playing'|'paused'|'error';
 position:number; plan:PlaybackPlan|null; error:string;
}
export interface PlaybackBackend {plan(source:string):Promise<PlaybackPlan>}
const clamp=(value:number,duration:number)=>Math.min(duration,Math.max(0,Number.isFinite(value)?value:0));
/** Cancellable transport. Only the audio clock advances musical playback. */
export class PlaybackTransport {
 private state:PlaybackState={status:'stopped',position:0,plan:null,error:''};
 private listeners=new Set<()=>void>();
 private source='';private sourceVersion=0;private operation=0;
 private preparation:Promise<PlaybackPlan>|null=null;
 private timer:ReturnType<typeof setInterval>|undefined;
 private anchorTime=0;private anchorPosition=0;private nextNote=0;
 constructor(private backend:PlaybackBackend,private output:AudioOutput=new WebAudioOutput()){}
 subscribe=(listener:()=>void)=>{this.listeners.add(listener);return()=>this.listeners.delete(listener);};
 getSnapshot=()=>this.state;
 private set(patch:Partial<PlaybackState>){this.state={...this.state,...patch};for(const listener of this.listeners)listener();}
 setSource(source:string){
  if(this.source===source)return;
  this.source=source;this.sourceVersion++;this.preparation=null;this.cancel();
  this.set({status:'stopped',position:0,plan:null,error:''});
 }
 private cancel(){this.operation++;clearInterval(this.timer);this.timer=undefined;this.output.silence();}
 private position(){return this.state.status==='playing'?clamp(this.anchorPosition+this.output.now-this.anchorTime,this.state.plan!.duration_seconds):this.state.position;}
 private async prepare(){
  if(this.state.plan)return this.state.plan;
  if(this.preparation)return this.preparation;
  const version=this.sourceVersion;
  const pending=this.backend.plan(this.source).then(plan=>{
   if(version===this.sourceVersion)this.set({plan});
   return plan;
  }).finally(()=>{if(this.preparation===pending)this.preparation=null;});
  this.preparation=pending;return pending;
 }
 async prepareSource(){
  const version=this.sourceVersion;
  try{await this.prepare();}catch(error){if(version===this.sourceVersion&&this.state.status==='stopped')this.set({error:describeError(error)});}
 }
 async play(){
  if(this.state.status==='playing'||this.state.status==='loading')return;
  const operation=++this.operation;
  this.set({status:'loading',error:''});
  try{
   // Activate Web Audio before the Rust call crosses an asynchronous boundary.
   const resumed=this.output.resume();
   const [plan]=await Promise.all([this.prepare(),resumed]);
   if(operation!==this.operation)return;
   if(plan.duration_seconds<=0){this.set({status:'stopped',position:0});return;}
   this.begin(this.state.position>=plan.duration_seconds?0:this.state.position);
  }catch(error){if(operation===this.operation)this.fail(error);}
 }
 toggle(){if(this.state.status==='playing'||this.state.status==='loading')this.pause();else void this.play();}
 pause(){const position=this.position();this.cancel();this.set({status:'paused',position});}
 stop(){this.cancel();this.set({status:'stopped',position:0,error:''});}
 private begin(position:number){
  this.output.silence();clearInterval(this.timer);
  this.anchorTime=this.output.now;this.anchorPosition=position;this.nextNote=0;
  this.set({status:'playing',position,error:''});
  this.tick();
  if(this.state.status==='playing')this.timer=setInterval(()=>this.tick(),25);
 }
 private tick(){
  if(this.state.status!=='playing'||!this.state.plan)return;
  try{
   if(!this.output.running)throw new Error('Audio output was interrupted. Press Play to resume.');
   const plan=this.state.plan,position=this.position();
   if(position>=plan.duration_seconds){this.cancel();this.set({status:'stopped',position:plan.duration_seconds});return;}
   const until=position+.18;
   while(this.nextNote<plan.notes.length&&plan.notes[this.nextNote].start_seconds<=until){
    const note=plan.notes[this.nextNote++];
    if(note.start_seconds+note.duration_seconds<=position)continue;
    this.output.schedule(note,Math.max(0,position-note.start_seconds),this.output.now+Math.max(0,note.start_seconds-position));
   }
   this.set({position});
  }catch(error){this.fail(error);}
 }
 seek(seconds:number){
  if(!this.state.plan)return;
  const position=clamp(seconds,this.state.plan.duration_seconds),playing=this.state.status==='playing';
  this.cancel();
  if(playing&&position<this.state.plan.duration_seconds)this.begin(position);
  else this.set({position,status:position===0?'stopped':'paused',error:''});
 }
 async seekEvent(id:string){
  const version=this.sourceVersion,operation=++this.operation;
  if(this.state.status==='loading')this.set({status:'paused'});
  try{
   const plan=await this.prepare();
   if(version!==this.sourceVersion||operation!==this.operation)return;
   const event=plan.event_positions.find(event=>event.id===id);
   if(event)this.seek(event.start_seconds);
  }catch(error){if(version===this.sourceVersion&&operation===this.operation)this.fail(error);}
 }
 private fail(error:unknown){const position=this.position();this.cancel();this.set({status:'error',position,error:describeError(error)});}
 dispose(){this.sourceVersion++;this.preparation=null;this.cancel();this.output.dispose();this.listeners.clear();}
}
/** Let controls, editors and modal surfaces own their keyboard semantics. */
export function isPlaybackShortcut(event:KeyboardEvent):boolean {
 if(event.defaultPrevented||event.repeat||event.isComposing||event.key!==' '||event.ctrlKey||event.metaKey||event.altKey||event.shiftKey)return false;
 const target=event.target;
 return !(target instanceof Element&&target.closest('input,textarea,select,button,a,[contenteditable]:not([contenteditable="false"]),[role="textbox"],[role="combobox"],[role="slider"],[role="spinbutton"],[role="menu"],[role="menuitem"],[role="dialog"],[role="alertdialog"],.monaco-editor'));
}
