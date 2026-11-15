import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import rawPlan from './fixtures/fermata-playback.json?raw';
import {gainAt,WebAudioOutput} from '../src/playback/audio';
import type {AudioOutput} from '../src/playback/audio';
import {PlaybackTransport} from '../src/playback/transport';
import type {PlaybackPlan} from '../src/playback/types';

// This fixture is generated and equality-checked by the Rust formats test from
// unchanged synthetic MusicXML, not assembled by a second JS score interpreter.
// Mocked scheduling is not a waveform, hardware or orchestral-audio test.
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

describe('qualified Rust fermata plan consumer, mocked audio only',()=>{
 it('keeps one tied attack, both selected sustains, numeric step, and final rest hold',()=>{
  const result=plan();expect(result.notes).toHaveLength(1);
  const note=result.notes[0];
  expect(note).toMatchObject({written_id:'start',start_seconds:0,duration_seconds:1.75,midi_note:67,velocity:45});
  expect(result.duration_seconds).toBe(3.5);
  expect(result.event_positions.map(e=>[e.id,e.start_seconds,e.end_seconds])).toEqual([
   ['start',0,.75],['stop',.75,1.75],['rest',1.75,3.5],
  ]);
  for(const position of [.5,.625,.749999])expect(gainAt(note.gain_envelope,position)).toBe(1);
  for(const position of [.75,1.25,1.5,1.75])expect(gainAt(note.gain_envelope,position)).toBe(2);
 });

 it('resumes inside interior and terminal holds with original age and remaining sustain',async()=>{
  const note=plan().notes[0],level=.12*note.velocity/127;
  for(const offset of [0,.5,.625,.75,1.25,1.5]){
   const output=new WebAudioOutput();await output.resume();output.schedule(note,offset,20);
   const context=MockContext.instances.at(-1)!,expression=context.gains[0].gain;
   expect(context.oscillators).toHaveLength(1);
   expect(context.oscillators[0].frequency.setValueAtTime).toHaveBeenCalledExactlyOnceWith(note.frequency_hz,20);
   expect(context.oscillators[0].stop).toHaveBeenCalledExactlyOnceWith(20+1.75-offset);
   expect(expression.setValueAtTime).toHaveBeenCalledWith(level*(offset<.75?1:2),20);
   if(offset<.75){
    expect(expression.linearRampToValueAtTime).toHaveBeenCalledWith(level,20+.75-offset);
    expect(expression.setValueAtTime).toHaveBeenCalledWith(2*level,20+.75-offset);
   }
   output.dispose();
  }
 });

 it('seeks through tied holds, resumes the root, remains silent in the rest, and clamps',async()=>{
  const result=plan();
  const output:AudioOutput & {now:number;running:boolean}={now:10,running:true,resume:vi.fn(async()=>{}),schedule:vi.fn(),silence:vi.fn(),dispose:vi.fn()};
  const transport=new PlaybackTransport({plan:vi.fn(async()=>result)},output);
  transport.setSource('qualified Rust fixture supplied by test backend');
  await transport.play();expect(output.schedule).toHaveBeenCalledExactlyOnceWith(result.notes[0],0,10);
  output.now=20;
  for(const position of [.625,.75,1.5]){
   vi.mocked(output.schedule).mockClear();transport.seek(position);
   expect(output.schedule).toHaveBeenCalledExactlyOnceWith(result.notes[0],position,20);
   expect(transport.getSnapshot().position).toBe(position);
  }
  vi.mocked(output.schedule).mockClear();await transport.seekEvent('stop');
  expect(output.schedule).toHaveBeenCalledExactlyOnceWith(result.notes[0],.75,20);
  vi.mocked(output.schedule).mockClear();transport.seek(3);
  expect(output.schedule).not.toHaveBeenCalled();expect(transport.getSnapshot().position).toBe(3);
  transport.seek(99);expect(transport.getSnapshot()).toMatchObject({status:'paused',position:3.5});
  expect(vi.getTimerCount()).toBe(0);
  transport.seek(-1);expect(transport.getSnapshot()).toMatchObject({status:'stopped',position:0});
  transport.dispose();
 });
});
