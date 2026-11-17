// @vitest-environment jsdom
import {act,useSyncExternalStore} from 'react';
import {createRoot,type Root} from 'react-dom/client';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {OfflineExport} from '../src/app/offline-export';
import {OfflineExportDialog} from '../src/components/OfflineExportDialog';
import {setLanguage} from '../src/i18n';
let root:Root;
beforeEach(()=>{vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT',true);setLanguage('en');const host=document.createElement('div');document.body.append(host);root=createRoot(host);});
afterEach(async()=>{await act(async()=>root.unmount());document.body.replaceChildren();vi.unstubAllGlobals();});
function Screen({model,enabled=true}:{model:OfflineExport;enabled?:boolean}){
 const state=useSyncExternalStore(model.subscribe,model.getSnapshot);
 return <OfflineExportDialog offline={{model,state,supported:true}} revision="current-revision" enabled={enabled}/>;
}
async function click(text:string){const button=[...document.querySelectorAll('button')].find(b=>b.textContent===text);expect(button).toBeDefined();await act(async()=>button!.click());}
it('keeps draft and validation reachable after backend rejection and reports incomplete choices without claiming audio',async()=>{
 const report={policy_compiled:false,unchosen_fields:['config.bank_sha256'],native_bank_verified:false,wav_created:false};
 const api={reserve:vi.fn(async()=> 'native-token'),run:vi.fn(async(_id:string,action:string)=>{if(action==='export')throw 'OFFLINE_BACKEND_UNAVAILABLE';return action==='validate'?report:{action:'draft',wav_created:false};}),cancel:vi.fn(async()=>{})};
 const model=new OfflineExport(api);model.open();await act(async()=>root.render(<Screen model={model}/>));
 await click('Use this rehearsal policy and choose files');
 expect(document.body.textContent).toContain('Policy draft and validation remain available.');
 await click('Close');await act(async()=>model.open());await click('Generate policy draft…');
 expect(api.run).toHaveBeenLastCalledWith('native-token','draft');
 expect(document.body.textContent).toContain('Incomplete policy draft saved.');
 await click('Close');await act(async()=>model.open());await click('Validate edited policy…');
 const output=document.querySelector('textarea')!;
 expect(output.readOnly).toBe(true);expect(JSON.parse(output.value)).toEqual(report);
 expect(document.body.textContent).toContain('a compiled policy is not native bank verification');
 expect(document.body.textContent).not.toContain('Verified WAV saved');
 expect(api.reserve).toHaveBeenCalledWith('current-revision');
});
it('disables all three actions for an unapplied score without reserving a native job',async()=>{
 const api={reserve:vi.fn(),run:vi.fn(),cancel:vi.fn()};const model=new OfflineExport(api);model.open();
 await act(async()=>root.render(<Screen model={model} enabled={false}/>));
 for(const text of ['Generate policy draft…','Validate edited policy…','Use this rehearsal policy and choose files'])await click(text);
 expect(api.reserve).not.toHaveBeenCalled();
});
