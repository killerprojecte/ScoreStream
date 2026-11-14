import {describe,it,expect} from 'vitest';
import {noteLocation,performanceTempo} from '../src/inspector/format';
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
});

