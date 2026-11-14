import type {EnvelopePoint, PlaybackNote} from './types';

export interface AudioOutput {
 readonly now:number;
 readonly running:boolean;
 resume():Promise<void>;
 schedule(note:PlaybackNote, offset:number, when:number):void;
 silence():void;
 dispose():void;
}
/** Rust envelopes are linear in seconds, relative to the initial velocity. */
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
   this.master=this.context.createDynamicsCompressor();
   this.master.connect(this.context.destination);
  }
  // Called synchronously from the user's Play gesture, before requesting a plan.
  await this.context.resume();
  if(this.context.state!=='running')throw new Error('Audio output could not start. Try Play again.');
 }
 schedule(note:PlaybackNote,offset:number,when:number){
  const context=this.context;
  if(!context||!this.master)throw new Error('Audio output is unavailable.');
  const remaining=note.duration_seconds-offset;
  if(remaining<=0)return;
  const oscillator=context.createOscillator(),expression=context.createGain(),gate=context.createGain();
  // A deliberately modest built-in timbre, not a sampled instrument claim.
  oscillator.type='triangle';
  oscillator.frequency.setValueAtTime(note.frequency_hz,when);
  const level=.12*note.velocity/127;
  expression.gain.setValueAtTime(level*gainAt(note.gain_envelope,offset),when);
  for(const point of note.gain_envelope){
   if(point.offset_seconds>offset&&point.offset_seconds<=note.duration_seconds)
    expression.gain.linearRampToValueAtTime(level*point.gain,when+point.offset_seconds-offset);
  }
  const attack=Math.min(.008,remaining/3),release=Math.min(.012,remaining/3);
  gate.gain.setValueAtTime(0,when);
  gate.gain.linearRampToValueAtTime(1,when+attack);
  gate.gain.setValueAtTime(1,when+remaining-release);
  gate.gain.linearRampToValueAtTime(0,when+remaining);
  oscillator.connect(expression);expression.connect(gate);gate.connect(this.master);
  const disconnect=()=>{oscillator.disconnect();expression.disconnect();gate.disconnect();this.voices.delete(cancel);};
  const cancel=()=>{gate.gain.cancelScheduledValues(context.currentTime);gate.gain.setValueAtTime(0,context.currentTime);try{oscillator.stop();}catch{/* Already ended. */}disconnect();};
  this.voices.add(cancel);oscillator.onended=disconnect;
  try{oscillator.start(when);oscillator.stop(when+remaining);}catch(error){cancel();throw error;}
 }
 silence(){for(const cancel of [...this.voices])cancel();}
 dispose(){this.silence();const context=this.context;this.context=null;this.master=null;if(context&&context.state!=='closed')void context.close().catch(()=>{});}
}
