import {describe,it,expect,vi} from 'vitest';
import {defaults,parsePreferences,PreferenceStore,PREFERENCES_KEY} from '../src/preferences-model';
import {setLanguage,t} from '../src/i18n';
describe('React preference state',()=>{
 it('defaults from locale and rejects malformed storage',()=>{expect(defaults('zh-HK').language).toBe('zh-CN');expect(parsePreferences('bad','en')).toEqual(defaults('en'));expect(parsePreferences('{"version":2}')).toEqual(defaults());});
 it('bounds controls and widths without accepting unknown values',()=>{expect(parsePreferences(JSON.stringify({...defaults(),fontSize:100,sourceWidth:-9,inspectorWidth:999,theme:'unexpected',language:'xx'}))).toMatchObject({fontSize:22,sourceWidth:220,inspectorWidth:440,theme:'system',language:'en'});});
 it('persists only preference data and reports storage failure without blocking changes',()=>{const storage={getItem:vi.fn(()=>null),setItem:vi.fn()},store=new PreferenceStore(storage);store.update({theme:'dark'});expect(storage.setItem).toHaveBeenCalledWith(PREFERENCES_KEY,JSON.stringify(store.value));expect(store.saved).toBe(true);storage.setItem.mockImplementation(()=>{throw new Error('quota');});store.update({fontSize:16});expect(store.value.fontSize).toBe(16);expect(store.saved).toBe(false);});
 it('notifies React subscribers on every accepted preference change',()=>{const store=new PreferenceStore(null);store.onChange=vi.fn();store.update({wrap:true});expect(store.onChange).toHaveBeenCalledWith(expect.objectContaining({wrap:true}));});
 it('switches translated labels and exact values without rebuilding editor state',()=>{setLanguage('zh-CN');expect(t('Saved {name}. Session history is not persisted.',{name:'音乐.sfs'})).toContain('音乐.sfs');expect(t('Source')).toBe('源码');setLanguage('en');expect(t('Source')).toBe('Source');});
});
