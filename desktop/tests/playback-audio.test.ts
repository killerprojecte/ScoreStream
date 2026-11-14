import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import {gainAt,WebAudioOutput} from '../src/playback/audio';
import type {PlaybackNote} from '../src/playback/types';
const note:PlaybackNote={id:'n',part:'p',voice:'v',instrument:'piano',start_seconds:2,duration_seconds:4,midi_note:69,frequency_hz:443.25,velocity:63.5,gain_envelope:[{offset_seconds:0,gain:.2},{offset_seconds:2,gain:1},{offset_seconds:4,gain:.5}]};
const parameter=()=>({setValueAtTime:vi.fn(),linearRampToValueAtTime:vi.fn(),cancelScheduledValues:vi.fn()});
const node=()=>({connect:vi.fn(),disconnect:vi.fn()});
const oscillatorNode=()=>({...node(),type:'sine',frequency:parameter(),start:vi.fn(),stop:vi.fn(),onended:null as null|(()=>void)});
const gainNode=()=>({...node(),gain:parameter()});
class MockContext {
 static instances:MockContext[]=[];
 currentTime=10;state='suspended';destination={};compressor=node();
 oscillators:Array<ReturnType<typeof oscillatorNode>>=[];
 gains:Array<ReturnType<typeof gainNode>>=[];
 constructor(){MockContext.instances.push(this);}
 resume=vi.fn(async()=>{this.state='running';});close=vi.fn(async()=>{this.state='closed';});
 createDynamicsCompressor=vi.fn(()=>this.compressor);
 createOscillator(){const oscillator=oscillatorNode();this.oscillators.push(oscillator);return oscillator;}
 createGain(){const gain=gainNode();this.gains.push(gain);return gain;}
}
beforeEach(()=>{MockContext.instances=[];vi.stubGlobal('AudioContext',MockContext);});afterEach(()=>vi.unstubAllGlobals());
describe('Rust gain envelope interpolation',()=>{
 it('interpolates at the absolute note offset and retains the final value',()=>{expect(gainAt(note.gain_envelope,0)).toBe(.2);expect(gainAt(note.gain_envelope,1)).toBeCloseTo(.6);expect(gainAt(note.gain_envelope,3)).toBe(.75);expect(gainAt(note.gain_envelope,9)).toBe(.5);});
 it('defaults to unity and interpolates before the first point',()=>{expect(gainAt([],3)).toBe(1);expect(gainAt([{offset_seconds:2,gain:0}],1)).toBe(.5);});
});
describe('Web Audio output, mocked context only, not hardware validation',()=>{
 it('creates a context lazily and reports its audio clock',async()=>{const output=new WebAudioOutput();expect(MockContext.instances).toHaveLength(0);expect(output.now).toBe(0);expect(output.running).toBe(false);await output.resume();const context=MockContext.instances[0];expect(context.compressor.connect).toHaveBeenCalledWith(context.destination);expect(output.now).toBe(10);expect(output.running).toBe(true);await output.resume();expect(MockContext.instances).toHaveLength(1);output.dispose();});
 it('schedules Rust pitch, velocity, envelope and remaining duration when seeking',async()=>{
  const output=new WebAudioOutput();await output.resume();output.schedule(note,1,20);const context=MockContext.instances[0],oscillator=context.oscillators[0],expression=context.gains[0],gate=context.gains[1];
  expect(oscillator.type).toBe('triangle');expect(oscillator.frequency.setValueAtTime).toHaveBeenCalledExactlyOnceWith(443.25,20);expect(expression.gain.setValueAtTime).toHaveBeenCalledExactlyOnceWith(.06*gainAt(note.gain_envelope,1),20);expect(expression.gain.linearRampToValueAtTime.mock.calls).toEqual([[.06,21],[.03,23]]);
  expect(gate.gain.setValueAtTime.mock.calls).toEqual([[0,20],[1,22.988]]);expect(gate.gain.linearRampToValueAtTime.mock.calls).toEqual([[1,20.008],[0,23]]);expect(oscillator.start).toHaveBeenCalledExactlyOnceWith(20);expect(oscillator.stop).toHaveBeenCalledExactlyOnceWith(23);expect(oscillator.connect).toHaveBeenCalledWith(expression);expect(expression.connect).toHaveBeenCalledWith(gate);expect(gate.connect).toHaveBeenCalledWith(context.compressor);output.dispose();
 });
 it('skips completed notes and envelope points beyond duration',async()=>{const output=new WebAudioOutput();await output.resume();const context=MockContext.instances[0];output.schedule(note,4,10);expect(context.oscillators).toHaveLength(0);output.schedule({...note,gain_envelope:[...note.gain_envelope,{offset_seconds:6,gain:0}]},3,10);expect(context.gains[0].gain.linearRampToValueAtTime.mock.calls).toEqual([[.03,11]]);output.dispose();});
 it('shortens attack and release for tiny notes',async()=>{const output=new WebAudioOutput();await output.resume();output.schedule({...note,duration_seconds:.003,gain_envelope:[]},0,10);const gate=MockContext.instances[0].gains[1];expect(gate.gain.linearRampToValueAtTime.mock.calls).toEqual([[1,10.001],[0,10.003]]);expect(gate.gain.setValueAtTime.mock.calls).toEqual([[0,10],[1,10.002]]);output.dispose();});
 it('silences queued/current voices and disconnects their nodes idempotently',async()=>{
  const output=new WebAudioOutput();await output.resume();output.schedule(note,0,20);output.schedule(note,0,30);const context=MockContext.instances[0];context.currentTime=11;output.silence();for(const oscillator of context.oscillators){expect(oscillator.stop).toHaveBeenLastCalledWith();expect(oscillator.disconnect).toHaveBeenCalledTimes(1);}for(const gain of context.gains)expect(gain.disconnect).toHaveBeenCalledTimes(1);for(const gate of [context.gains[1],context.gains[3]]){expect(gate.gain.cancelScheduledValues).toHaveBeenCalledWith(11);expect(gate.gain.setValueAtTime).toHaveBeenLastCalledWith(0,11);}output.silence();expect(context.oscillators[0].stop).toHaveBeenCalledTimes(2);output.dispose();expect(context.close).toHaveBeenCalledTimes(1);expect(output.running).toBe(false);
 });
 it('removes naturally ended voices',async()=>{const output=new WebAudioOutput();await output.resume();output.schedule(note,0,10);const oscillator=MockContext.instances[0].oscillators[0];oscillator.onended!();output.silence();expect(oscillator.stop).toHaveBeenCalledTimes(1);expect(oscillator.disconnect).toHaveBeenCalledTimes(1);output.dispose();});
 it('cleans up oscillator startup failure',async()=>{const output=new WebAudioOutput();await output.resume();const context=MockContext.instances[0],create=context.createOscillator.bind(context);vi.spyOn(context,'createOscillator').mockImplementation(()=>{const oscillator=create();oscillator.start.mockImplementation(()=>{throw new Error('start failed');});return oscillator;});expect(()=>output.schedule(note,0,10)).toThrow('start failed');expect(context.oscillators[0].disconnect).toHaveBeenCalled();expect(context.gains[1].gain.cancelScheduledValues).toHaveBeenCalledWith(10);output.dispose();});
 it('rejects unavailable and suspended audio instead of claiming playback',async()=>{const output=new WebAudioOutput();expect(()=>output.schedule(note,0,0)).toThrow('unavailable');await output.resume();const context=MockContext.instances[0];context.state='suspended';context.resume.mockResolvedValue();await expect(output.resume()).rejects.toThrow('could not start');context.resume.mockRejectedValue(new Error('permission denied'));await expect(output.resume()).rejects.toThrow('permission denied');output.dispose();});
 it('recreates a closed context and tolerates close failure',async()=>{const output=new WebAudioOutput();await output.resume();MockContext.instances[0].state='closed';await output.resume();expect(MockContext.instances).toHaveLength(2);MockContext.instances[1].close.mockRejectedValue(new Error('already closed'));output.dispose();await Promise.resolve();expect(output.now).toBe(0);});
});
