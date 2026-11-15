import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import rawPlan from './fixtures/grace-playback.json?raw';
import {gainAt,WebAudioOutput} from '../src/playback/audio';
import type {AudioOutput} from '../src/playback/audio';
import {PlaybackTransport} from '../src/playback/transport';
import type {PlaybackPlan} from '../src/playback/types';

// Generated and equality-checked by Rust from unchanged synthetic MusicXML and
// a complete explicit allocation. This checks scheduling, not rendered audio.
const plan=():PlaybackPlan=>JSON.parse(rawPlan);
const parameter=()=>({setValueAtTime:vi.fn(),linearRampToValueAtTime:vi.fn(),cancelScheduledValues:vi.fn()});
const node=()=>({connect:vi.fn(),disconnect:vi.fn()});
const oscillator=()=>({...node(),frequency:parameter(),setPeriodicWave:vi.fn(),start:vi.fn(),stop:vi.fn(),onended:null as null|(()=>void)});
const gain=()=>({...node(),gain:parameter()});
class MockContext {
 static instances:MockContext[]=[];
 currentTime=10;state='suspended';destination={};
 oscillators:Array<ReturnType<typeof oscillator>>=[];
 gains:Array<ReturnType<typeof gain>>=[];
 constructor(){MockContext.instances.push(this);}
 resume=vi.fn(async()=>{this.state='running';});
 close=vi.fn(async()=>{this.state='closed';});
 createDynamicsCompressor=()=>node();
 createPeriodicWave=vi.fn(()=>({}));
 createOscillator(){const result=oscillator();this.oscillators.push(result);return result;}
 createGain(){const result=gain();this.gains.push(result);return result;}
}
beforeEach(()=>{vi.useFakeTimers();MockContext.instances=[];vi.stubGlobal('AudioContext',MockContext);});
afterEach(()=>{vi.useRealTimers();vi.unstubAllGlobals();});

describe('qualified Rust grace plan consumer, mocked audio only',()=>{
 it('plays source order rather than ID order and keeps written markers separate',()=>{
  const result=plan();expect(result.notes).toHaveLength(3);
  expect(result.notes.map(n=>[n.written_id,n.midi_note,n.start_seconds,n.duration_seconds,n.velocity])).toEqual([
   ['z-grace',74,.25,.0625,45],
   ['a-grace',76,.3125,.1875,45],
   ['principal',67,.5,1.035,90],
  ]);
  expect(result.duration_seconds).toBe(1.625);
  const positions=Object.fromEntries(result.event_positions.map(e=>[e.id,[e.start_seconds,e.end_seconds]]));
  expect(positions['z-grace']).toEqual([.25,.25]);
  expect(positions['a-grace']).toEqual([.25,.25]);
  expect(positions.principal).toEqual([.25,1.625]);
  const second=result.notes[1];
  expect(gainAt(second.gain_envelope,.062499)).toBe(1);
  expect(gainAt(second.gain_envelope,.0625)).toBe(2);
  expect(gainAt(second.gain_envelope,.1875)).toBe(2);
 });

 it('resumes within either grace without restarting age or losing the numeric jump',async()=>{
  const result=plan();
  for(const [index,offset] of [[0,0],[0,.03],[1,0],[1,.03],[1,.0625],[1,.12],[2,.2]]){
   const note=result.notes[index],output=new WebAudioOutput();await output.resume();
   output.schedule(note,offset,20);
   const context=MockContext.instances.at(-1)!,expression=context.gains[0].gain;
   const level=.12*note.velocity/127;
   expect(context.oscillators).toHaveLength(1);
   expect(context.oscillators[0].frequency.setValueAtTime).toHaveBeenCalledExactlyOnceWith(note.frequency_hz,20);
   expect(context.oscillators[0].stop).toHaveBeenCalledExactlyOnceWith(20+note.duration_seconds-offset);
   expect(expression.setValueAtTime).toHaveBeenCalledWith(level*(index===1&&offset>=.0625?2:1),20);
   if(index===1&&offset<.0625){
    expect(expression.linearRampToValueAtTime).toHaveBeenCalledWith(level,20+.0625-offset);
    expect(expression.setValueAtTime).toHaveBeenCalledWith(level*2,20+.0625-offset);
   }
   output.dispose();
  }
 });

 it('seeks actual sounding windows while event seeking retains written onset and clamps',async()=>{
  const result=plan();
  const output:AudioOutput & {now:number;running:boolean}={now:10,running:true,resume:vi.fn(async()=>{}),schedule:vi.fn(),silence:vi.fn(),dispose:vi.fn()};
  const transport=new PlaybackTransport({plan:vi.fn(async()=>result)},output);
  transport.setSource('qualified Rust fixture supplied by test backend');
  await transport.prepareSource();transport.seek(.25);await transport.play();
  expect(output.schedule).toHaveBeenCalledWith(result.notes[0],0,10);
  expect(output.schedule).toHaveBeenCalledWith(result.notes[1],0,10.0625);
  output.now=20;
  for(const position of [.3125,.375,.499]){
   vi.mocked(output.schedule).mockClear();transport.seek(position);
   expect(output.schedule).not.toHaveBeenCalledWith(result.notes[0],expect.anything(),expect.anything());
   const resumed=vi.mocked(output.schedule).mock.calls.find(call=>call[0]===result.notes[1]);
   expect(resumed).toBeDefined();expect(resumed![1]).toBeCloseTo(position-.3125,12);expect(resumed![2]).toBe(20);
   expect(transport.getSnapshot().position).toBeCloseTo(position,12);
  }
  vi.mocked(output.schedule).mockClear();transport.seek(.5);
  expect(output.schedule).toHaveBeenCalledExactlyOnceWith(result.notes[2],0,20);
  vi.mocked(output.schedule).mockClear();await transport.seekEvent('principal');
  expect(transport.getSnapshot().position).toBe(.25);
  expect(output.schedule).toHaveBeenCalledWith(result.notes[0],0,20);
  vi.mocked(output.schedule).mockClear();transport.seek(1.6);
  expect(output.schedule).not.toHaveBeenCalled();
  transport.seek(99);expect(transport.getSnapshot()).toMatchObject({status:'paused',position:1.625});
  expect(vi.getTimerCount()).toBe(0);
  transport.seek(-1);expect(transport.getSnapshot()).toMatchObject({status:'stopped',position:0});
  transport.dispose();
 });
});
