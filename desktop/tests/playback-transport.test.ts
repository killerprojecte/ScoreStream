import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import {PlaybackTransport} from '../src/playback/transport';
import type {AudioOutput} from '../src/playback/audio';
import type {PlaybackNote,PlaybackPlan} from '../src/playback/types';

const note=(id:string,start:number,duration:number):PlaybackNote=>({id,part:'p',voice:'v',instrument:'piano',start_seconds:start,duration_seconds:duration,midi_note:69,frequency_hz:441.5,velocity:83,gain_envelope:[{offset_seconds:0,gain:.3},{offset_seconds:duration,gain:.9}]});
const plan=():PlaybackPlan=>({duration_seconds:4,notes:[note('tied',0,3),note('later',1,1),note('last',3.5,.5)],routes:[{part:'p',instrument:'piano'}],warnings:[],diagnostics:[],event_positions:[{id:'tie-continuation',start_seconds:2,end_seconds:3}],measures:[]});
function deferred<T>(){let resolve!:(value:T)=>void;let reject!:(reason:unknown)=>void;const promise=new Promise<T>((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};}
function setup(result=plan()){
 const output:AudioOutput & {now:number;running:boolean}={now:10,running:true,resume:vi.fn(async()=>{}),schedule:vi.fn(),silence:vi.fn(),dispose:vi.fn()};
 const backend={plan:vi.fn(async(_source:string)=>result)};
 const transport=new PlaybackTransport(backend,output);transport.setSource('opaque Rust score input');
 return {transport,output,backend,result};
}
beforeEach(()=>vi.useFakeTimers());
afterEach(()=>vi.useRealTimers());
describe('Rust-plan audio-clock transport',()=>{
 it('passes source unchanged and schedules the original Rust note, without deriving pitch, velocity or timing',async()=>{
  const {transport,output,backend,result}=setup();await transport.play();
  expect(backend.plan).toHaveBeenCalledWith('opaque Rust score input');
  expect(output.schedule).toHaveBeenCalledExactlyOnceWith(result.notes[0],0,10);
  expect(vi.mocked(output.schedule).mock.calls[0][0]).toBe(result.notes[0]);
  output.now=10.9;vi.advanceTimersByTime(25);
  expect(output.schedule).toHaveBeenLastCalledWith(result.notes[1],0,11);
  vi.advanceTimersByTime(2000);expect(transport.getSnapshot().position).toBeCloseTo(.9);
  transport.dispose();
 });
 it('ignores repeated Play while preparing and playing',async()=>{
  const {transport,backend,output,result}=setup(),pending=deferred<PlaybackPlan>();backend.plan.mockReturnValue(pending.promise);
  const first=transport.play();await transport.play();expect(backend.plan).toHaveBeenCalledTimes(1);expect(output.resume).toHaveBeenCalledTimes(1);
  pending.resolve(result);await first;await transport.play();expect(output.schedule).toHaveBeenCalledTimes(1);transport.dispose();
 });
 for(const boundary of ['plan','resume'] as const)for(const action of ['stop','pause','edit','dispose'] as const){
  it(`cancels late ${boundary} completion after ${action}`,async()=>{
   const {transport,backend,output,result}=setup(),pending=deferred<PlaybackPlan>(),resume=deferred<void>();
   if(boundary==='plan')backend.plan.mockReturnValue(pending.promise);else vi.mocked(output.resume).mockReturnValue(resume.promise);
   const playing=transport.play();
   if(action==='edit')transport.setSource('new source');else transport[action]();
   pending.resolve(result);resume.resolve();await playing;vi.advanceTimersByTime(1000);
   expect(output.schedule).not.toHaveBeenCalled();expect(transport.getSnapshot().status).not.toBe('playing');
   if(action==='edit')expect(transport.getSnapshot().plan).toBeNull();
   transport.dispose();
  });
 }
 it('does not let an old source plan overwrite the new plan or restart playback',async()=>{
  const {transport,backend,output,result}=setup(),old=deferred<PlaybackPlan>();backend.plan.mockReturnValueOnce(old.promise);
  const first=transport.play();transport.setSource('second');const newer={...result,duration_seconds:7};backend.plan.mockResolvedValue(newer);await transport.play();
  old.resolve(result);await first;expect(transport.getSnapshot().plan).toBe(newer);expect(output.schedule).toHaveBeenCalledTimes(1);transport.dispose();
 });
 it('ignores a canceled planning failure',async()=>{
  const {transport,backend}=setup(),pending=deferred<PlaybackPlan>();backend.plan.mockReturnValue(pending.promise);
  const first=transport.play();transport.stop();pending.reject(new Error('old error'));await first;expect(transport.getSnapshot()).toMatchObject({status:'stopped',error:''});transport.dispose();
 });
 it('pauses at the audio-clock position and resumes a sustaining note at the offset',async()=>{
  const {transport,output,result}=setup();await transport.play();output.now=11.25;transport.pause();expect(transport.getSnapshot()).toMatchObject({status:'paused',position:1.25});
  output.now=30;vi.mocked(output.schedule).mockClear();await transport.play();expect(output.schedule).toHaveBeenCalledWith(result.notes[0],1.25,30);expect(output.schedule).toHaveBeenCalledWith(result.notes[1],.25,30);
  output.now=30.5;vi.advanceTimersByTime(25);expect(transport.getSnapshot().position).toBeCloseTo(1.75);transport.dispose();
 });
 it('seeks forward and backward while playing, cancels queued voices and reanchors the audio clock',async()=>{
  const {transport,output,result}=setup();await transport.play();vi.mocked(output.schedule).mockClear();vi.mocked(output.silence).mockClear();
  output.now=20;transport.seek(2.25);expect(output.silence).toHaveBeenCalled();expect(output.schedule).toHaveBeenCalledExactlyOnceWith(result.notes[0],2.25,20);
  output.now=20.25;vi.advanceTimersByTime(25);expect(transport.getSnapshot().position).toBeCloseTo(2.5);
  transport.seek(.5);expect(output.schedule).toHaveBeenLastCalledWith(result.notes[0],.5,20.25);transport.dispose();
 });
 it('seeks to a tied continuation event from Rust and resumes the single sounding note',async()=>{
  const {transport,output,result}=setup();await transport.seekEvent('tie-continuation');expect(transport.getSnapshot()).toMatchObject({status:'paused',position:2});await transport.play();expect(output.schedule).toHaveBeenCalledExactlyOnceWith(result.notes[0],2,10);transport.dispose();
 });
 it('does not strand pending Play when an unknown score event is clicked',async()=>{
  const {transport,backend,result}=setup(),pending=deferred<PlaybackPlan>();backend.plan.mockReturnValue(pending.promise);
  const playing=transport.play(),seeking=transport.seekEvent('missing-event');pending.resolve(result);await Promise.all([playing,seeking]);
  expect(transport.getSnapshot().status).not.toBe('loading');transport.dispose();
 });
 it('does not publish a plan or notify listeners after disposal',async()=>{
  const {transport,backend,output,result}=setup(),pending=deferred<PlaybackPlan>(),listener=vi.fn();backend.plan.mockReturnValue(pending.promise);transport.subscribe(listener);
  const preparing=transport.prepareSource();transport.dispose();const snapshot=transport.getSnapshot();listener.mockClear();pending.resolve(result);await preparing;
  expect(transport.getSnapshot()).toBe(snapshot);expect(transport.getSnapshot().plan).toBeNull();expect(listener).not.toHaveBeenCalled();expect(output.schedule).not.toHaveBeenCalled();
 });
 it('does not contaminate a new source with a rejected old prefetch',async()=>{
  const {transport,backend,result}=setup(),old=deferred<PlaybackPlan>(),current=deferred<PlaybackPlan>();backend.plan.mockReturnValueOnce(old.promise).mockReturnValueOnce(current.promise);
  const first=transport.prepareSource();transport.setSource('new source');const second=transport.prepareSource();old.reject(new Error('obsolete parse failure'));await first;
  expect(transport.getSnapshot()).toMatchObject({status:'stopped',plan:null,error:''});const samePreparation=transport.prepareSource();expect(backend.plan).toHaveBeenCalledTimes(2);
  current.resolve(result);await Promise.all([second,samePreparation]);expect(transport.getSnapshot()).toMatchObject({plan:result,error:''});transport.dispose();
 });
 it('seeks a known event while audio resume is pending without allowing stale playback',async()=>{
  const {transport,output,result}=setup(),resume=deferred<void>();await transport.prepareSource();vi.mocked(output.resume).mockReturnValueOnce(resume.promise);
  const playing=transport.play();await transport.seekEvent('tie-continuation');expect(transport.getSnapshot()).toMatchObject({status:'paused',position:2});resume.resolve();await playing;
  expect(output.schedule).not.toHaveBeenCalled();expect(transport.getSnapshot()).toMatchObject({status:'paused',position:2});await transport.play();expect(output.schedule).toHaveBeenCalledExactlyOnceWith(result.notes[0],2,10);transport.dispose();
 });
 it('prefetches without starting audio and reuses the plan for Play',async()=>{
  const {transport,output,backend}=setup();await transport.prepareSource();expect(output.resume).not.toHaveBeenCalled();expect(output.schedule).not.toHaveBeenCalled();expect(transport.getSnapshot().status).toBe('stopped');await transport.play();expect(backend.plan).toHaveBeenCalledTimes(1);transport.dispose();
 });
 it('activates audio synchronously before requesting the Rust plan',async()=>{
  const {transport,output,backend,result}=setup(),order:string[]=[];vi.mocked(output.resume).mockImplementation(async()=>{order.push('audio');});backend.plan.mockImplementation(async()=>{order.push('plan');return result;});await transport.play();expect(order).toEqual(['audio','plan']);transport.dispose();
 });
 it('cancels pending event seeks after stop or a source edit',async()=>{
  for(const edit of [false,true]){const {transport,backend,result}=setup(),pending=deferred<PlaybackPlan>();backend.plan.mockReturnValue(pending.promise);const seek=transport.seekEvent('tie-continuation');if(edit)transport.setSource('new');else transport.stop();pending.resolve(result);await seek;expect(transport.getSnapshot().position).toBe(0);transport.dispose();}
 });
 it('ends at duration and replays from zero',async()=>{
  const {transport,output,result}=setup();await transport.play();output.now=14;vi.advanceTimersByTime(25);expect(transport.getSnapshot()).toMatchObject({status:'stopped',position:4});
  vi.mocked(output.schedule).mockClear();await transport.play();expect(output.schedule).toHaveBeenCalledExactlyOnceWith(result.notes[0],0,14);transport.dispose();
 });
 it('clamps seeks and clears a playing timer when seeking to the end',async()=>{
  const {transport}=setup();await transport.play();transport.seek(99);expect(transport.getSnapshot()).toMatchObject({status:'paused',position:4});transport.seek(-1);expect(transport.getSnapshot()).toMatchObject({status:'stopped',position:0});transport.seek(NaN);expect(transport.getSnapshot().position).toBe(0);transport.dispose();
 });
 it('handles an empty plan without running a timer',async()=>{
  const {transport,output}=setup({...plan(),duration_seconds:0,notes:[]});await transport.play();expect(transport.getSnapshot().status).toBe('stopped');expect(output.schedule).not.toHaveBeenCalled();expect(vi.getTimerCount()).toBe(0);transport.dispose();
 });
 it('surfaces unavailable audio and allows retry',async()=>{
  const {transport,output}=setup();vi.mocked(output.resume).mockRejectedValueOnce(new Error('Audio unavailable'));await transport.play();expect(transport.getSnapshot()).toMatchObject({status:'error',error:'Audio unavailable'});expect(output.schedule).not.toHaveBeenCalled();await transport.play();expect(transport.getSnapshot().status).toBe('playing');transport.dispose();
 });
 it('silences all voices and preserves position after scheduling failure or context interruption',async()=>{
  for(const scheduling of [false,true]){const {transport,output}=setup();await transport.play();output.now=10.9;if(scheduling)vi.mocked(output.schedule).mockImplementation(()=>{throw new Error('schedule failed');});else output.running=false;vi.advanceTimersByTime(25);expect(transport.getSnapshot().status).toBe('error');expect(transport.getSnapshot().position).toBeCloseTo(.9);expect(output.silence).toHaveBeenCalled();expect(vi.getTimerCount()).toBe(0);transport.dispose();}
 });
});
