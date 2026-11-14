import type {PlaybackPlan} from './types';
import type {Pcm16Layout} from './pcm';
import {PCM16_WAV,validatePcm16Plan,createPcm16WavHeader,encodePcm16StereoChunk} from './pcm.ts';
import {createBuiltInMaster,scheduleBuiltInVoice} from './audio.ts';

/**
 * A transactional destination, not a file/IPC implementation. Each callback is
 * awaited before the next. Keep output private until complete succeeds; abort
 * must discard incomplete output. Do not retain all chunks in memory.
 */
export interface Pcm16WavSink {
 writeHeader(header:Uint8Array,layout:Pcm16Layout):Promise<void>;
 writeChunk(bytes:Uint8Array,startFrame:number,frameCount:number):Promise<void>;
 complete(layout:Pcm16Layout):Promise<void>;
 abort(reason:unknown):Promise<void>;
}

export interface OfflineWavOptions {
 readonly signal?:AbortSignal;
 /** Injection seam for hosts/tests; must return a fresh, unscheduled context. */
 readonly createContext?:(options:OfflineAudioContextOptions)=>OfflineAudioContext;
}

// One slot for this module, including callers using different injected factories.
// No queue: a second request is rejected rather than retaining another score.
let active=false;
function checkCancelled(signal:AbortSignal|undefined):void {
 if(signal?.aborted)throw new DOMException('WAV rendering was cancelled.','AbortError');
}
function checkSink(sink:Pcm16WavSink):void {
 if(!sink||['writeHeader','writeChunk','complete','abort'].some(key=>typeof sink[key as keyof Pcm16WavSink]!=='function'))
  throw new TypeError('WAV rendering requires header, chunk, complete and abort sink callbacks.');
}

/**
 * Render one complete, unchanged Rust plan, including leading rests and the
 * fixed PCM16 tail. Uses the live built-in voice/master graph, not samples.
 * Validation precedes every audio allocation; only one bounded offline buffer
 * and one <=64 KiB PCM chunk are produced at a time. The sink owns persistence.
 *
 * OfflineAudioContext cannot be closed/cancelled. An aborted request drains an
 * outstanding render or sink callback before aborting the sink and freeing the
 * slot. Cancellation is accepted until complete is entered; once finalization
 * starts its result wins, so a published file is never reported as cancelled.
 */
export async function renderPcm16Wav(plan:Pick<PlaybackPlan,'duration_seconds'|'notes'>,sink:Pcm16WavSink,options:OfflineWavOptions={}):Promise<Pcm16Layout> {
 if(active)throw new Error('A WAV render is already in progress.');
 const layout=validatePcm16Plan(plan);
 checkSink(sink);
 checkCancelled(options.signal);
 active=true;
 let master:DynamicsCompressorNode|null=null;
 const voices=new Set<()=>void>();
 const cleanup=()=>{
  // Best-effort cleanup must not replace a render/write error or strand the slot.
  for(const cancel of voices){try{cancel();}catch{/* Remaining nodes still need cleanup. */}}
  voices.clear();
  if(master){try{master.disconnect();}catch{/* Already disconnected. */}master=null;}
 };
 const signal=options.signal;
 try{
  signal?.addEventListener('abort',cleanup,{once:true});
  const context=(options.createContext??(settings=>new OfflineAudioContext(settings)))({
   numberOfChannels:PCM16_WAV.channels,length:layout.frameCount,sampleRate:PCM16_WAV.sampleRate,
  });
  checkCancelled(signal);
  master=createBuiltInMaster(context);
  // This loop is synchronous after validation: no caller can replace timing or
  // routes during an awaited setup step. Every planned note is scheduled once.
  for(const note of plan.notes){
   let cancel:(()=>void)|null=null;
   cancel=scheduleBuiltInVoice(context,master,note,0,note.start_seconds,()=>{if(cancel)voices.delete(cancel);});
   if(cancel)voices.add(cancel);
  }
  checkCancelled(signal);
  const buffer=await context.startRendering();
  checkCancelled(signal);
  cleanup();
  if(buffer.sampleRate!==PCM16_WAV.sampleRate||buffer.numberOfChannels!==PCM16_WAV.channels||buffer.length!==layout.frameCount)
   throw new Error('Offline audio buffer does not match the validated WAV layout.');
  // Borrow the two rendered channels, with no slice/copy/full PCM allocation.
  const left=buffer.getChannelData(0),right=buffer.getChannelData(1);
  if(!(left instanceof Float32Array)||!(right instanceof Float32Array)||left.length!==layout.frameCount||right.length!==layout.frameCount)
   throw new Error('Offline audio channels do not match the validated WAV layout.');
  checkCancelled(signal);
  await sink.writeHeader(createPcm16WavHeader(layout.frameCount),layout);
  checkCancelled(signal);
  const chunkFrames=PCM16_WAV.maxChunkBytes/PCM16_WAV.bytesPerFrame;
  for(let start=0;start<layout.frameCount;start+=chunkFrames){
   const count=Math.min(chunkFrames,layout.frameCount-start);
   await sink.writeChunk(encodePcm16StereoChunk(left,right,start,count),start,count);
   checkCancelled(signal);
  }
  // All frames have been acknowledged. From here, complete owns the commit.
  signal?.removeEventListener('abort',cleanup);
  await sink.complete(layout);
  return layout;
 }catch(error){
  cleanup();
  try{await sink.abort(error);}catch(abortError){
   throw new AggregateError([error,abortError],'WAV rendering failed and the sink could not discard its incomplete output.');
  }
  throw error;
 }finally{
  signal?.removeEventListener('abort',cleanup);
  cleanup();
  active=false;
 }
}
