import type {EnvelopePoint} from './types';

interface BuiltInTimbre {
 /** Sine-series harmonics, from fundamental upward. Not recorded samples. */
 readonly harmonics:readonly number[];
 readonly envelope:EnvelopePoint[];
}
const piano:BuiltInTimbre={
 harmonics:[1,.45,.2,.12,.05,.02],
 // Absolute note age matters: seeking a held note must not restart its decay.
 envelope:[{offset_seconds:0,gain:1},{offset_seconds:.08,gain:.8},{offset_seconds:.6,gain:.55},{offset_seconds:2,gain:.3},{offset_seconds:6,gain:.12}],
};
const flute:BuiltInTimbre={harmonics:[1,.08,.04,.01],envelope:[]};
const strings:BuiltInTimbre={harmonics:[1,.7,.45,.32,.22,.16,.1,.08],envelope:[]};

/** Exact SFS identifiers only; arbitrary instrument atoms retain a neutral tone. */
export function builtInTimbre(instrument:string):BuiltInTimbre|null {
 switch(instrument){
  case 'piano':return piano;
  case 'flute':return flute;
  case 'cello':case 'violin':case 'strings':return strings;
  default:return null;
 }
}

const waves=new WeakMap<BaseAudioContext,Map<BuiltInTimbre,PeriodicWave>>();
/** Equal steady-state energy before instrument decay and Rust expression gains. */
export function applyBuiltInTimbre(context:BaseAudioContext,oscillator:OscillatorNode,instrument:string):EnvelopePoint[] {
 const timbre=builtInTimbre(instrument);
 if(!timbre){oscillator.type='triangle';return [];}
 let cached=waves.get(context);
 if(!cached){cached=new Map();waves.set(context,cached);}
 let wave=cached.get(timbre);
 if(!wave){
  const real=new Float32Array(timbre.harmonics.length+1),imag=new Float32Array(real.length);
  // Match the neutral triangle's RMS (sqrt(1/3)); avoid browser-specific peak
  // normalization changing the relative level of the three spectral profiles.
  const scale=Math.sqrt((2/3)/timbre.harmonics.reduce((sum,value)=>sum+value*value,0));
  timbre.harmonics.forEach((value,index)=>{imag[index+1]=value*scale;});
  wave=context.createPeriodicWave(real,imag,{disableNormalization:true});
  cached.set(timbre,wave);
 }
 oscillator.setPeriodicWave(wave);
 return timbre.envelope;
}
