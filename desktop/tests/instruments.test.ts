import {describe,expect,it} from 'vitest';
import {readFileSync} from 'node:fs';
import {INSTRUMENT_CATALOG,resolveInstrument} from '../src/instruments/catalog';
import {BUILT_IN_PROFILES,builtInProfile,builtInTimbre} from '../src/playback/timbres';

describe('format-neutral orchestral instrument catalog',()=>{
 it('matches the one core catalog exactly, with no independent desktop identities',()=>{
  const source=JSON.parse(readFileSync(new URL('../../crates/scorestream-core/src/instruments/catalog.json',import.meta.url),'utf8'));
  expect(INSTRUMENT_CATALOG).toEqual(source);
 });
 it('resolves unique exact aliases while preserving arbitrary custom instrument atoms',()=>{
  const identifiers=new Set<string>();
  for(const definition of INSTRUMENT_CATALOG){
   for(const id of [definition.id,...definition.aliases]){
    expect(identifiers.has(id)).toBe(false);identifiers.add(id);
    expect(id).toMatch(/^[a-z_-]+$/);expect(resolveInstrument(id)).toBe(definition);
   }
  }
  for(const id of ['Piano','unknown','constructor','__proto__','toString',' piccolo','flute.extra','drums'])expect(resolveInstrument(id)).toBeNull();
 });
 it('makes one honest support decision for each identity and covers every pitched family',()=>{
  expect(Object.keys(BUILT_IN_PROFILES).sort()).toEqual(INSTRUMENT_CATALOG.map(x=>x.id).sort());
  for(const definition of INSTRUMENT_CATALOG){
   if(definition.pitch_kind==='pitched')expect(builtInTimbre(definition.id)).not.toBeNull();
   else {expect(builtInProfile(definition.id)).toBeNull();expect(builtInTimbre(definition.id)).toBeNull();}
   for(const alias of definition.aliases)expect(builtInTimbre(alias)).toBe(builtInTimbre(definition.id));
  }
 });
 it('includes genuine profiles for the Beethoven Fifth finale additions and tuned timpani',()=>{
  for(const id of ['piccolo','contrabassoon','trombone','timpani']){
   expect(resolveInstrument(id)?.pitch_kind).toBe('pitched');expect(builtInProfile(id)).toBe(id);
  }
  expect(builtInTimbre('piccolo')).not.toBe(builtInTimbre('flute'));
  expect(builtInTimbre('contrabassoon')).not.toBe(builtInTimbre('bassoon'));
  expect(builtInTimbre('timpani')!.envelope.length).toBeGreaterThan(0);
 });
 it('keeps existing voices and reports deliberately shared profiles explicitly',()=>{
  expect(builtInTimbre('piano')!.harmonics).toEqual([1,.45,.2,.12,.05,.02]);
  expect(builtInTimbre('flute')!.harmonics).toEqual([1,.08,.04,.01]);
  for(const id of ['violin','cello','strings'])expect(builtInTimbre(id)!.harmonics).toEqual([1,.7,.45,.32,.22,.16,.1,.08]);
  expect(builtInProfile('viola')).toBe('strings');expect(builtInProfile('double-bass')).toBe('strings');
  expect(builtInProfile('bass_trombone')).toBe('trombone');expect(builtInProfile('crotales')).toBe('glockenspiel');
 });
 it('keeps every profile bounded, deterministic and independent of output pitch',()=>{
  for(const definition of INSTRUMENT_CATALOG){
   const timbre=builtInTimbre(definition.id);if(!timbre)continue;
   expect(timbre.harmonics.length).toBeGreaterThan(0);expect(timbre.harmonics.length).toBeLessThanOrEqual(16);
   expect(timbre.harmonics[0]).toBe(1);
   for(const harmonic of timbre.harmonics)expect(Number.isFinite(harmonic)&&harmonic>=0&&harmonic<=1).toBe(true);
   let time=-1;
   for(const point of timbre.envelope){expect(point.offset_seconds).toBeGreaterThan(time);time=point.offset_seconds;expect(point.gain).toBeGreaterThanOrEqual(0);expect(point.gain).toBeLessThanOrEqual(1);}
  }
 });
});
