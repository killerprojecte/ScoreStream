import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import rawPlan from './fixtures/joint-playback.json?raw';
import {gainAt,WebAudioOutput} from '../src/playback/audio';
import type {AudioOutput} from '../src/playback/audio';
import {PlaybackTransport} from '../src/playback/transport';
import type {PlaybackPlan} from '../src/playback/types';

// Generated and equality-checked by Rust from unchanged synthetic MusicXML and
// complete explicit grace and hold policies. This checks scheduling, not audio.
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

describe('joint qualified Rust plan consumer, mocked audio only',()=>{
 it('keeps the selected hold, all grace attacks, source markers and boundary dynamics',()=>{
  const result=plan();expect(result.notes).toHaveLength(4);
  expect(result.notes.map(n=>[n.written_id,n.midi_note,n.start_seconds,n.duration_seconds,n.velocity])).toEqual([
   ['held',60,0,1.5,45],['z-grace',74,1.5,.125,90],
   ['a-grace',76,1.625,.1875,90],['principal',67,1.8125,.7475,45],
  ]);
  expect(result.duration_seconds).toBe(2.625);
  const positions=Object.fromEntries(result.event_positions.map(e=>[e.id,[e.start_seconds,e.end_seconds]]));
  expect(positions.held).toEqual([0,1.5]);
  expect(positions['z-grace']).toEqual([1.5,1.5]);
  expect(positions['a-grace']).toEqual([1.5,1.5]);
  expect(positions.principal).toEqual([1.5,2.625]);
  for(const at of [.5,.75,1.25,1.5])expect(gainAt(result.notes[0].gain_envelope,at)).toBe(1);
  expect(gainAt(result.notes[2].gain_envelope,.124999)).toBe(1);
  expect(gainAt(result.notes[2].gain_envelope,.125)).toBe(.5);
  expect(gainAt(result.notes[2].gain_envelope,.1875)).toBe(.5);
 });

 it('resumes old held-note age and later grace ages without applying boundary controls early',async()=>{
  const result=plan();
  for(const [index,offset] of [[0,.5],[0,.75],[0,1.25],[1,0],[1,.0625],[2,.0625],[2,.125],[2,.15],[3,.25]]){
   const note=result.notes[index],output=new WebAudioOutput();await output.resume();
   output.schedule(note,offset,20);
   const context=MockContext.instances.at(-1)!,expression=context.gains[0].gain;
   const level=.12*note.velocity/127;
   expect(context.oscillators).toHaveLength(1);
   expect(context.oscillators[0].frequency.setValueAtTime).toHaveBeenCalledExactlyOnceWith(note.frequency_hz,20);
   expect(context.oscillators[0].stop).toHaveBeenCalledExactlyOnceWith(20+note.duration_seconds-offset);
   expect(expression.setValueAtTime).toHaveBeenCalledWith(level*(index===2&&offset>=.125?.5:1),20);
   if(index===2&&offset<.125){
    expect(expression.linearRampToValueAtTime).toHaveBeenCalledWith(level,20+.125-offset);
    expect(expression.setValueAtTime).toHaveBeenCalledWith(level*.5,20+.125-offset);
   }
   output.dispose();
  }
 });

 it('seeks holds and graces separately, retains written principal onset, and clamps',async()=>{
  const result=plan();
  const output:AudioOutput & {now:number;running:boolean}={now:10,running:true,resume:vi.fn(async()=>{}),schedule:vi.fn(),silence:vi.fn(),dispose:vi.fn()};
  const transport=new PlaybackTransport({plan:vi.fn(async()=>result)},output);
  transport.setSource('joint qualified Rust fixture supplied by test backend');
  await transport.play();expect(output.schedule).toHaveBeenCalledExactlyOnceWith(result.notes[0],0,10);
  output.now=20;
  for(const position of [.75,1.25]){
   vi.mocked(output.schedule).mockClear();transport.seek(position);
   expect(output.schedule).toHaveBeenCalledExactlyOnceWith(result.notes[0],position,20);
  }
  vi.mocked(output.schedule).mockClear();transport.seek(1.5);
  expect(output.schedule).not.toHaveBeenCalledWith(result.notes[0],expect.anything(),expect.anything());
  expect(output.schedule).toHaveBeenCalledWith(result.notes[1],0,20);
  expect(output.schedule).toHaveBeenCalledWith(result.notes[2],0,20.125);
  for(const position of [1.6875,1.75]){
   vi.mocked(output.schedule).mockClear();transport.seek(position);
   expect(output.schedule).toHaveBeenCalledWith(result.notes[2],position-1.625,20);
   expect(output.schedule).not.toHaveBeenCalledWith(result.notes[1],expect.anything(),expect.anything());
  }
  vi.mocked(output.schedule).mockClear();transport.seek(1.8125);
  expect(output.schedule).toHaveBeenCalledExactlyOnceWith(result.notes[3],0,20);
  vi.mocked(output.schedule).mockClear();await transport.seekEvent('principal');
  expect(transport.getSnapshot().position).toBe(1.5);
  expect(output.schedule).toHaveBeenCalledWith(result.notes[1],0,20);
  expect(output.schedule).not.toHaveBeenCalledWith(result.notes[3],expect.anything(),expect.anything());
  vi.mocked(output.schedule).mockClear();transport.seek(2.6);expect(output.schedule).not.toHaveBeenCalled();
  transport.seek(99);expect(transport.getSnapshot()).toMatchObject({status:'paused',position:2.625});
  expect(vi.getTimerCount()).toBe(0);
  transport.seek(-1);expect(transport.getSnapshot()).toMatchObject({status:'stopped',position:0});
  transport.dispose();
 });
});
