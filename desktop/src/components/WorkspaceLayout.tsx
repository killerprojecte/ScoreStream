import {useEffect,useMemo,useRef,useState,type ReactNode} from 'react';
import {useGroupRef,type Layout} from 'react-resizable-panels';
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
 const host=useRef<HTMLDivElement>(null),group=useGroupRef();
 const [width,setWidth]=useState(0);
 const sourceVisible=preferences.view==='split',inspectorVisible=preferences.showInspector;
 useEffect(()=>{const element=host.current!;const measure=()=>setWidth(element.clientWidth);measure();const observer=new ResizeObserver(measure);observer.observe(element);return()=>observer.disconnect();},[]);
 const requested=useMemo(()=>({source:0,'inspector-panel':0,...workspaceLayout(width,preferences)}),[width,preferences.sourceWidth,preferences.inspectorWidth,sourceVisible,inspectorVisible]);
 useEffect(()=>{
  if(width<=0)return;
  // The official Group publishes updated collapse constraints after child registration.
  // Apply pixels on the next frame, after that commit, and cancel stale view requests.
  const frame=requestAnimationFrame(()=>group.current?.setLayout(requested));
  return()=>cancelAnimationFrame(frame);
 },[width,requested,group]);
 function persist(layout:Layout,meta:{isUserInteraction:boolean}){
  if(!meta.isUserInteraction)return;
  // Callback layout is authoritative; DOM-backed getSize can still reflect the
  // previous paint during a keyboard event. Preserve pixels without that stale read.
  const available=Math.max(1,width-Number(sourceVisible)-Number(inspectorVisible));
  const patch=resizePreferences(preferences,sourceVisible?layout.source/100*available:undefined,inspectorVisible?layout['inspector-panel']/100*available:undefined);
  if(Object.keys(patch).length)onWidthsChange(patch);
 }
 return <div ref={host} className="desktop-workspace-panels"><ResizablePanelGroup id="score-workspace-panels" groupRef={group} orientation="horizontal" onLayoutChanged={persist}>
  <ResizablePanel id="source" defaultSize={preferences.sourceWidth} minSize={220} maxSize={680} collapsible={!sourceVisible} collapsedSize={0} disabled={!sourceVisible} groupResizeBehavior="preserve-pixel-size"><div hidden={!sourceVisible} className="workspace-panel-content">{source}</div></ResizablePanel><ResizableHandle id="source-resize" disabled={!sourceVisible} style={sourceVisible?undefined:{display:'none'}} aria-label={t('Editor width')}/>
  <ResizablePanel id="score" minSize={320}>{score}</ResizablePanel>
  <ResizableHandle id="inspector-resize" disabled={!inspectorVisible} style={inspectorVisible?undefined:{display:'none'}} aria-label={t('Inspector width')}/><ResizablePanel id="inspector-panel" defaultSize={preferences.inspectorWidth} minSize={240} maxSize={440} collapsible={!inspectorVisible} collapsedSize={0} disabled={!inspectorVisible} groupResizeBehavior="preserve-pixel-size"><div hidden={!inspectorVisible} className="workspace-panel-content">{inspector}</div></ResizablePanel>
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
