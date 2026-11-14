// Standalone byte/contract tests. No Vite, browser, audio graph, native IPC or I/O.
import assert from 'node:assert/strict';
import {describe,it} from 'node:test';
import {PCM16_WAV,validatePcm16Plan,createPcm16WavHeader,encodePcm16StereoChunk} from '../src/playback/pcm.ts';

const note=(changes={})=>({
 id:'n',part:'p',instrument:'piano',voice:'v',start_seconds:0,duration_seconds:1,
 midi_note:69,frequency_hz:440,velocity:80,
 gain_envelope:[{offset_seconds:0,gain:1},{offset_seconds:1,gain:.5}],...changes,
});
const plan=(changes={})=>({duration_seconds:1,notes:[note()],...changes});
const maxFrames=(PCM16_WAV.maxScoreSeconds+PCM16_WAV.tailSeconds)*PCM16_WAV.sampleRate;
const maxChunkFrames=PCM16_WAV.maxChunkBytes/PCM16_WAV.bytesPerFrame;
const ascii=(bytes,start,end)=>String.fromCharCode(...bytes.slice(start,end));
const samples=bytes=>{
 const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
 return Array.from({length:bytes.length/2},(_,i)=>view.getInt16(i*2,true));
};
function withoutByteAllocation(action){
 const original=globalThis.Uint8Array;
 globalThis.Uint8Array=new Proxy(original,{construct(){assert.fail('allocated PCM bytes before validating');}});
 try{action();}finally{globalThis.Uint8Array=original;}
}

describe('bounded audio-facing plan validation',()=>{
 it('returns fixed, frozen size metadata without allocating audio or PCM buffers',()=>{
  withoutByteAllocation(()=>assert.deepEqual(validatePcm16Plan(plan()),{frameCount:60_000,dataBytes:240_000,fileBytes:240_044}));
  assert.ok(Object.isFrozen(PCM16_WAV));assert.ok(Object.isFrozen(validatePcm16Plan(plan())));
 });
 it('rounds score duration upward to a complete sample before adding exactly 250 ms',()=>{
  const result=validatePcm16Plan(plan({duration_seconds:.5/48_000,notes:[]}));
  assert.equal(result.frameCount,12_001);
  assert.equal(validatePcm16Plan(plan({duration_seconds:120,notes:[]})).frameCount,5_772_000);
  assert.equal(validatePcm16Plan(plan({duration_seconds:120,notes:[]})).fileBytes,23_088_044);
 });
 it('accepts rest-only positive plans, microtones, relative gains and repeated envelope offsets',()=>{
  assert.doesNotThrow(()=>validatePcm16Plan(plan({notes:[]})));
  assert.doesNotThrow(()=>validatePcm16Plan(plan({notes:[note({midi_note:69.5,frequency_hz:452.89,velocity:1,gain_envelope:[{offset_seconds:0,gain:0},{offset_seconds:0,gain:127}]})]})));
 });
 it('allows only arithmetic roundoff when Rust end-start is added back to start',()=>{
  const end=.009,start=.001;
  assert.ok(start+(end-start)>end);
  assert.doesNotThrow(()=>validatePcm16Plan(plan({duration_seconds:end,notes:[note({start_seconds:start,duration_seconds:end-start,gain_envelope:[]})]})));
  assert.throws(()=>validatePcm16Plan(plan({notes:[note({duration_seconds:1+1/48_000})]})),RangeError);
  assert.throws(()=>validatePcm16Plan(plan({notes:[note({start_seconds:.5,duration_seconds:.5+1e-8})]})),/ends after/);
 });
 it('rejects invalid, zero, overlong and overflow-scale durations',()=>{
  for(const duration_seconds of [NaN,Infinity,-Infinity,-1,0,120+1/48_000,Number.MAX_VALUE,'1',null])
   withoutByteAllocation(()=>assert.throws(()=>validatePcm16Plan(plan({duration_seconds})),RangeError));
  for(const value of [null,undefined,{},plan({notes:{length:1}}),plan({notes:[null]}),plan({notes:[note({gain_envelope:null})]})])
   assert.throws(()=>validatePcm16Plan(value),RangeError);
 });
 it('accepts exact total budgets, then rejects excessive voices before reading them',()=>{
  const envelope=Array.from({length:8},(_,i)=>({offset_seconds:i/8,gain:1}));
  const notes=Array.from({length:PCM16_WAV.maxVoices},()=>note({gain_envelope:envelope}));
  assert.doesNotThrow(()=>validatePcm16Plan(plan({notes})));
  const oversized=new Array(PCM16_WAV.maxVoices+1);
  Object.defineProperty(oversized,0,{get(){assert.fail('read over-budget notes');}});
  assert.throws(()=>validatePcm16Plan(plan({notes:oversized})),/voice count/);
 });
 it('checks total expression budget before examining any point',()=>{
  const excessive=new Array(PCM16_WAV.maxExpressionPoints+1);
  Object.defineProperty(excessive,0,{get(){assert.fail('read over-budget expression points');}});
  assert.throws(()=>validatePcm16Plan(plan({notes:[note({gain_envelope:excessive})]})),/expression point count/);
  const points=Array.from({length:PCM16_WAV.maxExpressionPoints},()=>({offset_seconds:0,gain:1}));
  assert.doesNotThrow(()=>validatePcm16Plan(plan({notes:[note({gain_envelope:points})]})));
  assert.throws(()=>validatePcm16Plan(plan({notes:[note({gain_envelope:points}),note({gain_envelope:[{offset_seconds:0,gain:1}]})]})),/expression point count/);
 });
 it('rejects non-finite or out-of-range note fields, while retaining Rust fractional pitch',()=>{
  const invalid={
   start_seconds:[NaN,Infinity,-Infinity,-1,2,'0'],
   duration_seconds:[NaN,Infinity,-Infinity,-1,0,2,'1'],
   midi_note:[NaN,Infinity,-Infinity,-1,128,'69'],
   frequency_hz:[NaN,Infinity,-Infinity,-1,0,24_000,48_000,'440'],
   velocity:[NaN,Infinity,-Infinity,-1,0,128,1.5,'80'],
  };
  for(const [field,values] of Object.entries(invalid))for(const value of values)
   assert.throws(()=>validatePcm16Plan(plan({notes:[note({[field]:value})]})),RangeError,`${field}: ${value}`);
  for(const field of ['id','part','voice','instrument'])
   assert.throws(()=>validatePcm16Plan(plan({notes:[note({[field]:null})]})),/strings/);
  assert.throws(()=>validatePcm16Plan(plan({notes:[note({start_seconds:.5,duration_seconds:.5,gain_envelope:[]}),note()]})),/ordered by start/);
 });
 it('rejects missing, unordered, non-finite and out-of-range envelope points',()=>{
  for(const value of [null,undefined,{offset_seconds:0,gain:NaN},{offset_seconds:Infinity,gain:1},{offset_seconds:0,gain:Infinity},{offset_seconds:-1,gain:1},{offset_seconds:2,gain:1},{offset_seconds:0,gain:-1},{offset_seconds:0,gain:128},{offset_seconds:'0',gain:1}])
   assert.throws(()=>validatePcm16Plan(plan({notes:[note({gain_envelope:[value]})]})),RangeError);
  assert.throws(()=>validatePcm16Plan(plan({notes:[note({gain_envelope:[{offset_seconds:.8,gain:1},{offset_seconds:.2,gain:1}]})]})),/ordered by offset/);
 });
 it('does not mutate or retain plan data in returned metadata',()=>{
  const input=plan(),before=structuredClone(input),layout=validatePcm16Plan(input);
  assert.deepEqual(input,before);input.duration_seconds=120;input.notes.length=0;
  assert.deepEqual(layout,{frameCount:60_000,dataBytes:240_000,fileBytes:240_044});
 });
});

describe('canonical PCM16 RIFF header',()=>{
 it('writes every fixed format field and little-endian size',()=>{
  const bytes=createPcm16WavHeader(48_000),view=new DataView(bytes.buffer);
  assert.equal(bytes.length,44);assert.equal(ascii(bytes,0,4),'RIFF');assert.equal(view.getUint32(4,true),192_036);
  assert.equal(ascii(bytes,8,12),'WAVE');assert.equal(ascii(bytes,12,16),'fmt ');assert.equal(view.getUint32(16,true),16);
  assert.equal(view.getUint16(20,true),1);assert.equal(view.getUint16(22,true),2);assert.equal(view.getUint32(24,true),48_000);
  assert.equal(view.getUint32(28,true),192_000);assert.equal(view.getUint16(32,true),4);assert.equal(view.getUint16(34,true),16);
  assert.equal(ascii(bytes,36,40),'data');assert.equal(view.getUint32(40,true),192_000);
  assert.deepEqual([...bytes.slice(24,28)],[0x80,0xbb,0,0]);
 });
 it('supports a valid empty RIFF and the exact maximum, without payload allocation',()=>{
  for(const frames of [0,maxFrames]){
   const bytes=createPcm16WavHeader(frames),view=new DataView(bytes.buffer);
   assert.equal(bytes.length,44);assert.equal(view.getUint32(40,true),frames*4);assert.equal(view.getUint32(4,true),frames*4+36);
  }
 });
 it('rejects bad counts, profile overflow and RIFF-scale overflow before allocation',()=>{
  for(const frames of [-1,.5,NaN,Infinity,-Infinity,maxFrames+1,2**30,Number.MAX_SAFE_INTEGER,Number.MAX_VALUE,'1'])
   withoutByteAllocation(()=>assert.throws(()=>createPcm16WavHeader(frames),RangeError));
 });
});

describe('bounded, interleaved stereo PCM16 encoding',()=>{
 it('encodes exact endpoints, clipping, rounded fractions and silence for non-finite values',()=>{
  const left=new Float32Array([-1,1,-.5,.5,-2,2,NaN,Infinity,-Infinity,-0]);
  const right=new Float32Array([1,-1,.5,-.5,2,-2,Infinity,NaN,-Infinity,0]);
  const bytes=encodePcm16StereoChunk(left,right,0,left.length);
  assert.deepEqual(samples(bytes),[-32768,32767,32767,-32768,-16384,16384,16384,-16384,-32768,32767,32767,-32768,0,0,0,0,0,0,0,0]);
  assert.deepEqual([...bytes.slice(0,8)],[0,0x80,0xff,0x7f,0xff,0x7f,0,0x80]);
 });
 it('honors view offsets and the selected source frame range without changing inputs',()=>{
  const left=new Float32Array([99,-1,0,1,99]).subarray(1,4),right=new Float32Array([99,1,0,-1,99]).subarray(1,4);
  const beforeLeft=left.slice(),beforeRight=right.slice();
  assert.deepEqual(samples(encodePcm16StereoChunk(left,right,1,2)),[0,0,32767,-32768]);
  assert.deepEqual(left,beforeLeft);assert.deepEqual(right,beforeRight);
 });
 it('encodes a multi-chunk stream in exact frame order including a partial last chunk',()=>{
  const count=2*maxChunkFrames+3,left=new Float32Array(count),right=new Float32Array(count);
  for(let i=0;i<count;i++){left[i]=i%2?-1:1;right[i]=i%3?0:-.5;}
  const chunks=[];
  for(let start=0;start<count;start+=maxChunkFrames)
   chunks.push(encodePcm16StereoChunk(left,right,start,Math.min(maxChunkFrames,count-start)));
  assert.deepEqual(chunks.map(chunk=>chunk.byteLength),[65_536,65_536,12]);
  const bytes=Buffer.concat([createPcm16WavHeader(count),...chunks]);
  assert.equal(bytes.byteLength,44+count*4);assert.equal(bytes.readUInt32LE(40),count*4);
  for(let i=0;i<count;i++){
   assert.equal(bytes.readInt16LE(44+i*4),i%2?-32768:32767);
   assert.equal(bytes.readInt16LE(46+i*4),i%3?0:-16384);
  }
 });
 it('allows empty channels/ranges and returns independently owned chunk buffers',()=>{
  const channel=new Float32Array([1]);
  assert.equal(encodePcm16StereoChunk(new Float32Array(),new Float32Array(),0,0).length,0);
  assert.equal(encodePcm16StereoChunk(channel,channel,1,0).length,0);
  const first=encodePcm16StereoChunk(channel,channel,0,1),second=encodePcm16StereoChunk(channel,channel,0,1);
  first.fill(0);assert.deepEqual(samples(second),[32767,32767]);
 });
 it('rejects invalid source types, unequal lengths and oversized source buffers',()=>{
  const channel=new Float32Array(1);
  for(const other of [null,[],new Float64Array(1),{length:1}])
   assert.throws(()=>encodePcm16StereoChunk(channel,other,0,1),RangeError);
  assert.throws(()=>encodePcm16StereoChunk(channel,new Float32Array(2),0,1),/lengths must match/);
  const excessive=new Float32Array(maxFrames+1);
  withoutByteAllocation(()=>assert.throws(()=>encodePcm16StereoChunk(excessive,excessive,0,1),/channel frame count/));
 });
 it('rejects overflow, out-of-bounds and non-integer ranges before allocating',()=>{
  const channel=new Float32Array(maxChunkFrames+1);
  for(const start of [-1,.5,NaN,Infinity,Number.MAX_SAFE_INTEGER,'0'])
   withoutByteAllocation(()=>assert.throws(()=>encodePcm16StereoChunk(channel,channel,start,1),RangeError));
  for(const count of [-1,.5,NaN,Infinity,Number.MAX_SAFE_INTEGER,maxChunkFrames+1,'1'])
   withoutByteAllocation(()=>assert.throws(()=>encodePcm16StereoChunk(channel,channel,0,count),RangeError));
  withoutByteAllocation(()=>assert.throws(()=>encodePcm16StereoChunk(channel,channel,channel.length,1),/beyond/));
 });
});
