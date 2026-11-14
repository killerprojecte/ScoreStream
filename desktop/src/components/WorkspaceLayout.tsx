import {useEffect,useMemo,useRef,useState,type ReactNode} from 'react';
import {useGroupRef,usePanelRef,type Layout} from 'react-resizable-panels';
import {ResizableHandle,ResizablePanel,ResizablePanelGroup} from './ui/resizable';
import {ToggleGroup,ToggleGroupItem} from './ui/toggle-group';
import {t} from '../i18n';
import type {Preferences} from '../preferences-model';
import {workspaceLayout,resizePreferences} from '../app/workspace-layout';
import './workspace-layout.css';
export type CompactPane='source'|'score'|'inspector';
type Widths=Pick<Preferences,'sourceWidth'|'inspectorWidth'>;
interface Props {width:number;preferences:Preferences;compactPane:CompactPane;onCompactPaneChange:(value:CompactPane)=>void;onWidthsChange:(value:Partial<Widths>)=>void;source:ReactNode;score:ReactNode;inspector:ReactNode}
function DesktopPanels({preferences,onWidthsChange,source,score,inspector}:Props){
 const host=useRef<HTMLDivElement>(null),group=useGroupRef(),sourceRef=usePanelRef(),inspectorRef=usePanelRef();
 const [width,setWidth]=useState(0);
 const sourceVisible=preferences.view==='split',inspectorVisible=preferences.showInspector;
 useEffect(()=>{const element=host.current!;const measure=()=>setWidth(element.clientWidth);measure();const observer=new ResizeObserver(measure);observer.observe(element);return()=>observer.disconnect();},[]);
 const requested=useMemo(()=>workspaceLayout(width,preferences),[width,preferences.sourceWidth,preferences.inspectorWidth,sourceVisible,inspectorVisible]);
 useEffect(()=>{if(width>0)group.current?.setLayout(requested);},[width,requested,group]);
 function persist(_layout:Layout,meta:{isUserInteraction:boolean}){
  if(!meta.isUserInteraction)return;
  const patch=resizePreferences(preferences,sourceVisible?sourceRef.current?.getSize().inPixels:undefined,inspectorVisible?inspectorRef.current?.getSize().inPixels:undefined);
  if(Object.keys(patch).length)onWidthsChange(patch);
 }
 return <div ref={host} className="desktop-workspace-panels"><ResizablePanelGroup id="score-workspace-panels" groupRef={group} orientation="horizontal" onLayoutChanged={persist}>
  {sourceVisible&&<><ResizablePanel id="source" panelRef={sourceRef} defaultSize={preferences.sourceWidth} minSize={220} maxSize={680} groupResizeBehavior="preserve-pixel-size">{source}</ResizablePanel><ResizableHandle id="source-resize" aria-label={t('Editor width')}/></>}
  <ResizablePanel id="score" minSize={320}>{score}</ResizablePanel>
  {inspectorVisible&&<><ResizableHandle id="inspector-resize" aria-label={t('Inspector width')}/><ResizablePanel id="inspector-panel" panelRef={inspectorRef} defaultSize={preferences.inspectorWidth} minSize={240} maxSize={440} groupResizeBehavior="preserve-pixel-size">{inspector}</ResizablePanel></>}
 </ResizablePanelGroup></div>;
}
export function WorkspaceLayout(props:Props){
 const {width,compactPane,onCompactPaneChange,source,score,inspector}=props;
 if(width>850)return <main id="workspace" className="app-workspace"><DesktopPanels {...props}/></main>;
 return <main id="workspace" className="app-workspace"><div className="compact-workspace-layout">
  <nav aria-label={t('Workspace view')}><ToggleGroup className="gap-1" type="single" size="sm" spacing={1} value={compactPane} onValueChange={value=>{if(value)onCompactPaneChange(value as CompactPane);}} aria-label={t('Workspace view')}>{(['source','score','inspector'] as const).map(pane=><ToggleGroupItem key={pane} value={pane}>{t(pane==='source'?'Source':pane==='score'?'Score':'Inspector')}</ToggleGroupItem>)}</ToggleGroup></nav>
  <div hidden={compactPane!=='source'} className="compact-pane-content" role="region" aria-label={t('Source')}>{source}</div>
  <div hidden={compactPane!=='score'} className="compact-pane-content" role="region" aria-label={t('Score')}>{score}</div>
  <div hidden={compactPane!=='inspector'} className="compact-pane-content" role="region" aria-label={t('Inspector')}>{inspector}</div>
 </div></main>;
}
