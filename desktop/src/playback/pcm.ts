import type {PlaybackPlan} from './types';

/** A bounded output profile, not an audio renderer or a native file writer. */
export const PCM16_WAV = Object.freeze({
 sampleRate:48_000,
 channels:2,
 bitsPerSample:16,
 bytesPerFrame:4,
 headerBytes:44,
 maxScoreSeconds:120,
 // Reserved for the offline graph to drain after the score, not note time.
 tailSeconds:.25,
 maxVoices:4_096,
 maxExpressionPoints:32_768,
 maxChunkBytes:64*1024,
});

const tailFrames=PCM16_WAV.tailSeconds*PCM16_WAV.sampleRate;
const maxFrames=PCM16_WAV.maxScoreSeconds*PCM16_WAV.sampleRate+tailFrames;
const maxChunkFrames=PCM16_WAV.maxChunkBytes/PCM16_WAV.bytesPerFrame;

export interface Pcm16Layout {
 readonly frameCount:number;
 readonly dataBytes:number;
 readonly fileBytes:number;
}

function fail(message:string):never {throw new RangeError(`PCM16 WAV: ${message}`);}
function bounded(value:number,min:number,max:number,label:string):void {
 if(!Number.isFinite(value)||value<min||value>max)fail(`${label} must be finite and between ${min} and ${max}`);
}
function integer(value:number,min:number,max:number,label:string):void {
 if(!Number.isSafeInteger(value)||value<min||value>max)fail(`${label} must be an integer between ${min} and ${max}`);
}
function layoutForFrames(frameCount:number):Pcm16Layout {
 integer(frameCount,0,maxFrames,'frame count');
 const dataBytes=frameCount*PCM16_WAV.bytesPerFrame;
 const fileBytes=PCM16_WAV.headerBytes+dataBytes;
 // Keep the RIFF uint32 invariant explicit even if the smaller profile changes.
 if(!Number.isSafeInteger(fileBytes)||dataBytes>0xffffffff||fileBytes-8>0xffffffff)
  fail('size exceeds classic RIFF limits');
 return Object.freeze({frameCount,dataBytes,fileBytes});
}

/**
 * Validate only the audio-facing Rust DTO fields before creating an audio graph,
 * rendered channel buffers or PCM storage. This neither parses a score nor
 * changes its performance policy. Counts are TOTAL notes/points, not polyphony.
 * The caller must keep this same plan unchanged until rendering has finished.
 */
export function validatePcm16Plan(plan:Pick<PlaybackPlan,'duration_seconds'|'notes'>):Pcm16Layout {
 if(!plan||typeof plan!=='object')fail('plan is missing');
 const duration=plan.duration_seconds;
 bounded(duration,0,PCM16_WAV.maxScoreSeconds,'score duration');
 if(duration===0)fail('score duration must be positive');
 if(!Array.isArray(plan.notes))fail('notes must be an array');
 integer(plan.notes.length,0,PCM16_WAV.maxVoices,'voice count');

 // Check every work budget before iterating any expression points or allocating.
 let expressionPoints=0;
 for(const note of plan.notes){
  if(!note||typeof note!=='object'||!Array.isArray(note.gain_envelope))fail('note envelope must be an array');
  expressionPoints+=note.gain_envelope.length;
  integer(expressionPoints,0,PCM16_WAV.maxExpressionPoints,'expression point count');
 }

 let previousStart=0;
 // Rust computes duration as end-start. Accommodate only floating-point roundoff
 // when that duration is added back to start, never a full sample of extra time.
 const timeTolerance=4*Number.EPSILON*Math.max(1,duration);
 for(const note of plan.notes){
  for(const value of [note.id,note.part,note.voice,note.instrument])
   if(typeof value!=='string')fail('note identity and route must be strings');
  bounded(note.start_seconds,0,duration,'note start');
  bounded(note.duration_seconds,0,duration,'note duration');
  if(note.duration_seconds===0)fail('note duration must be positive');
  if(note.start_seconds<previousStart)fail('notes must be ordered by start time');
  previousStart=note.start_seconds;
  if(note.start_seconds+note.duration_seconds>duration+timeTolerance)fail('note ends after score duration');
  bounded(note.midi_note,0,127,'MIDI pitch'); // Fractional pitches remain valid.
  bounded(note.frequency_hz,0,PCM16_WAV.sampleRate/2,'note frequency');
  if(note.frequency_hz===0||note.frequency_hz===PCM16_WAV.sampleRate/2)fail('note frequency must be above zero and below Nyquist');
  integer(note.velocity,1,127,'note velocity');
  let previousOffset=0;
  for(const point of note.gain_envelope){
   if(!point||typeof point!=='object')fail('envelope point is missing');
   bounded(point.offset_seconds,0,note.duration_seconds+timeTolerance,'envelope offset');
   if(point.offset_seconds<previousOffset)fail('envelope points must be ordered by offset');
   previousOffset=point.offset_seconds;
   // Rust gains are relative to initial velocity (1..127), so they can exceed 1.
   bounded(point.gain,0,127,'envelope gain');
  }
 }
 return layoutForFrames(Math.ceil(duration*PCM16_WAV.sampleRate)+tailFrames);
}

/** Classic RIFF/WAVE, PCM format 1, 48 kHz stereo, signed 16-bit little-endian. */
export function createPcm16WavHeader(frameCount:number):Uint8Array {
 const layout=layoutForFrames(frameCount); // Validate before allocating bytes.
 const bytes=new Uint8Array(PCM16_WAV.headerBytes),view=new DataView(bytes.buffer);
 const tag=(offset:number,value:string)=>{
  for(let i=0;i<value.length;i++)view.setUint8(offset+i,value.charCodeAt(i));
 };
 tag(0,'RIFF');view.setUint32(4,layout.fileBytes-8,true);tag(8,'WAVE');
 tag(12,'fmt ');view.setUint32(16,16,true);view.setUint16(20,1,true);
 view.setUint16(22,PCM16_WAV.channels,true);view.setUint32(24,PCM16_WAV.sampleRate,true);
 view.setUint32(28,PCM16_WAV.sampleRate*PCM16_WAV.bytesPerFrame,true);
 view.setUint16(32,PCM16_WAV.bytesPerFrame,true);view.setUint16(34,PCM16_WAV.bitsPerSample,true);
 tag(36,'data');view.setUint32(40,layout.dataBytes,true);
 return bytes;
}

function quantize(sample:number):number {
 // Invalid rendered samples become silence rather than contaminating the file.
 if(!Number.isFinite(sample))return 0;
 const clipped=Math.max(-1,Math.min(1,sample));
 return Math.round(clipped*(clipped<0?32768:32767));
}

/**
 * Encode one explicit source range, L0 R0 L1 R1..., never more than 64 KiB.
 * Channels must be equally sized 48 kHz Float32 buffers within this profile.
 * No source mutation, whole-file buffer, I/O, channel mixing, resampling or
 * dither. Finite samples hard-clip to [-1,1]; non-finite samples become zero.
 * The caller owns ordered, non-overlapping ranges and backpressure to a writer.
 */
export function encodePcm16StereoChunk(left:Float32Array,right:Float32Array,startFrame:number,frameCount:number):Uint8Array {
 if(!(left instanceof Float32Array)||!(right instanceof Float32Array))fail('channels must be Float32Array buffers');
 integer(left.length,0,maxFrames,'channel frame count');
 if(left.length!==right.length)fail('channel lengths must match');
 integer(startFrame,0,left.length,'chunk start');
 integer(frameCount,0,maxChunkFrames,'chunk frame count');
 if(frameCount>left.length-startFrame)fail('chunk extends beyond the channels');
 const bytes=new Uint8Array(frameCount*PCM16_WAV.bytesPerFrame),view=new DataView(bytes.buffer);
 for(let frame=0;frame<frameCount;frame++){
  view.setInt16(frame*4,quantize(left[startFrame+frame]),true);
  view.setInt16(frame*4+2,quantize(right[startFrame+frame]),true);
 }
 return bytes;
}
