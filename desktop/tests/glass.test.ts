// @vitest-environment jsdom
import {describe,it,expect,vi} from 'vitest';
import {resolveGlassPolicy,installGlassPolicy} from '../src/glass/glass-policy.mjs';
import {parsePreferences,PreferenceStore} from '../src/preferences-model';

describe('portable glass policy',()=>{
 it('resolves theme independently from platform and keeps layered material when blur is unsupported',()=>{
  expect(resolveGlassPolicy({}, {dark:true},{},{backdropFilter:false})).toMatchObject({theme:'dark',material:'glass',quality:'layered'});
  expect(resolveGlassPolicy({theme:'light',quality:'layered'},{dark:true})).toMatchObject({theme:'light',quality:'layered'});
  expect(resolveGlassPolicy({theme:'dark',material:'solid'})).toMatchObject({theme:'dark',material:'solid'});
 });
 it.each([{reduceTransparency:true},{forcedColors:true}])('system media %o takes priority over glass',media=>{
  expect(resolveGlassPolicy({material:'glass'},media)).toMatchObject({material:'solid'});
 });
 it('combines native accessibility signals and never ignores media when native says false',()=>{
  expect(resolveGlassPolicy({}, {reduceMotion:true,increaseContrast:true},{reduceTransparency:true,reduceMotion:false})).toMatchObject({material:'solid',reducedMotion:true,contrast:'more'});
  expect(resolveGlassPolicy({}, {}, {}, {compositorUnusable:true})).toMatchObject({material:'solid'});
 });
 it('tracks live queries, prefixed support, portal root state and removes listeners on disposal',()=>{
  const queries=new Map<string,{matches:boolean;addEventListener:ReturnType<typeof vi.fn>;removeEventListener:ReturnType<typeof vi.fn>}>();
  const env={matchMedia:(query:string)=>{const value={matches:false,addEventListener:vi.fn(),removeEventListener:vi.fn()};queries.set(query,value);return value;},CSS:{supports:(name:string)=>name==='-webkit-backdrop-filter'}} as unknown as typeof globalThis;
  const root=document.createElement('html'),policy=installGlassPolicy(root,{},env);
  expect(root.dataset.glassQuality).toBe('blur');
  const dark=queries.get('(prefers-color-scheme: dark)')!;dark.matches=true;dark.addEventListener.mock.calls[0][1]();expect(root.dataset.theme).toBe('dark');
  policy.updatePreferences({quality:'layered',material:'solid'});expect(root.dataset.material).toBe('solid');expect(root.dataset.glassQuality).toBe('layered');
  policy.updateNativeAccessibility({reduceMotion:true});expect(root.dataset.reducedMotion).toBe('true');
  policy.dispose();for(const query of queries.values())expect(query.removeEventListener).toHaveBeenCalledWith('change',query.addEventListener.mock.calls[0][1]);
  expect(policy.updatePreferences({material:'glass'})).toBeUndefined();expect(root.dataset.material).toBe('solid');
 });
 it('treats missing CSS capability APIs as layered fallback',()=>{
  const root=document.createElement('html'),policy=installGlassPolicy(root,{},{} as typeof globalThis);expect(root.dataset.glassQuality).toBe('layered');policy.dispose();
 });
});
describe('glass preference migration',()=>{
 it('migrates existing version-one records and rejects unknown material values',()=>{
  expect(parsePreferences('{"version":1,"theme":"dark","fontSize":15}')).toMatchObject({theme:'dark',fontSize:15,material:'glass',glassQuality:'auto'});
  expect(parsePreferences('{"version":1,"material":"transparent","glassQuality":"GPU"}')).toMatchObject({material:'glass',glassQuality:'auto'});
 });
 it('persists material independently from appearance and keeps controls usable if storage fails',()=>{
  const storage={getItem:()=>null,setItem:vi.fn()},store=new PreferenceStore(storage);store.update({theme:'dark',material:'solid',glassQuality:'layered'});expect(parsePreferences(storage.setItem.mock.calls[0][1])).toMatchObject({theme:'dark',material:'solid',glassQuality:'layered'});
  storage.setItem.mockImplementation(()=>{throw new Error('full')});store.update({material:'glass'});expect(store.value.material).toBe('glass');expect(store.saved).toBe(false);
 });
});
