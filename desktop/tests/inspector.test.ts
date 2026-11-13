import {describe,it,expect,vi,afterEach} from 'vitest';
import {noteLocation,performanceTempo} from '../src/inspector/format';
import {inspectorMarkup,Inspector} from '../src/inspector/view';
import type {View,ScoreEvent} from '../src/types';
const note:ScoreEvent={id:'n1',voice:'rh',pitch:'E4',time:'1',duration:'1',staff:1,chord:null,written_duration:null,dots:0,source_line:11};
const view=():View=>({valid:true,source:'',diagnostics:[],events:[note],parts:[{id:'p',instrument:'piano',staves:2}],voices:[{id:'rh',part:'p',staff:1}],states:[{time:'0',scope:'score',values:{tempo:'q.:104/3'}},{time:'0',scope:'p.s1',values:{meter:'6/8'}}],measures:[{id:'m1',part:'p',time:'0',length:'3',label:'1'}],document:{frames:[]},revision_id:'r1',dirty:false,draft_seq:0,can_undo:false,can_redo:false});
describe('contextual inspector',()=>{
 it('keeps the exact tempo rate and names the beat unit without fake music glyphs',()=>{expect(performanceTempo(view())).toEqual({rate:'104/3',unit:'Dotted quarter note'});});
 it('handles unset tempo without a placeholder mark',()=>{const v=view();v.states=[];expect(performanceTempo(v)).toBeNull();});
 it('finds tempo even when score states are in separate records',()=>{const v=view();v.states.unshift({time:'0',scope:'score',values:{key:0}});expect(performanceTempo(v)?.rate).toBe('104/3');});
 it('uses meter beats, not quarter offsets, for note position',()=>{expect(noteLocation(note,view())).toMatchObject({part:'piano',measure:'1',beat:'3',staff:'1',line:'11'});});
 it('does not inherit a score-wide meter over an explicit staff meter',()=>{const v=view();v.states.push({time:'0',scope:'score',values:{meter:'4/4'}});expect(noteLocation(note,v).beat).toBe('3');});
 it('keeps fractional beat positions exact',()=>{expect(noteLocation({...note,time:'1/3'},view()).beat).toBe('5/3');});
 it('keeps controls labelled and their source details optional',()=>{expect(inspectorMarkup).toContain('for="pitch-input"');expect(inspectorMarkup).toContain('for="duration-input"');expect(inspectorMarkup).toContain('<details class="inspector-group source-details">');expect(inspectorMarkup).not.toContain('selection-symbol');expect(inspectorMarkup).not.toContain('event-pitch');});
});

// Minimal field harness tests inspector state, not browser layout/rendering.
class Field extends EventTarget {
 value='';disabled=false;hidden=false;textContent='';attributes=new Map<string,string>();
 setAttribute(name:string,value:string){this.attributes.set(name,value);}
 removeAttribute(name:string){this.attributes.delete(name);}
}
function fields(){
 const elements=new Map<string,Field>();
 const get=(id:string)=>{if(!elements.has(id))elements.set(id,new Field());return elements.get(id)!;};
 vi.stubGlobal('document',{getElementById:get});return get;
}
afterEach(()=>vi.unstubAllGlobals());
describe('inspector field state',()=>{
 it('preserves the other staged property while accepting one committed value',()=>{
  const get=fields(),inspector=new Inspector(()=>{});
  inspector.updateSelection(note,view(),true,false);
  get('pitch-input').value='F4';get('duration-input').value='3/2';
  inspector.updateSelection(note,view(),false,false);
  inspector.acceptProperty('pitch','F4');inspector.updateSelection({...note,pitch:'F4'},view(),true,false);
  expect(get('pitch-input').value).toBe('F4');expect(get('duration-input').value).toBe('3/2');
  expect(get('set-pitch').disabled).toBe(true);expect(get('set-duration').disabled).toBe(false);
 });
 it('resets fields only when moving to another event',()=>{
  const get=fields(),inspector=new Inspector(()=>{});inspector.updateSelection(note,view(),true,false);
  get('pitch-input').value='F4';inspector.updateSelection({...note,id:'n2',pitch:'A4'},view(),true,false);
  expect(get('pitch-input').value).toBe('A4');expect(get('set-pitch').disabled).toBe(true);
 });
 it('handles Escape locally and Enter only for editable changed values',()=>{
  const get=fields(),commit=vi.fn(),inspector=new Inspector(commit);inspector.updateSelection(note,view(),true,false);
  const key=(key:string)=>get('pitch-input').dispatchEvent(Object.assign(new Event('keydown',{cancelable:true}),{key}));
  key('Enter');expect(commit).not.toHaveBeenCalled();get('pitch-input').value='F4';key('Enter');expect(commit).toHaveBeenCalledWith('pitch');
  inspector.showError('pitch','Invalid');key('Escape');expect(get('pitch-input').value).toBe('E4');expect(get('inspector-feedback').hidden).toBe(true);
  inspector.updateSelection(note,view(),false,true);get('pitch-input').value='F4';commit.mockClear();key('Enter');expect(commit).not.toHaveBeenCalled();
 });
});
