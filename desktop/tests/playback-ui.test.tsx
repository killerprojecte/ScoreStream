// @vitest-environment jsdom
import {act} from 'react';
import {createRoot,type Root} from 'react-dom/client';
import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import {ScorePane} from '../src/components/ScorePane';
import {TooltipProvider} from '../src/components/ui/tooltip';
import {PlaybackTransport,isPlaybackShortcut} from '../src/playback/transport';
import {anchorIndex,playheadGeometry,scoreAnchors} from '../src/playback/playhead';
import {displayMeasures,formatTime,measureAt,measureKey,parseSeconds} from '../src/playback/timeline';
import type {PlaybackPlan} from '../src/playback/types';
import type {Workbench,WorkbenchState} from '../src/app/workbench';
import type {View} from '../src/types';
vi.mock('../src/bridge',()=>({native:true}));
const plan:PlaybackPlan={duration_seconds:4,notes:[],routes:[],warnings:[],diagnostics:[],event_positions:[{id:'n1',start_seconds:0,end_seconds:1},{id:'rest',start_seconds:1,end_seconds:2},{id:'tie-tail',start_seconds:2,end_seconds:4}],measures:[{id:'m1',part:'p',label:'A',start_seconds:0,end_seconds:1},{id:'m2',part:'p',label:'B',start_seconds:1,end_seconds:4}]};
const svg='<svg xmlns="http://www.w3.org/2000/svg"><g class="system" id="system"><g id="note"/><g id="rest"/><g id="tail"/></g></svg>';
const view={valid:true,source:'valid',document:{},events:[{id:'n1',pitch:'C4',duration:'1'},{id:'rest',pitch:'r',duration:'1'},{id:'tie-tail',pitch:'C4',duration:'2'}]} as View;
let host:HTMLDivElement,root:Root,transport:PlaybackTransport,state:WorkbenchState,controller:Workbench;
const audio={now:0,running:true,resume:vi.fn(async()=>{}),schedule:vi.fn(),silence:vi.fn(),dispose:vi.fn()};
const rect=(left:number,top:number,width:number,height:number)=>({left,top,width,height,right:left+width,bottom:top+height,x:left,y:top,toJSON:()=>({})});
async function render(){await act(async()=>root.render(<TooltipProvider><ScorePane controller={controller} state={state} showTitle={false}/></TooltipProvider>));}
async function click(selector:string){await act(async()=>{document.querySelector(selector)!.dispatchEvent(new MouseEvent('click',{bubbles:true}));});}
async function input(value:string){await act(async()=>{const el=host.querySelector('#playback-seconds') as HTMLInputElement;Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value')!.set!.call(el,value);el.dispatchEvent(new Event('input',{bubbles:true}));});}
beforeEach(async()=>{
 vi.useFakeTimers();Object.assign(globalThis,{IS_REACT_ACT_ENVIRONMENT:true});
 vi.stubGlobal('ResizeObserver',class{observe(){}disconnect(){}});
 vi.stubGlobal('CSS',{escape:(text:string)=>text});
 vi.spyOn(Element.prototype,'getBoundingClientRect').mockImplementation(function(this:Element){return this.id==='system'?rect(20,30,300,100):this.id==='note'?rect(30,50,10,20):this.id==='rest'?rect(130,50,10,20):this.id==='tail'?rect(230,50,10,20):rect(0,0,500,400);});
 host=document.createElement('div');document.body.append(host);root=createRoot(host);audio.now=0;
 transport=new PlaybackTransport({plan:vi.fn(async()=>plan)},audio);transport.setSource('valid');await transport.prepareSource();
 state={source:'valid',published:view,phase:'ready',pages:[svg],renderedMap:{note:'n1',rest:'rest',tail:'tie-tail'},sequence:1,renderedSequence:1,zoom:1,warnings:[],selected:null} as unknown as WorkbenchState;
 controller={playback:transport,setWidth:vi.fn(),select:vi.fn(),setZoom:vi.fn(),dirty:false} as unknown as Workbench;
 await render();
});
afterEach(async()=>{await act(async()=>{root.unmount();transport.dispose();});host.remove();vi.restoreAllMocks();vi.unstubAllGlobals();vi.useRealTimers();});
describe('score playback composition',()=>{
 it('uses labelled official controls, Rust duration and the actual nonuniform measure map',async()=>{
  expect(host.querySelector('#playback-time')!.textContent).toBe('0:00.0 / 0:04.0');
  expect(host.querySelector('#playback-toggle')!.tagName).toBe('BUTTON');
  expect(host.querySelector('#playback-seconds')!.getAttribute('data-slot')).toBe('input');
  expect(host.querySelector('#playback-measure')!.textContent).toContain('A');
  await act(async()=>transport.seek(1.2));
  expect(host.querySelector('#playback-measure')!.textContent).toContain('B');
 });
 it('clicks rests and tied continuation glyphs to seek using the original Rust intervals',async()=>{
  await click('#rest');expect(transport.getSnapshot().position).toBe(1);
  await click('#tail');expect(transport.getSnapshot().position).toBe(2);
  expect(controller.select).toHaveBeenLastCalledWith('tie-tail',true);
  const cursor=host.querySelector('#score-playhead') as HTMLElement;
  expect(cursor.hidden).toBe(false);expect(cursor.style.left).toBe('230px');
 });
 it('moves the score cursor with the audio clock, freezes on pause and resets on stop',async()=>{
  await click('#playback-toggle');audio.now=.5;await act(async()=>vi.advanceTimersByTime(25));
  const cursor=host.querySelector('#score-playhead') as HTMLElement;
  expect(cursor.style.left).toBe('80px');
  await click('#playback-toggle');audio.now=1.5;await act(async()=>vi.advanceTimersByTime(100));expect(cursor.style.left).toBe('80px');
  await click('#playback-stop');expect(transport.getSnapshot().position).toBe(0);expect(cursor.style.left).toBe('30px');
 });
 it('refuses seeks and hides the cursor over retained stale or mismatched notation',async()=>{
  state={...state,phase:'invalid',source:'invalid draft'};await render();await click('#rest');
  expect(transport.getSnapshot().position).toBe(0);expect((host.querySelector('#score-playhead') as HTMLElement).hidden).toBe(true);
  state={...state,phase:'ready',source:'valid',sequence:2};await render();await click('#tail');expect(transport.getSnapshot().position).toBe(0);
 });
 it('accepts exact seconds, rejects invalid values, and Escape does not seek to the discarded draft',async()=>{
  const field=host.querySelector('#playback-seconds') as HTMLInputElement;
  await input('1.25');await act(async()=>field.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true})));expect(transport.getSnapshot().position).toBe(1.25);
  await input('20');await act(async()=>field.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true})));expect(field.getAttribute('aria-invalid')).toBe('true');expect(transport.getSnapshot().position).toBe(1.25);
  await act(async()=>field.focus());await input('3');await act(async()=>field.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true})));expect(transport.getSnapshot().position).toBe(1.25);
 });
});
describe('timeline and keyboard boundaries',()=>{
 it('keeps seconds and measure boundaries accurate without assuming constant tempo or equal measures',()=>{
  const measures=displayMeasures({...plan,measures:[...plan.measures,{...plan.measures[0],id:'other',part:'q'}]});
  expect(measures).toHaveLength(2);expect(measureAt(measures,1)!.id).toBe('m2');expect(measureAt(measures,4)!.id).toBe('m2');
  expect(formatTime(59.999)).toBe('0:59.9');expect(formatTime(60)).toBe('1:00.0');expect(parseSeconds('1.25',4)).toBe(1.25);
  for(const text of ['','-1','NaN','Infinity','1e2','5'])expect(parseSeconds(text,4)).toBeNull();
 });
 it('uses one cursor for polyphony and follows renderer systems without cross-page interpolation',()=>{
  const notation=host.querySelector('#notation')!;
  const anchors=scoreAnchors(notation,state.renderedMap,plan.event_positions);
  expect(playheadGeometry(notation,anchors,.5)!.left).toBe(80);
  expect(anchorIndex([{id:'a',start_seconds:0,end_seconds:2},{id:'b',start_seconds:0,end_seconds:1},{id:'c',start_seconds:1,end_seconds:2}],.5)).toBe(0);
  expect(playheadGeometry(notation,[],2)).toBeNull();
 });
 it('assigns distinct selector keys to repeated measures while retaining written labels',()=>{
  const first={...plan.measures[0],occurrence_index:0},again={...plan.measures[0],occurrence_index:2,start_seconds:2,end_seconds:3};
  expect(measureKey(first)).not.toBe(measureKey(again));
  expect(measureAt([first,again],2)).toBe(again);
  expect(measureKey(plan.measures[0])).toBe('m1');
 });
 it('retains every performed visit to the same written SVG anchor',()=>{
  const notation=host.querySelector('#notation')!;
  const positions=[{id:'n1',occurrence_id:'occ0:n1',start_seconds:0,end_seconds:1},{id:'tie-tail',occurrence_id:'occ1:tie-tail',start_seconds:1,end_seconds:2},{id:'n1',occurrence_id:'occ2:n1',start_seconds:2,end_seconds:3}];
  const anchors=scoreAnchors(notation,{note:'n1',tail:'tie-tail'},positions);
  expect(anchors.map(a=>a.occurrence_id)).toEqual(['occ0:n1','occ1:tie-tail','occ2:n1']);
  expect(anchors[0].node).toBe(anchors[2].node);
  expect(anchorIndex(anchors,2)).toBe(2);
 });
 it('ignores empty spacer geometry in a polyphonic onset and as the next interpolation target',()=>{
  const notation=host.querySelector('#notation')!,space=document.createElementNS('http://www.w3.org/2000/svg','g');
  space.setAttribute('id','space');space.setAttribute('class','space');host.querySelector('#system')!.prepend(space);
  vi.spyOn(space,'getBoundingClientRect').mockReturnValue(rect(0,0,0,0) as DOMRect);
  const anchors=scoreAnchors(notation,{space:'spacer',...state.renderedMap},[{id:'spacer',start_seconds:0,end_seconds:1},...plan.event_positions]);
  expect(playheadGeometry(notation,anchors,0)!.left).toBe(30);
  expect(playheadGeometry(notation,anchors,.5)!.left).toBe(80);
  const silent=scoreAnchors(notation,{note:'n1',space:'spacer',tail:'tie-tail'},[{id:'spacer',start_seconds:1,end_seconds:2},...plan.event_positions]);
  expect(playheadGeometry(notation,silent,1)!.left).toBe(130);
  expect(playheadGeometry(notation,[anchors[0]],.5)).toBeNull();
 });
 it('keeps the final cursor moving through the longest simultaneous written duration',()=>{
  const notation=host.querySelector('#notation')!;
  const anchors=scoreAnchors(notation,{note:'short',tail:'long'},[{id:'short',start_seconds:0,end_seconds:1},{id:'long',start_seconds:0,end_seconds:4}]);
  expect(playheadGeometry(notation,anchors,1)!.left).toBe(101.5);
  expect(playheadGeometry(notation,anchors,4)!.left).toBe(316);
 });
 it('lets text fields, Monaco, menus and dialogs own Space while the score canvas toggles transport',()=>{
  const event=(target:Element,extra:KeyboardEventInit={})=>{const e=new KeyboardEvent('keydown',{key:' ',bubbles:true,...extra});Object.defineProperty(e,'target',{value:target});return e;};
  expect(isPlaybackShortcut(event(host.querySelector('#note')!))).toBe(true);
  for(const html of ['<input/>','<textarea/>','<button/>','<div contenteditable="true"><span/></div>','<div class="monaco-editor"><span/></div>','<div role="dialog"><span/></div>','<div role="menu"><span/></div>']){const el=document.createElement('div');el.innerHTML=html;expect(isPlaybackShortcut(event(el.querySelector('span')??el.firstElementChild!))).toBe(false);}
  for(const extra of [{repeat:true},{ctrlKey:true},{metaKey:true},{altKey:true},{shiftKey:true},{isComposing:true}])expect(isPlaybackShortcut(event(host,extra))).toBe(false);
  const prevented=event(host,{cancelable:true});prevented.preventDefault();expect(isPlaybackShortcut(prevented)).toBe(false);
 });
});
