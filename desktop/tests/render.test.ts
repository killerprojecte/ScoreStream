import {describe,it,expect} from 'vitest';
import {Q,rhythm} from '../src/render/exact';
import {project} from '../src/render/project';
import {LatestOnly} from '../src/render/scheduler';
import type {View,RenderRequest,RenderResult} from '../src/types';
const fixture=():View=>({valid:true,diagnostics:[],source:'backend-only',document:{frames:[]},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false,parts:[{id:'p',instrument:'Piano & strings',staves:1}],voices:[{id:'v',part:'p',staff:1}],states:[{time:'0',scope:'p.s1',values:{meter:'4/4',clef:'G2',key:0}}],measures:[{id:'m',part:'p',time:'0',length:'4',label:'1'}],events:[{id:'n1',voice:'v',pitch:'C#4',time:'0',duration:'1',staff:1,chord:null,written_duration:null,dots:0,source_line:5},{id:'n2',voice:'v',pitch:'r',time:'1',duration:'3',staff:1,chord:null,written_duration:null,dots:1,source_line:6}]});
describe('bounded typed MEI projection',()=>{
 it('uses exact rhythm without floating point',()=>{expect(new Q('90071992547409930000000001/3').sub(new Q('90071992547409930000000000/3')).eq(new Q('1/3'))).toBe(true);expect(rhythm(new Q('3/2'))).toEqual({dur:4,dots:1});expect(()=>rhythm(new Q('1/3'))).toThrow();});
 it('escapes labels and keeps stable event identity',()=>{const result=project(fixture());expect(result.mei).toContain('Piano &amp; strings');expect(result.mei).toContain('accid="s"');expect(result.map).toEqual({'ss-e0':'n1','ss-e1':'n2'});});
 it('refuses invalid snapshots and unsupported pitches',()=>{const v=fixture();v.valid=false;expect(()=>project(v)).toThrow('Rust-validated');v.valid=true;v.events[0].pitch='C(+1/3)4';expect(()=>project(v)).toThrow('not supported');});
 it('refuses missing measures instead of dropping events',()=>{const v=fixture();v.events[0].time='4';expect(()=>project(v)).toThrow('outside');});
 it('refuses unequal chord rhythms',()=>{const v=fixture();v.events[0].chord='c';v.events[1]={...v.events[1],pitch:'E4',time:'0',duration:'2',chord:'c'};expect(()=>project(v)).toThrow('Unequal-duration');});
 it('refuses cross-bar duration instead of clipping',()=>{const v=fixture();v.events[1].duration='4';expect(()=>project(v)).toThrow('Cross-bar');});
 it('refuses state changes instead of displaying wrong clef',()=>{const v=fixture();v.states.push({time:'2',scope:'p.s1',values:{clef:'F4'}});expect(()=>project(v)).toThrow('Mid-score');});
});
describe('latest-only renderer scheduling',()=>{
 it('rejects stale results even after a newer invalid draft and bounds queue',()=>{
  const sent:RenderRequest[]=[],published:RenderResult[]=[];
  const worker:{onmessage:((e:MessageEvent<RenderResult>)=>void)|null;postMessage:(x:RenderRequest)=>void}={onmessage:null,postMessage:r=>sent.push(r)};
  const scheduler=new LatestOnly(worker as unknown as Worker,r=>published.push(r));
  const request=(id:number):RenderRequest=>({requestId:id,key:{kind:'draft',session:'s',sequence:id,hash:String(id),baseRevision:'r1',layout:'x'},mei:'',width:600,map:{}});
  const finish=(id:number)=>worker.onmessage!({data:{...request(id),pages:[],elapsed:1}} as unknown as MessageEvent<RenderResult>);
  scheduler.invalidate(1);scheduler.submit(request(1));scheduler.invalidate(2);scheduler.submit(request(2));scheduler.invalidate(3);scheduler.submit(request(3));expect(sent.map(x=>x.requestId)).toEqual([1]);finish(1);expect(sent.map(x=>x.requestId)).toEqual([1,3]);expect(published).toEqual([]);
  scheduler.invalidate(4);finish(3);expect(published).toEqual([]);
  scheduler.submit(request(4));finish(4);expect(published.map(x=>x.requestId)).toEqual([4]);
 });
});
