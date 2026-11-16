// Fake Web Audio/sink orchestration only: no real signal, device, file, IPC or browser.
import assert from 'node:assert/strict';
import {describe,it} from 'node:test';
import {renderPcm16Wav} from '../src/playback/offline.ts';
import {WebAudioOutput} from '../src/playback/audio.ts';
import {INSTRUMENT_CATALOG} from '../src/instruments/catalog.ts';
import {validatePcm16Plan} from '../src/playback/pcm.ts';
const note=(changes={})=>({id:'n',part:'p',voice:'v',instrument:'piano',start_seconds:.25,duration_seconds:.5,midi_note:69,frequency_hz:440,velocity:80,gain_envelope:[],...changes});
const plan=(changes={})=>({duration_seconds:1,notes:[note()],...changes});
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};};
const parameter=()=>({events:[],setValueAtTime(...args){this.events.push(['set',...args]);},linearRampToValueAtTime(...args){this.events.push(['ramp',...args]);},cancelScheduledValues(...args){this.events.push(['cancel',...args]);}});
const node=()=>({connections:[],disconnects:0,connect(target){this.connections.push(target);},disconnect(){this.disconnects++;}});
class FakeContext {
 constructor(options){this.options=options;this.currentTime=0;this.state='suspended';this.destination={};this.oscillators=[];this.gains=[];this.waves=[];this.compressorCalls=0;this.renderCalls=0;this.channelReads=[];}
 createDynamicsCompressor(...args){this.compressorCalls++;this.compressorArgs=args;return this.master=node();}
 createPeriodicWave(real,imag,options){const wave={real:[...real],imag:[...imag],options};this.waves.push(wave);return wave;}
 createOscillator(){const oscillator={...node(),type:'sine',frequency:parameter(),starts:[],stops:[],onended:null,start(when){this.starts.push(when);},stop(when){this.stops.push(when);},setPeriodicWave(wave){this.wave=wave;}};this.oscillators.push(oscillator);return oscillator;}
 createGain(){const gain={...node(),gain:parameter()};this.gains.push(gain);return gain;}
 makeBuffer(){this.channels=[new Float32Array(this.options.length),new Float32Array(this.options.length)];return {sampleRate:this.options.sampleRate,numberOfChannels:this.options.numberOfChannels,length:this.options.length,getChannelData:index=>{this.channelReads.push(index);return this.channels[index];}};}
 startRendering(){this.renderCalls++;return this.render?this.render():Promise.resolve(this.makeBuffer());}
 async resume(){this.state='running';}
 async close(){this.state='closed';}
}
function setup(configure=()=>{}){const contexts=[];return {contexts,createContext:options=>{const context=new FakeContext(options);contexts.push(context);configure(context);return context;}};}
function sink(overrides={}){const calls=[];return {calls,async writeHeader(header,layout){calls.push(['header',header.byteLength,layout]);},async writeChunk(bytes,start,count){calls.push(['chunk',start,count,bytes.byteLength]);},async complete(layout){calls.push(['complete',layout]);},async abort(error){calls.push(['abort',error]);},...overrides};}
const isAbort=error=>error?.name==='AbortError';
async function canRenderAgain(){const environment=setup();await renderPcm16Wav(plan(),sink(),environment);assert.equal(environment.contexts.length,1);}
function graph(context){return {compressorArgs:context.compressorArgs,waves:context.waves,oscillators:context.oscillators.map(oscillator=>({type:oscillator.type,wave:oscillator.wave,frequency:oscillator.frequency.events,starts:oscillator.starts,stop:oscillator.stops[0]})),gains:context.gains.map(gain=>gain.gain.events)};}

describe('offline validation and shared graph (fake contexts only)',()=>{
 it('allocates exactly one bounded stereo context after pure validation',async()=>{
  const input=plan(),before=structuredClone(input),environment=setup(),output=sink();
  const result=await renderPcm16Wav(input,output,environment),context=environment.contexts[0];
  assert.deepEqual(input,before);assert.deepEqual(result,validatePcm16Plan(input));assert.ok(Object.isFrozen(result));
  assert.equal(environment.contexts.length,1);assert.deepEqual(context.options,{numberOfChannels:2,length:60_000,sampleRate:48_000});assert.equal(context.renderCalls,1);
  assert.equal(context.compressorCalls,1);assert.deepEqual(context.compressorArgs,[]);assert.deepEqual(context.master.connections,[context.destination]);assert.deepEqual(context.channelReads,[0,1]);assert.equal(output.calls.at(-1)[0],'complete');
 });
 it('validates scope, time, routes, pitch, gain and total graph budgets before allocation or sink calls',async()=>{
  const invalid=[plan({duration_seconds:0}),plan({duration_seconds:Infinity}),plan({notes:[note({start_seconds:NaN})]}),plan({notes:[note({duration_seconds:0})]}),plan({notes:[note({duration_seconds:1})]}),plan({notes:[note(),note({start_seconds:0})]}),plan({notes:[note({instrument:null})]}),plan({notes:[note({frequency_hz:24_000})]}),plan({notes:[note({velocity:.5})]}),plan({notes:[note({gain_envelope:[{offset_seconds:1,gain:1}]})]})];
  for(const input of invalid){const environment=setup(),output=sink();await assert.rejects(renderPcm16Wav(input,output,environment),RangeError);assert.equal(environment.contexts.length,0);assert.deepEqual(output.calls,[]);}
 });
 it('rejects missing sink callbacks and already aborted signals without taking a slot',async()=>{
  const environment=setup();await assert.rejects(renderPcm16Wav(plan(),{},environment),/sink callbacks/);const controller=new AbortController();controller.abort();const output=sink();
  await assert.rejects(renderPcm16Wav(plan(),output,{...environment,signal:controller.signal}),isAbort);assert.deepEqual(output.calls,[]);assert.equal(environment.contexts.length,0);await canRenderAgain();
 });
 it('passes former duration and count ceilings through to context setup without allocating a large render',async()=>{
  const marker=new Error('test stops before audio allocation');
  const inputs=[plan({duration_seconds:600}),plan({notes:Array(4097).fill(note())}),plan({notes:[note({gain_envelope:Array(32769).fill({offset_seconds:0,gain:1})})]})];
  for(const input of inputs){
   let requested;
   const output=sink();
   await assert.rejects(renderPcm16Wav(input,output,{createContext(options){requested=options;throw marker;}}),error=>error===marker);
   assert.equal(requested.length,Math.ceil(input.duration_seconds*48000)+12000);
   assert.deepEqual(output.calls.map(call=>call[0]),['abort']);
  }
 });
 it('schedules every note once from score zero, retaining leading rests and exactly 12000 tail frames',async()=>{
  const notes=[note(),note({instrument:'flute',start_seconds:.5,duration_seconds:.5})],environment=setup();await renderPcm16Wav(plan({notes}),sink(),environment);const context=environment.contexts[0];
  assert.deepEqual(context.oscillators.map(oscillator=>oscillator.starts),[[.25],[.5]]);assert.deepEqual(context.oscillators.map(oscillator=>oscillator.stops[0]),[.75,1]);assert.equal(context.options.length,60_000);
  for(let index=0;index<notes.length;index++)assert.equal(context.oscillators[index].frequency.events[0][2],notes[index].start_seconds);
 });
 it('accepts a rest-only plan and rounds score frames before adding the tail',async()=>{
  const environment=setup();await renderPcm16Wav(plan({duration_seconds:.5/48_000,notes:[]}),sink(),environment);assert.equal(environment.contexts[0].options.length,12_001);assert.equal(environment.contexts[0].oscillators.length,0);
 });
 it('reuses live voices, instrument aliases, envelopes, connections and default compressor',async()=>{
  const notes=['piano','flute','cello','violin','strings','unknown','Piano','constructor'].map(instrument=>note({instrument,gain_envelope:[{offset_seconds:0,gain:.5},{offset_seconds:.5,gain:1}]}));const environment=setup(),original=globalThis.AudioContext;let live;
  globalThis.AudioContext=class extends FakeContext {constructor(){super({});live=this;}};
  try{
   const output=new WebAudioOutput();await output.resume();for(const entry of notes)output.schedule(entry,0,entry.start_seconds);output.dispose();await renderPcm16Wav(plan({notes}),sink(),environment);
   const offline=environment.contexts[0];assert.deepEqual(graph(offline),graph(live));assert.equal(offline.waves.length,3);assert.equal(offline.oscillators[2].wave,offline.oscillators[3].wave);assert.equal(offline.oscillators[2].wave,offline.oscillators[4].wave);
   for(let index=0;index<notes.length;index++){
    assert.deepEqual(offline.oscillators[index].connections,[offline.gains[index*3+2]]);assert.deepEqual(offline.gains[index*3+2].connections,[offline.gains[index*3]]);assert.deepEqual(offline.gains[index*3].connections,[offline.gains[index*3+1]]);assert.deepEqual(offline.gains[index*3+1].connections,[offline.master]);
   }
  }finally{if(original===undefined)delete globalThis.AudioContext;else globalThis.AudioContext=original;}
 });
 it('routes every pitched orchestral identity identically through live and WAV graphs',async()=>{
  const notes=INSTRUMENT_CATALOG.filter(entry=>entry.pitch_kind==='pitched').map(entry=>note({instrument:entry.id}));
  const environment=setup(),original=globalThis.AudioContext;let live;
  globalThis.AudioContext=class extends FakeContext {constructor(){super({});live=this;}};
  try{
   const output=new WebAudioOutput();await output.resume();for(const entry of notes)output.schedule(entry,0,entry.start_seconds);output.dispose();
   await renderPcm16Wav(plan({notes}),sink(),environment);const offline=environment.contexts[0];
   assert.deepEqual(graph(offline),graph(live));assert.equal(offline.waves.length,25);
   assert.equal(offline.oscillators.length,40);assert.ok(offline.oscillators.every(oscillator=>oscillator.wave));
   assert.ok(offline.oscillators.every(oscillator=>oscillator.frequency.events[0][1]===440));
  }finally{if(original===undefined)delete globalThis.AudioContext;else globalThis.AudioContext=original;}
 });
 it('uses the platform OfflineAudioContext by default',async()=>{
  const original=globalThis.OfflineAudioContext;let context;globalThis.OfflineAudioContext=class extends FakeContext {constructor(options){super(options);context=this;}};
  try{await renderPcm16Wav(plan(),sink());assert.deepEqual(context.options,{numberOfChannels:2,length:60_000,sampleRate:48_000});}finally{if(original===undefined)delete globalThis.OfflineAudioContext;else globalThis.OfflineAudioContext=original;}
 });
});

describe('ordered bounded PCM delivery',()=>{
 it('streams exact header and mock samples in order, including leading silence, tail and final partial chunk',async()=>{
  let expectedStart=0,totalBytes=44,calls=0;const sizes=[];const environment=setup(context=>{context.render=async()=>{const buffer=context.makeBuffer();for(let i=12_000;i<48_000;i++){context.channels[0][i]=i%2?-.5:1;context.channels[1][i]=i%3?0:-1;}for(const channel of context.channels){channel.slice=()=>assert.fail('copied a rendered channel');channel.subarray=()=>assert.fail('copied a rendered channel');}return buffer;};});
  const output=sink({
   async writeHeader(header,layout){assert.equal(calls++,0);const view=new DataView(header.buffer,header.byteOffset,header.byteLength);assert.equal(header.length,44);assert.equal(view.getUint32(40,true),layout.dataBytes);assert.equal(view.getUint32(24,true),48_000);},
   async writeChunk(bytes,start,count){assert.ok(calls++>0);assert.equal(start,expectedStart);assert.equal(bytes.length,count*4);assert.ok(bytes.length<=65_536);sizes.push(bytes.length);const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);for(let j=0;j<count;j++){const frame=start+j,audible=frame>=12_000&&frame<48_000;assert.equal(view.getInt16(j*4,true),audible?(frame%2?-16_384:32_767):0);assert.equal(view.getInt16(j*4+2,true),audible?(frame%3?0:-32_768):0);}expectedStart+=count;totalBytes+=bytes.length;},
   async complete(layout){assert.equal(expectedStart,layout.frameCount);assert.equal(totalBytes,layout.fileBytes);calls++;},
  });
  const Original=globalThis.Uint8Array,allocations=[];globalThis.Uint8Array=new Proxy(Original,{construct(Target,args){allocations.push(args[0]);assert.ok(args[0]<=65_536,'whole PCM file allocated');return new Target(...args);}});
  try{await renderPcm16Wav(plan(),output,environment);}finally{globalThis.Uint8Array=Original;}
  assert.deepEqual(sizes,[65_536,65_536,65_536,43_392]);assert.deepEqual(allocations,[44,...sizes]);assert.equal(calls,6);assert.deepEqual(output.calls,[]);
 });
 it('awaits header, each chunk and completion with one outstanding callback',async()=>{
  const entered=[],releases=[];let pending=0,maximum=0;const callback=label=>async()=>{pending++;maximum=Math.max(maximum,pending);entered.push(label);const gate=deferred();releases.push(gate);await gate.promise;pending--;};
  const output=sink({writeHeader:callback('header'),writeChunk:callback('chunk'),complete:callback('complete')}),rendering=renderPcm16Wav(plan(),output,setup());await Promise.resolve();assert.deepEqual(entered,['header']);let settled=false;void rendering.then(()=>{settled=true;});
  for(const expected of ['chunk','chunk','chunk','chunk','complete']){releases.at(-1).resolve();await new Promise(resolve=>setImmediate(resolve));assert.equal(entered.at(-1),expected);assert.equal(settled,false);}
  releases.at(-1).resolve();await rendering;assert.equal(maximum,1);assert.equal(pending,0);assert.equal(settled,true);
 });
 it('rejects mismatched rendered metadata or channels before writing a header',async()=>{
  for(const mutate of [buffer=>buffer.sampleRate=44_100,buffer=>buffer.numberOfChannels=1,buffer=>buffer.length--,buffer=>buffer.getChannelData=()=>new Float32Array(1),buffer=>buffer.getChannelData=()=>new Float64Array(60_000)]){
   const environment=setup(context=>{context.render=async()=>{const buffer=context.makeBuffer();mutate(buffer);return buffer;};}),output=sink();await assert.rejects(renderPcm16Wav(plan(),output,environment),/validated WAV layout/);assert.deepEqual(output.calls.map(call=>call[0]),['abort']);
  }
 });
});

describe('failure, cancellation and exclusive slot lifecycle',()=>{
 it('rejects concurrent requests across factories until the outstanding render settles',async()=>{
  const gate=deferred(),environment=setup(context=>{context.render=()=>gate.promise;}),output=sink(),first=renderPcm16Wav(plan(),output,environment),secondEnvironment=setup(),secondSink=sink();
  await assert.rejects(renderPcm16Wav(plan(),secondSink,secondEnvironment),/already in progress/);assert.equal(secondEnvironment.contexts.length,0);assert.deepEqual(secondSink.calls,[]);gate.resolve(environment.contexts[0].makeBuffer());await first;await canRenderAgain();
 });
 it('drains a cancelled render, disconnects its graph, emits no bytes and only then releases the slot',async()=>{
  const gate=deferred(),controller=new AbortController(),environment=setup(context=>{context.render=()=>gate.promise;}),output=sink(),first=renderPcm16Wav(plan(),output,{...environment,signal:controller.signal});let settled=false;const rejected=assert.rejects(first,isAbort).then(()=>{settled=true;});
  controller.abort();await Promise.resolve();assert.equal(settled,false);assert.deepEqual(output.calls,[]);const context=environment.contexts[0];assert.equal(context.master.disconnects,1);for(const oscillator of context.oscillators)assert.equal(oscillator.disconnects,1);
  await assert.rejects(renderPcm16Wav(plan(),sink(),setup()),/already in progress/);gate.resolve(context.makeBuffer());await rejected;assert.deepEqual(output.calls.map(call=>call[0]),['abort']);await canRenderAgain();
 });
 it('drains an outstanding sink callback on cancellation before aborting without more output',async()=>{
  for(const phase of ['writeHeader','writeChunk']){
   const gate=deferred(),entered=deferred(),controller=new AbortController(),output=sink();output[phase]=async()=>{output.calls.push([phase]);entered.resolve();await gate.promise;};const rendering=renderPcm16Wav(plan(),output,{...setup(),signal:controller.signal}),rejection=assert.rejects(rendering,isAbort);
   await entered.promise;controller.abort();await assert.rejects(renderPcm16Wav(plan(),sink(),setup()),/already in progress/);assert.notEqual(output.calls.at(-1)[0],'abort');gate.resolve();await rejection;assert.equal(output.calls.at(-1)[0],'abort');assert.ok(!output.calls.some(call=>call[0]==='complete'));assert.equal(output.calls.filter(call=>call[0]===phase).length,1);await canRenderAgain();
  }
 });
 it('accepts cancellation after the final data acknowledgement but before completion',async()=>{
  const controller=new AbortController(),output=sink({async writeChunk(bytes,start,count){if(start+count===60_000)controller.abort();}});await assert.rejects(renderPcm16Wav(plan(),output,{...setup(),signal:controller.signal}),isAbort);assert.deepEqual(output.calls.map(call=>call[0]),['header','abort']);
 });
 it('makes finalization the commit point, so late cancellation cannot report published output as cancelled',async()=>{
  const controller=new AbortController(),gate=deferred(),entered=deferred(),output=sink({async complete(){entered.resolve();await gate.promise;}}),rendering=renderPcm16Wav(plan(),output,{...setup(),signal:controller.signal});await entered.promise;controller.abort();await assert.rejects(renderPcm16Wav(plan(),sink(),setup()),/already in progress/);gate.resolve();assert.deepEqual(await rendering,validatePcm16Plan(plan()));assert.ok(!output.calls.some(call=>call[0]==='abort'));await canRenderAgain();
 });
 it('propagates context, graph and render failures, cleans nodes and allows retry',async()=>{
  const failure=new Error('injected failure'),cases=[{createContext(){throw failure;}},setup(context=>{context.createPeriodicWave=()=>{throw failure;};}),setup(context=>{context.createOscillator=()=>{throw failure;};}),setup(context=>{const create=context.createOscillator.bind(context);context.createOscillator=()=>{const oscillator=create();oscillator.start=()=>{throw failure;};return oscillator;};}),setup(context=>{context.render=()=>{throw failure;};}),setup(context=>{context.render=()=>Promise.reject(failure);})];
  for(const environment of cases){const output=sink();await assert.rejects(renderPcm16Wav(plan(),output,environment),error=>error===failure);assert.deepEqual(output.calls.map(call=>call[0]),['abort']);for(const context of environment.contexts??[]){if(context.master)assert.equal(context.master.disconnects,1);for(const oscillator of context.oscillators)assert.equal(oscillator.disconnects,1);for(const gain of context.gains)assert.equal(gain.disconnects,1);}await canRenderAgain();}
 });
 it('does not complete failed writes, and never reports failed finalization as success',async()=>{
  for(const phase of ['writeHeader','writeChunk','complete']){const failure=new Error(`${phase} failed`),output=sink({async [phase](){this.calls.push([phase]);throw failure;}});await assert.rejects(renderPcm16Wav(plan(),output,setup()),error=>error===failure);assert.equal(output.calls.at(-1)[0],'abort');assert.equal(output.calls.filter(call=>call[0]===phase).length,1);if(phase!=='complete')assert.ok(!output.calls.some(call=>call[0]==='complete'));await canRenderAgain();}
 });
 it('awaits abort cleanup while holding the slot and surfaces original and cleanup failures',async()=>{
  const failure=new Error('write failed'),cleanupFailure=new Error('discard failed'),gate=deferred(),entered=deferred(),output=sink({async writeHeader(){throw failure;},async abort(reason){assert.equal(reason,failure);entered.resolve();await gate.promise;throw cleanupFailure;}}),rendering=renderPcm16Wav(plan(),output,setup());await entered.promise;await assert.rejects(renderPcm16Wav(plan(),sink(),setup()),/already in progress/);gate.resolve();await assert.rejects(rendering,error=>error instanceof AggregateError&&error.errors[0]===failure&&error.errors[1]===cleanupFailure);await canRenderAgain();
 });
 it('removes abort listeners after success and failure',async()=>{
  for(const fails of [false,true]){const controller=new AbortController(),signal=controller.signal,add=signal.addEventListener.bind(signal),remove=signal.removeEventListener.bind(signal),listeners=new Set();signal.addEventListener=(type,listener,options)=>{if(type==='abort')listeners.add(listener);return add(type,listener,options);};signal.removeEventListener=(type,listener)=>{if(type==='abort')listeners.delete(listener);return remove(type,listener);};const output=sink(fails?{async writeHeader(){throw new Error('failed');}}:{}),rendering=renderPcm16Wav(plan(),output,{...setup(),signal});if(fails)await assert.rejects(rendering,/failed/);else await rendering;assert.equal(listeners.size,0);const calls=output.calls.length;controller.abort();assert.equal(output.calls.length,calls);}
 });
});
