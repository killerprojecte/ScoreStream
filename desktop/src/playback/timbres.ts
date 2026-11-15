import type {EnvelopePoint} from './types';
import {resolveInstrument} from '../instruments/catalog.ts';
import type {StandardInstrumentId} from '../instruments/catalog.ts';

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

function sustained(harmonics:readonly number[]):BuiltInTimbre {return {harmonics,envelope:[]};}
function struck(harmonics:readonly number[],decay:number):BuiltInTimbre {
 return {harmonics,envelope:[
  {offset_seconds:0,gain:1},{offset_seconds:decay*.08,gain:.72},
  {offset_seconds:decay*.3,gain:.4},{offset_seconds:decay,gain:.08},
  {offset_seconds:decay*3,gain:0},
 ]};
}

// Deliberately simple additive approximations, not instrument recordings or
// acoustic models. Shared profiles are explicit; identities remain independent.
const profiles={
 piano,flute,strings,
 piccolo:sustained([1,.15,.08,.035]),
 oboe:sustained([1,.85,.55,.35,.2,.12,.07]),
 english_horn:sustained([1,.65,.5,.24,.18,.08,.04]),
 clarinet:sustained([1,.03,.6,.02,.3,.01,.14]),
 bass_clarinet:sustained([1,.07,.45,.04,.2,.02,.09]),
 bassoon:sustained([1,.55,.75,.3,.38,.15,.18,.08]),
 contrabassoon:sustained([1,.38,.6,.2,.3,.12,.12]),
 saxophone:sustained([1,.6,.5,.32,.22,.14,.1]),
 horn:sustained([1,.3,.16,.08,.035,.015]),
 trumpet:sustained([1,.9,.7,.48,.3,.18,.1,.05]),
 trombone:sustained([1,.65,.42,.28,.18,.1,.06]),
 tuba:sustained([1,.4,.22,.12,.06,.025]),
 harp:struck([1,.3,.14,.06,.025],2),
 celesta:struck([1,.1,.5,.06,.22,.03,.08],1.6),
 harpsichord:struck([1,.7,.4,.25,.15,.08],1.2),
 organ:sustained([1,.15,.32,.1,.05,.08]),
 timpani:struck([1,.42,.18,.12,.05],1.4),
 glockenspiel:struck([1,.04,.65,.025,.28,.015,.14],1.8),
 xylophone:struck([1,.08,.65,.1,.25,.03],.35),
 marimba:struck([1,.03,.3,.02,.1],.8),
 vibraphone:struck([1,.12,.35,.05,.15],2.5),
 tubular_bells:struck([1,.45,.12,.3,.08,.18,.04],3),
} satisfies Record<string,BuiltInTimbre>;
export type BuiltInProfileId=keyof typeof profiles;
/** Every catalog identity has a deliberate support decision; null is NOT a drum. */
export const BUILT_IN_PROFILES:Readonly<Record<StandardInstrumentId,BuiltInProfileId|null>>={
 violin:'strings',viola:'strings',cello:'strings',double_bass:'strings',strings:'strings',
 piccolo:'piccolo',flute:'flute',alto_flute:'flute',bass_flute:'flute',
 oboe:'oboe',english_horn:'english_horn',clarinet:'clarinet',eb_clarinet:'clarinet',
 bass_clarinet:'bass_clarinet',bassoon:'bassoon',contrabassoon:'contrabassoon',
 soprano_saxophone:'saxophone',alto_saxophone:'saxophone',tenor_saxophone:'saxophone',baritone_saxophone:'saxophone',
 horn:'horn',trumpet:'trumpet',cornet:'trumpet',alto_trombone:'trombone',trombone:'trombone',bass_trombone:'trombone',tuba:'tuba',euphonium:'tuba',
 harp:'harp',piano:'piano',celesta:'celesta',harpsichord:'harpsichord',organ:'organ',
 timpani:'timpani',glockenspiel:'glockenspiel',xylophone:'xylophone',marimba:'marimba',
 vibraphone:'vibraphone',tubular_bells:'tubular_bells',crotales:'glockenspiel',
 snare_drum:null,bass_drum:null,crash_cymbals:null,suspended_cymbal:null,triangle:null,
 tambourine:null,tam_tam:null,wood_block:null,castanets:null,whip:null,claves:null,
};
/** Exact catalog aliases only; arbitrary atoms retain the legacy neutral tone. */
export function builtInProfile(instrument:string):BuiltInProfileId|null {
 const definition=resolveInstrument(instrument);
 return definition?BUILT_IN_PROFILES[definition.id]:null;
}
export function builtInTimbre(instrument:string):BuiltInTimbre|null {
 const profile=builtInProfile(instrument);
 return profile?profiles[profile]:null;
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
  // normalization changing the relative level of the spectral profiles.
  const scale=Math.sqrt((2/3)/timbre.harmonics.reduce((sum,value)=>sum+value*value,0));
  timbre.harmonics.forEach((value,index)=>{imag[index+1]=value*scale;});
  wave=context.createPeriodicWave(real,imag,{disableNormalization:true});
  cached.set(timbre,wave);
 }
 oscillator.setPeriodicWave(wave);
 return timbre.envelope;
}
