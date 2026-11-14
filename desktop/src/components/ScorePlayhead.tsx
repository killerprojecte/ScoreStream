import {useEffect,useRef,useSyncExternalStore,type RefObject} from 'react';
import type {PlaybackTransport} from '../playback/transport';
import {playheadGeometry,scoreAnchors} from '../playback/playhead';
import type {WorkbenchState} from '../app/workbench';

/** Purely visual overlay on the exact rendered source, never a second score model. */
export function ScorePlayhead({transport,state,notation,canvas}:{transport:PlaybackTransport;state:WorkbenchState;notation:RefObject<HTMLDivElement|null>;canvas:RefObject<HTMLDivElement|null>}){
 const playback=useSyncExternalStore(transport.subscribe,transport.getSnapshot),cursor=useRef<HTMLDivElement>(null),followed=useRef<Element|null>(null);
 const anchors=useRef<ReturnType<typeof scoreAnchors>>([]);
 const matching=state.phase==='ready'&&state.source===state.published?.source&&state.sequence===state.renderedSequence;
 useEffect(()=>{anchors.current=matching&&playback.plan&&notation.current?scoreAnchors(notation.current,state.renderedMap,playback.plan.event_positions):[];followed.current=null;},[matching,playback.plan,state.pages,state.renderedMap,notation]);
 useEffect(()=>{
  const update=()=>{
   const host=notation.current,node=cursor.current;
   if(!host||!node)return;
   const geometry=matching?playheadGeometry(host,anchors.current,playback.position):null;
   node.hidden=!geometry;
   if(!geometry)return;
   node.style.left=`${geometry.left}px`;node.style.top=`${geometry.top}px`;node.style.height=`${geometry.height}px`;
   if(playback.status==='playing'&&followed.current!==geometry.system&&canvas.current){
    const bounds=geometry.system.getBoundingClientRect(),viewport=canvas.current.getBoundingClientRect();
    if(bounds.top<viewport.top+12||bounds.bottom>viewport.bottom-12)canvas.current.scrollTop+=bounds.top-viewport.top-24;
    followed.current=geometry.system;
   }
  };
  update();
  const observer=new ResizeObserver(update);if(notation.current)observer.observe(notation.current);
  return()=>observer.disconnect();
 },[matching,playback,state.pages,state.zoom,notation,canvas]);
 return <div id="score-playhead" className="score-playhead" aria-hidden="true" hidden ref={cursor}/>;
}
