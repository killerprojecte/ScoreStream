import type {EnvelopePoint, PlaybackNote} from './types';
import {applyBuiltInTimbre} from './timbres.ts';

export interface AudioOutput {
 readonly now:number;
 readonly running:boolean;
 resume():Promise<void>;
 schedule(note:PlaybackNote, offset:number, when:number):void;
 silence():void;
 dispose():void;
}
/** Piecewise-linear Rust envelopes; equal-time anchors encode a jump. */
export function gainAt(points:EnvelopePoint[], at:number):number {
 let previous={offset_seconds:0,gain:1};
 for(const point of points){
  if(point.offset_seconds>at){
   const span=point.offset_seconds-previous.offset_seconds;
   return span>0?previous.gain+(point.gain-previous.gain)*(at-previous.offset_seconds)/span:point.gain;
  }
  previous=point;
 }
 return previous.gain;
}
function scheduleEnvelope(parameter:AudioParam,points:EnvelopePoint[],level:number,offset:number,duration:number,when:number,completeRamp=false){
 parameter.setValueAtTime(level*gainAt(points,offset),when);
 let previousTime:number|undefined;
 for(const point of points){
  if(point.offset_seconds>offset&&point.offset_seconds<=duration){
   const time=when+point.offset_seconds-offset;
   // Ordered equal-time anchors are an explicit jump, not a second ramp.
   if(previousTime===point.offset_seconds)parameter.setValueAtTime(level*point.gain,time);
   else parameter.linearRampToValueAtTime(level*point.gain,time);
  }
  previousTime=point.offset_seconds;
 }
 // Preserve interpolation when a note ends between two envelope anchors.
 if(completeRamp&&points.some(point=>point.offset_seconds>duration))
  parameter.linearRampToValueAtTime(level*gainAt(points,duration),when+duration-offset);
}
/** Shared real-time/offline voice graph. Rust owns route, pitch and note timing. */
export function scheduleBuiltInVoice(context:BaseAudioContext,destination:AudioNode,note:PlaybackNote,offset:number,when:number,onEnded:()=>void=()=>{}):(()=>void)|null {
 const remaining=note.duration_seconds-offset;
 if(remaining<=0)return null;
 const oscillator=context.createOscillator(),expression=context.createGain(),gate=context.createGain(),color=context.createGain();
 let disconnected=false;
 const disconnect=()=>{
  if(disconnected)return;
  disconnected=true;oscillator.onended=null;
  oscillator.disconnect();expression.disconnect();gate.disconnect();color.disconnect();onEnded();
 };
 const cancel=()=>{
  if(disconnected)return;
  gate.gain.cancelScheduledValues(context.currentTime);gate.gain.setValueAtTime(0,context.currentTime);
  try{oscillator.stop();}catch{/* Already ended or not started. */}
  disconnect();
 };
 try{
  const envelope=applyBuiltInTimbre(context,oscillator,note.instrument);
  oscillator.frequency.setValueAtTime(note.frequency_hz,when);
  scheduleEnvelope(expression.gain,note.gain_envelope,.12*note.velocity/127,offset,note.duration_seconds,when);
  scheduleEnvelope(color.gain,envelope,1,offset,note.duration_seconds,when,true);
  const attack=Math.min(.008,remaining/3),release=Math.min(.012,remaining/3);
  gate.gain.setValueAtTime(0,when);
  gate.gain.linearRampToValueAtTime(1,when+attack);
  gate.gain.setValueAtTime(1,when+remaining-release);
  gate.gain.linearRampToValueAtTime(0,when+remaining);
  oscillator.connect(color);color.connect(expression);expression.connect(gate);gate.connect(destination);
  oscillator.onended=disconnect;oscillator.start(when);oscillator.stop(when+remaining);
 }catch(error){cancel();throw error;}
 return cancel;
}
/** Both outputs use the same compressor with unchanged Web Audio defaults. */
export function createBuiltInMaster(context:BaseAudioContext):DynamicsCompressorNode {
 const master=context.createDynamicsCompressor();
 master.connect(context.destination);
 return master;
}
/** Built-in synthesized tones, with all performance timing supplied by Rust. */
export class WebAudioOutput implements AudioOutput {
 private context:AudioContext|null=null;
 private master:DynamicsCompressorNode|null=null;
 private voices=new Set<()=>void>();
 get now(){return this.context?.currentTime??0;}
 get running(){return this.context?.state==='running';}
 async resume(){
  if(!this.context||this.context.state==='closed'){
   this.context=new AudioContext();
   this.master=createBuiltInMaster(this.context);
  }
  // Called synchronously from the user's Play gesture, before requesting a plan.
  await this.context.resume();
  if(this.context.state!=='running')throw new Error('Audio output could not start. Try Play again.');
 }
 schedule(note:PlaybackNote,offset:number,when:number){
  const context=this.context;
  if(!context||!this.master)throw new Error('Audio output is unavailable.');
  let cancel:(()=>void)|null=null;
  cancel=scheduleBuiltInVoice(context,this.master,note,offset,when,()=>{if(cancel)this.voices.delete(cancel);});
  if(cancel)this.voices.add(cancel);
 }
 silence(){for(const cancel of [...this.voices])cancel();}
 dispose(){this.silence();const context=this.context;this.context=null;this.master=null;if(context&&context.state!=='closed')void context.close().catch(()=>{});}
}
