// Standalone controller/IPC tests. No native picker, filesystem writer, WebView or Vite.
import assert from 'node:assert/strict';
import {describe,it} from 'node:test';
import {readFileSync} from 'node:fs';
import {WavExportController} from '../src/app/wav-export.ts';
import {createWavExportBridge} from '../src/app/wav-export-bridge.ts';
import {zh,setLanguage,t} from '../src/i18n.ts';
import {createPcm16WavHeader,validatePcm16Plan} from '../src/playback/pcm.ts';

const plan=()=>({duration_seconds:.125,notes:[],routes:[],warnings:[],diagnostics:[],event_positions:[],measures:[]});
const session=()=>{const value=plan();return {sessionId:'opaque-session',path:'/private/export/Example.wav',plan:value,...validatePcm16Plan(value)};};
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};};
const turn=()=>new Promise(resolve=>setImmediate(resolve));
const aborted=()=>new DOMException('Cancelled','AbortError');
async function fakeRender(input,sink,{signal}) {
 const layout=validatePcm16Plan(input);
 const check=()=>{if(signal.aborted)throw aborted();};
 try{
  check();await sink.writeHeader(createPcm16WavHeader(layout.frameCount),layout);check();
  for(let start=0;start<layout.frameCount;start+=16384){const count=Math.min(16384,layout.frameCount-start);await sink.writeChunk(new Uint8Array(count*4),start,count);check();}
  await sink.complete(layout);return layout;
 }catch(error){await sink.abort(error);throw error;}
}
function setup(overrides={},render=fakeRender){
 const calls=[],value=session();
 const api={
  async begin(source){calls.push(['begin',source]);return value;},
  async write(id,offset,bytes){calls.push(['write',id,offset,bytes]);return offset+bytes.byteLength;},
  async finish(id){calls.push(['finish',id]);return value.path;},
  async abort(id){calls.push(['abort',id]);},
  ...overrides,
 };
 const controller=new WavExportController(api,render);
 return {controller,calls,value,api};
}

describe('native WAV binary bridge',()=>{
 it('uses native authority with one top-level Uint8Array and string headers, no writable path',async()=>{
  const calls=[],raw=new Uint8Array([1,2,3,4]);
  const bridge=createWavExportBridge(async(...args)=>{calls.push(args);return args[0]==='wav_export_write'?48:undefined;});
  await bridge.begin('click-time source');assert.equal(await bridge.write('session-1',44,raw),48);await bridge.finish('session-1');await bridge.abort('session-1');
  assert.deepEqual(calls[0],['wav_export_begin',{source:'click-time source'}]);
  assert.equal(calls[1][1],raw);assert.deepEqual(calls[1],['wav_export_write',raw,{headers:{'x-scorestream-wav-session':'session-1','x-scorestream-wav-offset':'44'}}]);
  assert.deepEqual(calls[2],['wav_export_finish',{sessionId:'session-1'}]);assert.deepEqual(calls[3],['wav_export_abort',{sessionId:'session-1'}]);
 });
});

describe('captured independent WAV export operation',()=>{
 it('captures one source, retains the exact native plan and streams bounded byte references in order',async()=>{
  let value,receivedPlan;const chunks=[];
  const render=async(input,sink,options)=>{receivedPlan=input;return fakeRender(input,{...sink,writeChunk:async(bytes,start,count)=>{chunks.push(bytes);return sink.writeChunk(bytes,start,count);}},options);};
  const test=setup({},render);value=test.value;let current='captured draft';const before=structuredClone(value);
  const operation=test.controller.start(current);current='new unsaved source';await operation;
  assert.equal(receivedPlan,value.plan);assert.deepEqual(value,before);assert.deepEqual(test.calls[0],['begin','captured draft']);
  const writes=test.calls.filter(call=>call[0]==='write');assert.deepEqual(writes.map(call=>[call[2],call[3].length]),[[0,44],[44,65536],[65580,6464]]);
  assert.equal(writes[1][3],chunks[0]);assert.equal(writes[2][3],chunks[1]);assert.deepEqual(test.calls.at(-1),['finish','opaque-session']);
  assert.equal(test.controller.getSnapshot().phase,'succeeded');assert.equal(test.controller.getSnapshot().fileName,'Example.wav');assert.equal(test.controller.getSnapshot().writtenBytes,value.fileBytes);assert.equal(test.controller.busy,false);
  assert.ok(!JSON.stringify(test.controller.getSnapshot()).includes('captured draft'));assert.ok(!JSON.stringify(test.controller.getSnapshot()).includes('/private/'));
 });
 it('guards repeated requests throughout the native chooser and closes cancellation quietly',async()=>{
  const gate=deferred();let begins=0,renders=0;const {controller,calls}=setup({begin:async()=>{begins++;return gate.promise;}},async()=>{renders++;});
  const first=controller.start('first');assert.equal(controller.busy,true);assert.equal(controller.getSnapshot().phase,'choosing');
  await controller.start('second');controller.dismiss();assert.equal(controller.getSnapshot().phase,'choosing');assert.equal(begins,1);
  gate.resolve(null);await first;assert.equal(controller.getSnapshot().phase,'idle');assert.equal(controller.busy,false);assert.equal(renders,0);assert.deepEqual(calls,[]);
 });
 it('waits for every native write and finalization before releasing the operation',async()=>{
  const gates=[],calls=[];let inFlight=0,maximum=0;
  const block=async(label,result)=>{calls.push(label);maximum=Math.max(maximum,++inFlight);const gate=deferred();gates.push(gate);await gate.promise;inFlight--;return result;};
  const {controller}=setup({write:async(_id,offset,bytes)=>block('write '+offset,offset+bytes.length),finish:async()=>block('finish','done.wav')});
  const operation=controller.start('score');await turn();assert.deepEqual(calls,['write 0']);
  for(const expected of ['write 44','write 65580','finish']){gates.at(-1).resolve();await turn();assert.equal(calls.at(-1),expected);assert.equal(controller.busy,true);await controller.start('ignored');}
  assert.equal(controller.getSnapshot().phase,'finishing');controller.cancel();assert.equal(controller.getSnapshot().phase,'finishing');
  gates.at(-1).resolve();await operation;assert.equal(maximum,1);assert.equal(controller.getSnapshot().phase,'succeeded');assert.equal(controller.getSnapshot().fileName,'done.wav');
 });
 it('cancels pending rendering only after the renderer drains, aborts once and never finishes',async()=>{
  const gate=deferred();let renderStarted=false;
  const {controller,calls}=setup({},async(_plan,sink,{signal})=>{renderStarted=true;await gate.promise;assert.equal(signal.aborted,true);const error=aborted();await sink.abort(error);throw error;});
  const operation=controller.start('score');await turn();assert.equal(renderStarted,true);controller.cancel();assert.equal(controller.getSnapshot().phase,'cancelling');assert.equal(controller.busy,true);
  await controller.start('ignored');assert.equal(calls.filter(c=>c[0]==='begin').length,1);gate.resolve();await operation;
  assert.equal(controller.getSnapshot().phase,'idle');assert.deepEqual(calls.map(c=>c[0]),['begin','abort']);assert.equal(controller.busy,false);
 });
 it('waits for an in-flight write before aborting and never requests the next chunk',async()=>{
  const gate=deferred(),writes=[];const {controller,calls}=setup({write:async(_id,offset,bytes)=>{writes.push(offset);await gate.promise;return offset+bytes.length;}});
  const operation=controller.start('score');await turn();controller.cancel();assert.equal(calls.some(c=>c[0]==='abort'),false);
  gate.resolve();await operation;assert.deepEqual(writes,[0]);assert.deepEqual(calls.map(c=>c[0]),['begin','abort']);assert.equal(controller.getSnapshot().phase,'idle');
 });
 it('aborts a session returned after cancellation/disposal without rendering or publishing stale state',async()=>{
  for(const dispose of [false,true]){
   const gate=deferred();let renders=0;const {controller,calls,value}=setup({begin:async()=>gate.promise},async()=>{renders++;});let notifications=0;controller.subscribe(()=>notifications++);
   const operation=controller.start('score');if(dispose)controller.dispose();else controller.cancel();const before=notifications;
   gate.resolve(value);await operation;assert.equal(renders,0);assert.deepEqual(calls,[['abort','opaque-session']]);assert.equal(controller.busy,false);
   if(dispose){assert.equal(notifications,before);await controller.start('ignored');assert.equal(calls.length,1);}else assert.equal(controller.getSnapshot().phase,'idle');
  }
 });
 it('leaves finalization result authoritative even if disposed during commit',async()=>{
  const gate=deferred();const {controller,calls}=setup({finish:async()=>gate.promise});const operation=controller.start('score');await turn();assert.equal(controller.getSnapshot().phase,'finishing');controller.dispose();gate.resolve('done.wav');await operation;assert.equal(calls.some(c=>c[0]==='abort'),false);
 });
 it('releases the slot and allows another export after success, cancel or failure',async()=>{
  const {controller,calls,api}=setup();await controller.start('first');controller.dismiss();assert.equal(controller.getSnapshot().phase,'idle');
  api.begin=async()=>null;await controller.start('cancel');assert.equal(controller.getSnapshot().phase,'idle');
  api.begin=async()=>{throw new Error('failure');};await controller.start('failure');assert.equal(controller.getSnapshot().phase,'failed');
  api.begin=async()=>session();await controller.start('again');assert.equal(controller.getSnapshot().phase,'succeeded');assert.equal(calls.filter(c=>c[0]==='finish').length,2);
 });
});

describe('safe WAV export failures',()=>{
 it('rejects native layout mismatches before rendering and discards the session',async()=>{
  let renders=0;const value=session();value.frameCount++;const {controller,calls}=setup({begin:async()=>value},async()=>{renders++;});await controller.start('score');
  assert.equal(renders,0);assert.deepEqual(calls,[['abort','opaque-session']]);assert.equal(controller.getSnapshot().phase,'failed');assert.match(controller.getSnapshot().error,/do not match/);
 });
 it('rejects an over-budget native plan before rendering',async()=>{
  const value=session();value.plan.duration_seconds=121;let renders=0;const {controller,calls}=setup({begin:async()=>value},async()=>{renders++;});await controller.start('score');
  assert.equal(renders,0);assert.deepEqual(calls,[['abort','opaque-session']]);assert.equal(controller.getSnapshot().phase,'failed');
 });
 it('rejects malformed sink header, chunk order and incomplete completion before publication',async()=>{
  const bad=[
   async(layout,sink)=>sink.writeHeader(new Uint8Array(43),layout),
   async(layout,sink)=>sink.writeChunk(new Uint8Array(4),0,1),
   async(layout,sink)=>{await sink.writeHeader(createPcm16WavHeader(layout.frameCount),layout);await sink.writeChunk(new Uint8Array(8),0,1);},
   async(layout,sink)=>{await sink.writeHeader(createPcm16WavHeader(layout.frameCount),layout);await sink.complete(layout);},
  ];
  for(const attempt of bad){const {controller,calls}=setup({},async(input,sink)=>attempt(validatePcm16Plan(input),sink));await controller.start('score');assert.equal(controller.getSnapshot().phase,'failed');assert.equal(calls.some(c=>c[0]==='finish'),false);assert.equal(calls.filter(c=>c[0]==='abort').length,1);}
 });
 it('rejects wrong native byte acknowledgement without finalization or another write',async()=>{
  let writes=0;const {controller,calls}=setup({write:async()=>{writes++;return 1;}});await controller.start('score');
  assert.equal(writes,1);assert.deepEqual(calls.map(c=>c[0]),['begin','abort']);assert.match(controller.getSnapshot().error,/free disk space/);
 });
 it('aborts render, write, and finalization failures exactly once and exposes no raw diagnostics',async()=>{
  for(const stage of ['render','write','finish']){
   const secret='private score contents and /private/user/path';const failure=()=>{throw new Error(secret);};
   const test=setup(stage==='render'?{}:{[stage]:failure},stage==='render'?failure:fakeRender);await test.controller.start(secret);
   assert.equal(test.controller.getSnapshot().phase,'failed');assert.equal(test.calls.filter(c=>c[0]==='abort').length,1);assert.ok(!JSON.stringify(test.controller.getSnapshot()).includes(secret));
  }
 });
 it('maps only known structured codes, including destination collision and invalid source',async()=>{
  const expected={WAV_SOURCE_INVALID:/valid score/,WAV_LIMIT_EXCEEDED:/4,096/,WAV_DESTINATION_EXISTS:/new filename/,WAV_BUSY:/Another WAV export/,WAV_BAD_REQUEST:/rejected/,WAV_SESSION_NOT_FOUND:/session has ended/,WAV_CLEANUP_FAILED:/Restart/,WAV_IO:/Could not start/,constructor:/Could not start/,['__proto__']:/Could not start/,toString:/Could not start/};
  for(const [code,pattern] of Object.entries(expected)){
   const {controller,calls}=setup({begin:async()=>{throw {code,message:'secret source'};}});await controller.start('score');assert.equal(controller.getSnapshot().phase,'failed');assert.match(controller.getSnapshot().error,pattern);assert.equal(calls.length,0);assert.ok(!controller.getSnapshot().error.includes('secret'));
  }
  const {controller}=setup({finish:async()=>{throw {code:'WAV_DESTINATION_EXISTS',message:'sensitive path'};}});await controller.start('score');assert.match(controller.getSnapshot().error,/new filename/);
 });
 it('treats native cancellation as quiet and makes cleanup failure visible even after cancel',async()=>{
  const native=setup({begin:async()=>{throw {code:'WAV_CANCELLED'};}});await native.controller.start('score');assert.equal(native.controller.getSnapshot().phase,'idle');
  const gate=deferred();let aborts=0;const test=setup({write:async(_id,offset,bytes)=>{await gate.promise;return offset+bytes.length;},abort:async()=>{aborts++;throw new Error('permission denied');}});
  const operation=test.controller.start('score');await turn();test.controller.cancel();gate.resolve();await operation;assert.equal(aborts,1);assert.equal(test.controller.getSnapshot().phase,'failed');assert.match(test.controller.getSnapshot().error,/discard.*Restart/);
 });
 it('preserves native write cleanup failure after cancellation when stale-session abort succeeds',async()=>{
  const gate=deferred();let aborts=0,writes=0;
  const {controller,calls}=setup({write:async()=>{writes++;return gate.promise;},abort:async()=>{aborts++;}});
  const operation=controller.start('score');await turn();assert.equal(writes,1);controller.cancel();
  gate.reject({code:'WAV_CLEANUP_FAILED',message:'private temporary path'});await operation;
  assert.equal(aborts,1);assert.equal(calls.some(c=>c[0]==='finish'),false);assert.equal(controller.busy,false);
  assert.equal(controller.getSnapshot().phase,'failed');assert.match(controller.getSnapshot().error,/discard.*Restart/);
  assert.ok(!controller.getSnapshot().error.includes('private temporary path'));
 });
 it('cleans up renderer preflight rejection even when the renderer never calls sink.abort',async()=>{
  const {controller,calls}=setup({},async()=>{throw new Error('another offline render is active');});await controller.start('score');assert.deepEqual(calls.map(c=>c[0]),['begin','abort']);assert.equal(controller.busy,false);
 });
});

it('wires the existing menu and shadcn dialog without invoking source-save or history operations',()=>{
 const menu=readFileSync(new URL('../src/components/AppMenus.tsx',import.meta.url),'utf8');assert.match(menu,/<MenubarItem id="export-wav" disabled=\{!controller.canExportWav\} onSelect=\{\(\)=>void controller.exportWav\(\)\}/);
 const workbench=readFileSync(new URL('../src/app/workbench.ts',import.meta.url),'utf8');assert.match(workbench,/exportWav\(\)\{if\(this.canExportWav\)return this.wavExport.start\(this.state.source\);return Promise.resolve\(\);\}/);
 const dialog=readFileSync(new URL('../src/components/WavExportDialog.tsx',import.meta.url),'utf8');for(const control of ['Dialog','Button','Alert','Spinner'])assert.match(dialog,new RegExp('<'+control));assert.doesNotMatch(dialog,/<button|<input|<dialog/);
 const app=readFileSync(new URL('../src/App.tsx',import.meta.url),'utf8');assert.match(app,/useSyncExternalStore\(controller.wavExport.subscribe,controller.wavExport.getSnapshot\)/);assert.match(app,/if\(wavExportOpen\|\|preferenceOpen/);
});


it('connects the default offline renderer to native chunks using a fake OfflineAudioContext',async()=>{
 const original=globalThis.OfflineAudioContext;let contexts=0;
 globalThis.OfflineAudioContext=class {
  constructor(options){this.options=options;this.destination={};contexts++;}
  createDynamicsCompressor(){return {connect(){},disconnect(){}};}
  async startRendering(){const {numberOfChannels,length,sampleRate}=this.options;return {numberOfChannels,length,sampleRate,getChannelData:()=>new Float32Array(length)};}
 };
 try{
  const {api,calls,value}=setup();const controller=new WavExportController(api);await controller.start('rest-only score');
  assert.equal(contexts,1);assert.equal(controller.getSnapshot().phase,'succeeded');assert.equal(calls.filter(c=>c[0]==='write').reduce((sum,c)=>sum+c[3].length,0),value.fileBytes);assert.equal(calls.at(-1)[0],'finish');
 }finally{if(original===undefined)delete globalThis.OfflineAudioContext;else globalThis.OfflineAudioContext=original;}
});

it('localizes every export dialog status and safe error, including parameterized success',()=>{
 const controller=readFileSync(new URL('../src/app/wav-export.ts',import.meta.url),'utf8');
 const dialog=readFileSync(new URL('../src/components/WavExportDialog.tsx',import.meta.url),'utf8');
 const messages=[...controller.matchAll(/(?:choosing|rendering|writing|finishing|cleanup|layout|WAV_[A-Z_]+):'([^']+)'/g),...dialog.matchAll(/(?:choosing|rendering|writing|finishing|cancelling|failed):'([^']+)'/g),...dialog.matchAll(/t\('([^']+)'/g)].map(match=>match[1]);
 for(const message of messages)assert.equal(typeof zh[message],'string',message);
 setLanguage('zh-CN');try{assert.equal(t('Exported {name}.',{name:'test.wav'}),'已导出 test.wav。');assert.equal(t('Export WAV…'),'导出 WAV…');}finally{setLanguage('en');}
});
