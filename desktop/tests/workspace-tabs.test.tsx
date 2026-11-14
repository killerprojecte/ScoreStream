// @vitest-environment jsdom
import {act,useState} from 'react';
import {createRoot} from 'react-dom/client';
import {describe,it,expect,vi} from 'vitest';
import {WorkspaceLayout} from '../src/components/WorkspaceLayout';
import {Tabs,TabsList,TabsTrigger,TabsContent} from '../src/components/ui/tabs';
import {defaults} from '../src/preferences-model';
import {workspaceLayout,resizePreferences} from '../src/app/workspace-layout';
describe('official workspace tabs and pixel preference bridge',()=>{
 it('retains legacy pixel widths and uses percentages only at the group boundary',()=>{
  const p=defaults();const layout=workspaceLayout(1188,p),available=1186;
  expect(layout.source!/100*available).toBeCloseTo(360);expect(layout['inspector-panel']!/100*available).toBeCloseTo(256);
  expect(Object.values(layout).reduce((a,b)=>a+b,0)).toBeCloseTo(100);
  const narrow=workspaceLayout(851,{...p,sourceWidth:680,inspectorWidth:440});
  expect(narrow.score/100*849).toBeGreaterThanOrEqual(319.999);
  expect(narrow.source!/100*849).toBeGreaterThanOrEqual(220);
  expect(workspaceLayout(1188,{...p,view:'score',showInspector:false})).toEqual({score:100});
  expect(resizePreferences(p,400.2,undefined)).toEqual({sourceWidth:400});
  expect(resizePreferences(p,999,10)).toEqual({sourceWidth:680,inspectorWidth:240});
  expect(resizePreferences(p,NaN,Infinity)).toEqual({});
 });
 it('links shared layout tabs to the active panel',async()=>{
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT',true);const host=document.createElement('div');document.body.append(host);const root=createRoot(host);
  function Harness(){const [view,setView]=useState('split');return <Tabs value={view} onValueChange={setView}><TabsList aria-label="Workspace view"><TabsTrigger value="split">Edit</TabsTrigger><TabsTrigger value="score">Preview</TabsTrigger></TabsList><TabsContent value={view}><div>Stable workspace</div></TabsContent></Tabs>;}
  try{await act(async()=>root.render(<Harness/>));const tabs=host.querySelectorAll<HTMLButtonElement>('[role=tab]');expect(tabs).toHaveLength(2);expect(host.querySelector('[role=tabpanel]')?.id).toBe(tabs[0].getAttribute('aria-controls'));await act(async()=>tabs[1].dispatchEvent(new MouseEvent('mousedown',{bubbles:true,button:0})));expect(tabs[1].getAttribute('aria-selected')).toBe('true');expect(host.querySelector('[role=tabpanel]')?.id).toBe(tabs[1].getAttribute('aria-controls'));}finally{await act(async()=>root.unmount());host.remove();vi.unstubAllGlobals();}
 });
 it('retains the existing compact ToggleGroup without unmounting source drafts',async()=>{
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT',true);const host=document.createElement('div');document.body.append(host);const root=createRoot(host);let mounts=0;
  function Draft(){mounts++;return <span data-draft="kept">Unsaved source</span>;}
  function Harness(){const [pane,setPane]=useState<'source'|'score'|'inspector'>('source');return <WorkspaceLayout width={600} preferences={defaults()} compactPane={pane} onCompactPaneChange={setPane} onWidthsChange={()=>{}} source={<Draft/>} score={<span>Score</span>} inspector={<span>Inspector</span>}/>;}
  try{await act(async()=>root.render(<Harness/>));const draft=host.querySelector('[data-draft]');const controls=host.querySelectorAll<HTMLButtonElement>('[data-slot=toggle-group-item]');expect(controls).toHaveLength(3);await act(async()=>controls[1].click());expect(controls[1].getAttribute('aria-checked')).toBe('true');expect(host.querySelector('[role=region][aria-label=Source]')?.hasAttribute('hidden')).toBe(true);expect(host.querySelector('[data-draft]')).toBe(draft);expect(mounts).toBeGreaterThan(0);}finally{await act(async()=>root.unmount());host.remove();vi.unstubAllGlobals();}
 });
});
