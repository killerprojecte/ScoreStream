import type {EventPosition} from './types';
export interface ScoreAnchor extends EventPosition {node:Element;system:Element}
export function scoreAnchors(host:Element,map:Record<string,string>,positions:EventPosition[]):ScoreAnchor[]{
 const events=new Map(positions.map(position=>[position.id,position]));
 const anchors:ScoreAnchor[]=[];
 for(const [svgId,id] of Object.entries(map)){
  const position=events.get(id),node=host.querySelector(`[id="${CSS.escape(svgId)}"]`);
  if(!position||!node)continue;
  anchors.push({...position,node,system:node.closest('.system')??node.closest('svg')??node});
 }
 return anchors.sort((a,b)=>a.start_seconds-b.start_seconds);
}
export function anchorIndex(anchors:EventPosition[],seconds:number):number {
 let lo=0,hi=anchors.length;
 while(lo<hi){const mid=(lo+hi)>>>1;if(anchors[mid].start_seconds<=seconds)lo=mid+1;else hi=mid;}
 let index=Math.max(0,lo-1);
 while(index>0&&anchors[index-1].start_seconds===anchors[index].start_seconds)index--;
 return index;
}
export function playheadGeometry(host:Element,anchors:ScoreAnchor[],seconds:number){
 if(!anchors.length)return null;
 const usable=(index:number)=>{const box=anchors[index].node.getBoundingClientRect();return box.width>0||box.height>0;};
 let index=anchorIndex(anchors,seconds);
 // Verovio represents spacers as empty SVG groups. Prefer a drawable voice at
 // the same onset, then bridge silent positions between real engraved anchors.
 const simultaneous=anchors[index].start_seconds;
 let candidate=index;
 while(candidate<anchors.length&&anchors[candidate].start_seconds===simultaneous&&!usable(candidate))candidate++;
 if(candidate<anchors.length&&anchors[candidate].start_seconds===simultaneous)index=candidate;
 else{
  while(index>=0&&!usable(index))index--;
  if(index<0){index=candidate;while(index<anchors.length&&!usable(index))index++;}
 }
 if(index>=anchors.length)return null;
 const anchor=anchors[index],rect=anchor.node.getBoundingClientRect(),system=anchor.system.getBoundingClientRect(),origin=host.getBoundingClientRect();
 if(!system.height)return null;
 let next=index+1;
 while(next<anchors.length&&(anchors[next].start_seconds===anchor.start_seconds||!usable(next)))next++;
 const target=anchors[next],sameSystem=target?.system===anchor.system;
 const endTime=target?.start_seconds??anchors.reduce((end,item)=>Math.max(end,item.end_seconds),anchor.end_seconds);
 const endX=sameSystem?target.node.getBoundingClientRect().left:Math.max(rect.right,system.right-4);
 const fraction=endTime>anchor.start_seconds?Math.max(0,Math.min(1,(seconds-anchor.start_seconds)/(endTime-anchor.start_seconds))):0;
 return {left:rect.left-origin.left+Math.max(0,endX-rect.left)*fraction,top:system.top-origin.top,height:system.height,system:anchor.system};
}
