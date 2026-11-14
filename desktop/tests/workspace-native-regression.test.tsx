// @vitest-environment jsdom
import {act,useState} from 'react';
import {createRoot} from 'react-dom/client';
import {expect,it,vi} from 'vitest';
import {Tabs,TabsList,TabsTrigger,TabsContent} from '../src/components/ui/tabs';
import {WorkspaceLayout} from '../src/components/WorkspaceLayout';
import {defaults} from '../src/preferences-model';
it('switches actual Resizable panel topology in both directions without throwing',async()=>{
 vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT',true);
 class Observer{observe(){}unobserve(){}disconnect(){}}
 vi.stubGlobal('ResizeObserver',Observer);
 vi.spyOn(HTMLElement.prototype,'clientWidth','get').mockReturnValue(1188);
 const widthOf=(element:HTMLElement):number=>{
  if(element.getAttribute('data-slot')==='resizable-handle')return element.style.display==='none'?0:1;
  if(element.hasAttribute('data-panel')){const grow=element.style.flexGrow;return grow?1186*Number(grow)/100:({source:360,score:570,'inspector-panel':256}[element.id]??0);}
  return 1188;
 };
 vi.spyOn(HTMLElement.prototype,'offsetLeft','get').mockImplementation(function(this:HTMLElement){return ({source:0,'source-resize':360,score:361,'inspector-resize':932,'inspector-panel':933}[this.id]??0);});
 vi.spyOn(HTMLElement.prototype,'offsetWidth','get').mockImplementation(function(this:HTMLElement){return widthOf(this);});
 vi.spyOn(HTMLElement.prototype,'getBoundingClientRect').mockImplementation(function(this:HTMLElement){const width=widthOf(this);return {x:0,y:0,left:0,top:0,width,height:700,right:width,bottom:700,toJSON(){return {};}};});
 const host=document.createElement('div');document.body.append(host);const errors:unknown[]=[];const widths=vi.fn();const root=createRoot(host,{onUncaughtError:error=>errors.push(error)});
 function Harness(){const [p,setP]=useState(defaults());return <Tabs value={p.view} onValueChange={view=>setP({...p,view:view as 'split'|'score'})}><TabsList><TabsTrigger id="tab-split" value="split">Split</TabsTrigger><TabsTrigger id="tab-score" value="score">Score</TabsTrigger></TabsList><button id="change-inspector" onClick={()=>setP({...p,showInspector:!p.showInspector})}>Inspector</button><TabsContent value={p.view}><WorkspaceLayout width={1188} preferences={p} compactPane="score" onCompactPaneChange={()=>{}} onWidthsChange={widths} source={<textarea defaultValue="Unsaved source"/>} score={<span>Score</span>} inspector={<span>Inspector</span>}/></TabsContent></Tabs>;}
 try{
  await act(async()=>root.render(<Harness/>));await act(async()=>new Promise(resolve=>setTimeout(resolve,35)));expect(errors).toEqual([]);
  const draft=host.querySelector('textarea')!;draft.value='Unsaved source changed';draft.setSelectionRange(3,7);
  for(const id of ['tab-score','tab-split','change-inspector','tab-score','change-inspector','tab-split']){
   await act(async()=>{const button=host.querySelector<HTMLButtonElement>('#'+id)!;if(id.startsWith('tab-'))button.dispatchEvent(new MouseEvent('mousedown',{bubbles:true,button:0}));else button.click();});
   await act(async()=>new Promise(resolve=>setTimeout(resolve,35)));
   expect(errors.map(e=>String(e))).toEqual([]);
   expect(host.querySelector('#workspace')).not.toBeNull();
   expect(host.querySelector('textarea')).toBe(draft);expect(draft.value).toBe('Unsaved source changed');expect(draft.selectionStart).toBe(3);expect(draft.selectionEnd).toBe(7);
   expect(host.querySelectorAll('[data-slot=resizable-panel]')).toHaveLength(3);
   if(id==='tab-score'){expect(draft.parentElement?.hidden).toBe(true);expect(host.querySelector<HTMLElement>('#source')?.style.flexGrow).toBe('0');}
   if(id==='tab-split'){expect(draft.parentElement?.hidden).toBe(false);expect(Number(host.querySelector<HTMLElement>('#source')?.style.flexGrow)).toBeGreaterThan(0);}
   expect(widths).not.toHaveBeenCalled();
  }
  await act(async()=>{const handle=host.querySelector<HTMLElement>('#source-resize')!;handle.focus();handle.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true}));});
  expect(widths).toHaveBeenLastCalledWith({sourceWidth:419});
 }finally{await act(async()=>root.unmount());host.remove();vi.restoreAllMocks();vi.unstubAllGlobals();}
});
