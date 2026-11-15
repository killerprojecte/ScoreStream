// Browser-only OfflineAudioContext regression, not imported by the application.
import {scheduleBuiltInVoice} from '../src/playback/audio';
import type {PlaybackNote} from '../src/playback/types';
import {INSTRUMENT_CATALOG} from '../src/instruments/catalog';
import {builtInProfile,builtInTimbre} from '../src/playback/timbres';

const sampleRate=48000;
const base:PlaybackNote={id:'n',part:'p',voice:'v',instrument:'flute',start_seconds:0,duration_seconds:1,midi_note:69,frequency_hz:400,velocity:100,gain_envelope:[]};
function assert(condition:boolean,message:string):asserts condition {if(!condition)throw new Error(message);}
async function render(notes:PlaybackNote[],offset=0){
 const context=new OfflineAudioContext(1,Math.ceil((Math.max(...notes.map(note=>note.duration_seconds-offset))+.1)*sampleRate),sampleRate);
 for(const note of notes)scheduleBuiltInVoice(context,context.destination,note,offset,.02);
 const data=(await context.startRendering()).getChannelData(0);
 assert(data.every(Number.isFinite),'The rendered signal contains a non-finite sample.');
 return data;
}
function rms(data:Float32Array,from:number,to:number){
 const samples=data.slice(Math.round(from*sampleRate),Math.round(to*sampleRate));
 return Math.sqrt(samples.reduce((sum,value)=>sum+value*value,0)/samples.length);
}
function amplitude(data:Float32Array,harmonic:number){
 const start=Math.round(.12*sampleRate),end=Math.round(.22*sampleRate);let real=0,imag=0;
 for(let index=start;index<end;index++){
  const phase=2*Math.PI*400*harmonic*index/sampleRate;
  real+=data[index]*Math.cos(phase);imag+=data[index]*Math.sin(phase);
 }
 return 2*Math.hypot(real,imag)/(end-start);
}
function maxDifference(left:Float32Array,right:Float32Array){
 assert(left.length===right.length,'Signal lengths differ.');let max=0;
 for(let index=0;index<left.length;index++)max=Math.max(max,Math.abs(left[index]-right[index]));
 return max;
}
async function check(){
 const signals:Record<string,Float32Array>={},harmonics:Record<string,number>={};
 for(const instrument of ['piano','flute','cello']){
  const data=signals[instrument]=await render([{...base,instrument}]);
  assert(rms(data,.12,.22)>.01,`${instrument} is silent.`);
  assert(rms(data,0,.019)===0&&rms(data,1.021,1.09)===0,`${instrument} violates its note boundaries.`);
  harmonics[instrument]=amplitude(data,2)/amplitude(data,1);
 }
 assert(Math.abs(harmonics.piano-.45)<.03&&Math.abs(harmonics.flute-.08)<.03&&Math.abs(harmonics.cello-.7)<.03,'The rendered timbres do not have distinct expected spectra.');
 for(const alias of ['violin','strings'])assert(maxDifference(signals.cello,await render([{...base,instrument:alias}]))<1e-6,`${alias} did not route to bowed strings.`);
 const fallback=await render([{...base,instrument:'unknown'}]);
 for(const instrument of ['Piano','constructor','__proto__'])assert(maxDifference(fallback,await render([{...base,instrument}]))<1e-6,'Fallback is not deterministic.');
 // Render every declared pitched identity, checking actual audio rather than a
 // map of names. Distinct profile checks do not pretend shared voices are unique.
 const renderedProfiles=new Map<string,Float32Array>();
 let pitched=0,unpitched=0;
 for(const definition of INSTRUMENT_CATALOG){
  const profile=builtInProfile(definition.id);
  if(definition.pitch_kind==='unpitched'){
   assert(profile===null,`${definition.id} falsely advertises a percussion renderer.`);unpitched++;continue;
  }
  assert(profile!==null,`${definition.id} has no synthesized voice.`);
  const signal=await render([{...base,instrument:definition.id}]);
  assert(rms(signal,.025,.08)>.001,`${definition.id} is silent.`);
  assert(rms(signal,0,.019)===0&&rms(signal,1.021,1.09)===0,`${definition.id} exceeds note boundaries.`);
  assert(maxDifference(signal,fallback)>.001,`${definition.id} is only the neutral fallback.`);
  const existing=renderedProfiles.get(profile);
  if(existing)assert(maxDifference(existing,signal)<1e-6,`${definition.id} did not use its declared shared profile.`);
  else renderedProfiles.set(profile,signal);
  for(const alias of definition.aliases)assert(builtInTimbre(alias)===builtInTimbre(definition.id),`${alias} changed its canonical voice.`);
  pitched++;
 }
 const distinct=[...renderedProfiles];
 for(let a=0;a<distinct.length;a++)for(let b=a+1;b<distinct.length;b++)
  assert(maxDifference(distinct[a][1],distinct[b][1])>.001,`${distinct[a][0]} and ${distinct[b][0]} are not distinct voices.`);
 const quiet=await render([{...base,velocity:50}]);
 const velocityRatio=rms(quiet,.12,.22)/rms(signals.flute,.12,.22);
 assert(Math.abs(velocityRatio-.5)<.001,'Rust velocity scaling changed.');
 const expression=await render([{...base,gain_envelope:[{offset_seconds:0,gain:.25}]}]);
 assert(Math.abs(rms(expression,.12,.22)/rms(signals.flute,.12,.22)-.25)<.001,'Rust expression gain changed.');
 const swellNote={...base,gain_envelope:[{offset_seconds:0,gain:.2},{offset_seconds:1,gain:1}]};
 const swell=await render([swellNote]),sought=await render([swellNote],.4);
 assert(rms(swell,.72,.82)>2.4*rms(swell,.12,.22),'Held-note crescendo did not reach the rendered signal.');
 assert(Math.abs(rms(sought,.12,.22)/rms(swell,.52,.62)-1)<.001,'Seek restarted the expression envelope.');
 const soughtPiano=await render([{...base,instrument:'piano',duration_seconds:2}],1);
 assert(rms(soughtPiano,.12,.22)<.7*rms(signals.piano,.12,.22),'Seek restarted the piano decay.');
 const mix=await render(['piano','flute','cello'].map(instrument=>({...base,instrument})));
 const expected=signals.piano.map((value,index)=>value+signals.flute[index]+signals.cello[index]);
 assert(maxDifference(mix,expected)<1e-6,'Concurrent part/instrument routing changed the rendered mix.');
 const tiny=await render([{...base,duration_seconds:.003}]);
 assert(rms(tiny,.02,.023)>0&&rms(tiny,.024,.09)===0,'Short-note gate or release changed.');
 return {status:'PASS',renderer:'OfflineAudioContext',sampleRate,harmonics,velocityRatio,pitchedIdentities:pitched,unpitchedIdentitiesWithoutRenderer:unpitched,synthProfiles:renderedProfiles.size,cases:['spectral differentiation','exact routes and aliases','neutral fallback','velocity and held expression','seek continuity','concurrent voices','note boundaries and short gates'],hardwareVerified:false};
}
void check().then(result=>{document.body.textContent=JSON.stringify(result);document.body.dataset.status='PASS';}).catch(error=>{document.body.textContent=JSON.stringify({status:'FAIL',error:String(error)});document.body.dataset.status='FAIL';});
