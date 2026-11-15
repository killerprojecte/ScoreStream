#!/usr/bin/env python3
"""Build distributable format contracts. Packaging utility, not product code."""
from pathlib import Path
import json

ROOT = Path(__file__).resolve().parents[1]
for d in ['spec','schemas','examples/sfs','examples/ast','mcp/examples']:
    (ROOT/d).mkdir(parents=True, exist_ok=True)

def write(path, value):
    p=ROOT/path
    p.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')

ID=r'^[A-Za-z][A-Za-z0-9_-]*$'
RAT=r'^-?(?:0|[1-9][0-9]*)(?:/[1-9][0-9]*)?$'
POS=r'^[1-9][0-9]*(?:/[1-9][0-9]*)?$'
NONNEG=r'^(?:0|[1-9][0-9]*)(?:/[1-9][0-9]*)?$'
PITCH=r'^[A-G](?:#{1,2}|b{1,2}|\([+-]?(?:0|[1-9][0-9]*)(?:/[1-9][0-9]*)?\))?-?(?:0|[1-9][0-9]*)$'
SCOPE=r'^(?:score|[A-Za-z][A-Za-z0-9_-]*(?:\.s[1-9][0-9]*)?)$'
REF=r'^#[A-Za-z][A-Za-z0-9_-]*$'

def obj(properties, required=None, **extra):
    return {'type':'object','properties':properties,'required':list(properties) if required is None else required,'additionalProperties':False,**extra}
def arr(items, **kw): return {'type':'array','items':items,**kw}
def ref(name): return {'$ref':'#/$defs/'+name}
def s(pattern=None, **kw):
    x={'type':'string',**kw}
    if pattern: x['pattern']=pattern
    return x

defs={
 'Identifier':s(ID), 'Rational':s(RAT,description='Canonical reduced rational string. Reduction is checked semantically.'),
 'PositiveRational':s(POS), 'NonNegativeRational':s(NONNEG), 'Pitch':s(PITCH),
 'Scope':s(SCOPE), 'Reference':s(REF),
 'Scalar':{'type':['string','integer','boolean']},
 'Attributes':{'type':'object','propertyNames':{'pattern':r'^[A-Za-z][A-Za-z0-9_.-]*$'},'additionalProperties':ref('Scalar'),'description':'Flat values. The language registry checks names, types, scopes and extension requirements. Unknown unqualified fields fail semantic validation.'},
}
metadata_text=s(description='Plain score text, never markup or a filename fallback. Optional; an empty string is explicit empty text.')
metadata_fields=['title','description','header','footer','subtitle','composer','lyricist','arranger','work_number','movement','rights','language']
defs['DocumentMetadata']=obj({**{name:metadata_text for name in metadata_fields},'authors':arr(s(minLength=1, allOf=[{'pattern':r'\S'},{'pattern':r'^[^\u0000-\u001F\u007F-\u009F\u2028\u2029]*$'}]), description='Ordered plain author names. Empty or absent means no generic author list; legacy role fields remain separate.')}, [])
defs['Part']=obj({'id':ref('Identifier'),'instrument':s(minLength=1),'staves':{'type':'integer','minimum':1}})
defs['Voice']=obj({'id':ref('Identifier'),'part':ref('Identifier'),'staff':{'type':'integer','minimum':1}})
defs['Event']=obj({'record':{'const':'event'},'voice':ref('Identifier'),
 'pitch':{'oneOf':[ref('Pitch'),{'enum':['r','_']},s(r'^u\.[A-Za-z][A-Za-z0-9_-]*$')]},
 'duration':{'oneOf':[ref('PositiveRational'),{'const':'?'}]},'attributes':ref('Attributes')},
 ['record','voice','pitch','duration','attributes'],
 allOf=[{'if':{'properties':{'duration':{'const':'?'}}},'then':{'properties':{'attributes':{'required':['kind','anchor','order'],'properties':{'kind':{'const':'grace'},'anchor':ref('Reference'),'order':{'type':'integer','not':{'const':0}}}}}}}])
defs['State']=obj({'record':{'const':'state'},'target':ref('Scope'),'attributes':ref('Attributes')})
defs['Mark']=obj({'record':{'const':'mark'},'target':{'oneOf':[ref('Scope'),ref('Reference')]},'kind':s(minLength=1),'attributes':ref('Attributes')})
defs['Relation']=obj({'record':{'const':'relation'},'kind':s(minLength=1),'id':ref('Identifier'),'phase':{'enum':['start','end','continue']},'anchor':ref('Reference'),'attributes':ref('Attributes')})
defs['Measure']=obj({'record':{'const':'measure'},'target':ref('Scope'),'id':ref('Identifier'),'length':ref('PositiveRational'),'attributes':ref('Attributes')})
defs['Record']={'oneOf':[ref(n) for n in ['Event','State','Mark','Relation','Measure']]}
defs['Frame']=obj({'time':ref('NonNegativeRational'),'records':arr(ref('Record'))})
props={'format':{'const':'ScoreStream'},'format_version':{'const':'1.1'},'document_kind':{'enum':['score','fragment']},'time_unit':{'const':'quarter'},
 'parts':arr(ref('Part')),'voices':arr(ref('Voice')),'metadata':ref('DocumentMetadata'),'required_extensions':arr(s(minLength=1),uniqueItems=True),
 'frames':arr(ref('Frame')),'end':ref('NonNegativeRational'),'span':ref('PositiveRational')}
schema={'$schema':'https://json-schema.org/draft/2020-12/schema','$id':'urn:scorestream:schema:ast:1.1',
 'title':'ScoreStream 1.1 parsed core AST','description':'Validates parsed structure, not the raw SFS text. A semantic validator is still required.',
 **obj(props,['format','format_version','document_kind','time_unit','parts','voices','metadata','required_extensions','frames','end']),
 '$defs':defs,
 'allOf':[{'if':{'properties':{'document_kind':{'const':'fragment'}}},'then':{'required':['span'],'properties':{'parts':{'maxItems':0},'voices':{'maxItems':0}}},'else':{'not':{'required':['span']}}}]}
write('schemas/scorestream-ast.schema.json',schema)

records=[
 {'prefix':'!score','syntax':'!score 1.1 unit=q','purpose':'Start a complete score; declare parts and voices before frames.'},
 {'prefix':'!fragment','syntax':'!fragment 1.1 unit=q span=4','purpose':'Start a bounded local-time replacement. Voice bindings come from the edit request; no part/voice declarations inside.'},
 {'prefix':'!part','syntax':'!part p instrument=piano staves=2','purpose':'Declare a part and its staff count.'},
 {'prefix':'!voice','syntax':'!voice pr part=p staff=1','purpose':'Declare a stable voice alias and its default display staff.'},
 {'prefix':'!meta','syntax':'!meta title="Example" authors=["A. Author","B. Author"] description="Study" header="Rehearsal" footer="Draft"','purpose':'Optional plain score text; never a filename fallback or markup. Ordered authors use a JSON string array; legacy composer/lyricist/arranger remain distinct roles. Declare before frames; repeated fields fail.'},
 {'prefix':'!requires','syntax':'!requires vendor.feature@1','purpose':'Declare a mandatory extension. Unknown mandatory semantics cannot be executed as core.'},
 {'prefix':'@','syntax':'@1/3','purpose':'Open a shared-score-time frame; quarter-note unit; exact rational.'},
 {'prefix':'event','syntax':'pr C#4:1 id=n1 w=4','purpose':'One event per line; duration is explicit; no pitch or duration inheritance.'},
 {'prefix':'=','syntax':'= p.s1 meter=4/4 key=0 clef=G2','purpose':'Persistent assignment to an exact scope. No implicit state inheritance.'},
 {'prefix':'+','syntax':'+ #n1 lyric verse=1 text="Spring"','purpose':'Attach intrinsic notation or a point mark; analytical prose belongs to the separate future annotation store.'},
 {'prefix':'~','syntax':'~ tie t1 start=#n1 sound=true print=true','purpose':'Start/continue/end an identified relation; crossing and nested relations are allowed.'},
 {'prefix':'|','syntax':'| p pm1 len=4 label="1"','purpose':'A stable measure ID and actual duration; display label is not identity.'},
 {'prefix':'.','syntax':'.','purpose':'Commit the complete frame atomically; bare EOF never commits.'},
 {'prefix':'!end','syntax':'!end at=4','purpose':'End a closed document at a declared score position; do not truncate active notes.'},
]
attrs={
 'event':{
  'id':{'type':'identifier','meaning':'Stable referenced event identity.'},
  'staff':{'type':'positive_integer','meaning':'This event only; does not change voice default.'},
  'chord':{'type':'identifier','meaning':'Membership in a notated chord; members keep independent durations. First member in declared member order is the head.'},
  'w':{'type':'note_type','values':['1','2','4','8','16','32','64','128','256','512','1024','breve','long','maxima'],'meaning':'Written note type; independent of actual duration.'},
  'dots':{'type':'nonnegative_integer','meaning':'Written augmentation dots.'},
  'tm':{'type':'positive_integer:positive_integer','meaning':'Actual:normal time modification, such as 3:2. Tuplet graphics are separate relations.'},
  'tm_normal_type':{'type':'note_type','values':['1','2','4','8','16','32','64','128','256','512','1024','breve','long','maxima'],'meaning':'Optional explicitly notated normal note type; maps to written.time_modification.normal_type in typed edits.'},
  'tm_normal_dots':{'type':'nonnegative_integer','meaning':'Optional explicitly notated normal dots; maps to written.time_modification.normal_dots. Requires tm.'},
  'kind':{'type':'enum','values':['note','cue','grace'],'meaning':'Ordinary note is the default. Grace requires duration=?, anchor and nonzero order.'},
  'anchor':{'type':'event_reference','meaning':'Principal note for a grace event; # prefix required.'},
  'order':{'type':'nonzero_integer','meaning':'Grace order relative to principal: negative before, positive after; unique within anchor/group.'},
  'visible':{'type':'boolean','meaning':'Intrinsic printed visibility; a hidden rest is still a rest.'},
  'stem':{'type':'enum','values':['up','down','none','double'],'meaning':'Written stem attribute.'},
  'notehead':{'type':'registered_string','meaning':'Registered notehead name; unknown names require declared vocabulary support.'},
  'slash':{'type':'boolean','meaning':'Grace slash notation.'}
 },
 'state':{
  'tempo':{'scope':['score'],'syntax':'q:120 or q.:60','meaning':'Tempo unit and rate; printed metronome mark is separate.'},
  'meter':{'scope':['staff'],'syntax':'4/4, 6/8, 3+2/8, none','meaning':'Meter is not elapsed duration. More complex signatures use a registered extension.'},
  'key':{'scope':['staff'],'syntax':'signed integer fifths count or unset','meaning':'Traditional key signature, not inferred tonality.'},
  'clef':{'scope':['staff'],'syntax':'G2, F4, C3, percussion, TAB','meaning':'Registered clef token. Other variants require registered fields/extensions.'},
  'staff':{'scope':['voice'],'syntax':'positive integer','meaning':'Change the voice default staff from this position.'},
  'instrument':{'scope':['part'],'syntax':'instrument ID','meaning':'Change a declared instrument identity.'},
  'transpose_chromatic':{'scope':['part','staff'],'syntax':'rational','meaning':'Chromatic written-to-sounding interval.'},
  'transpose_diatonic':{'scope':['part','staff'],'syntax':'integer','meaning':'Diatonic written-to-sounding interval; retain separately.'},
  'transpose_octaves':{'scope':['part','staff'],'syntax':'integer','meaning':'Additional octave transposition.'},
  'sound_dynamics':{'scope':['part','staff'],'syntax':'nonnegative rational percent','meaning':'Explicit numeric playback dynamics as a percentage of MIDI-relative forte 90; separate from printed dynamics. Exact storage; no label substitution.'}
 },
 'marks':{
  'accidental':['value','visible','cautionary','parentheses','editorial'],
  'articulation':['value','placement'], 'dynamic':['value','placement','sound'],
  'lyric':['verse','text','syllabic','language'], 'words':['text','placement','language'],
  'metronome':['beat','bpm','text'], 'harmony':['root','kind','bass','text']
 },
 'relations':{
  'tie':{'attributes':['sound','print','placement'],'end_semantics':'The next written note head; sounding continuity must be validated.'},
  'slur':{'attributes':['placement','line_type'],'end_semantics':'The linked terminal event; may cross other slurs.'},
  'tuplet':{'attributes':['actual','normal','bracket','show_number'],'end_semantics':'Last member inclusive of its duration; per-note tm remains independent.'},
  'beam':{'attributes':['level'],'end_semantics':'Ordered written member group.'},
  'crescendo':{'attributes':['placement'],'end_semantics':'Anchored span; performance interpretation is explicit.'},
  'diminuendo':{'attributes':['placement'],'end_semantics':'Anchored span; performance interpretation is explicit.'},
  'pedal':{'attributes':['value','line','sign'],'end_semantics':'Pedal span/control anchors; partial pedals require an appropriate registered model.'},
  'octave':{'attributes':['octaves','print','sound'],'end_semantics':'Written/sounding octave span.'},
  'lyric_extend':{'attributes':['verse'],'end_semantics':'Lyric extension ending at an explicit event.'}
 }
}
# Detailed field definitions complement the compact attribute-name lists.
# Keeping names in one registry lets the future Rust implementation generate
# language help and semantic validation from the same versioned source.
attrs['metadata']={name:{'type':'string','meaning':'Optional plain score text; no markup interpretation. Empty strings are retained. Control characters other than tab/LF/CR are preserved with a portability warning; exporters must diagnose unsupported text, not silently discard it.'} for name in metadata_fields}
attrs['metadata']['title']['meaning']='Explicit score title. Never inferred from a filename, application name, header or description.'
attrs['metadata']['description']['meaning']='Catalog/editor description; not automatically printed as notation or a page title.'
attrs['metadata']['header']['meaning']='Optional running page-header text, independent of the title; no markup or template expansion.'
attrs['metadata']['footer']['meaning']='Optional running page-footer text; no markup or template expansion.'
attrs['metadata']['authors']={'type':'string_list','meaning':'Ordered plain names. JSON string array; horizontal whitespace allowed inside the array. Nonblank single-line names without control characters. Empty or absent means none. Do not infer roles or merge with composer/lyricist/arranger.'}
attrs['measure']={'label':{'type':'string','meaning':'Display label, never identity'},'implicit':{'type':'boolean','default':False}}
attrs['common_mark_attributes']={'id':{'type':'identifier','meaning':'Stable text mark identity; map to opaque object ID in API responses'}}
for fields in attrs['marks'].values():
    fields.append('id')
placement={'type':'enum','values':['above','below']}
boolean={'type':'boolean'}
text_field={'type':'string'}
attrs['mark_fields']={
 'accidental':{'targets':['pitched_event'],'required':['value'],'fields':{'value':{'type':'enum','values':['natural','sharp','flat','double-sharp','flat-flat','quarter-sharp','quarter-flat']},'visible':boolean,'cautionary':boolean,'parentheses':boolean,'editorial':boolean}},
 'articulation':{'targets':['event'],'required':['value'],'fields':{'value':{'type':'enum','values':['staccato','staccatissimo','tenuto','accent','strong-accent','breath-mark']},'placement':placement}},
 'dynamic':{'targets':['scope','event'],'required':['value'],'fields':{'value':{'type':'enum','values':['pppp','ppp','pp','p','mp','mf','f','ff','fff','ffff','sf','sfz','fp']},'placement':placement,'sound':{'type':'boolean','default':True,'meaning':'False preserves printed dynamics without assigning performer-policy loudness.'}}},
 'lyric':{'targets':['event'],'required':['verse','text'],'fields':{'verse':{'type':'positive_integer'},'text':text_field,'syllabic':{'type':'enum','values':['single','begin','middle','end']},'language':text_field}},
 'words':{'targets':['scope','event'],'required':['text'],'fields':{'text':text_field,'placement':placement,'language':text_field}},
 'metronome':{'targets':['scope','event'],'required':['beat','bpm'],'fields':{'beat':{'type':'beat_unit','values':['w','h','q','e','s','w.','h.','q.','e.','s.']},'bpm':{'type':'positive_rational'},'text':text_field}},
 'harmony':{'targets':['scope','event'],'required':['root','kind'],'fields':{'root':{'type':'spelled_pitch_class'},'kind':{'type':'enum','values':['major','minor','dominant','major-seventh','minor-seventh','diminished','augmented']},'bass':{'type':'spelled_pitch_class'},'text':text_field}}
}
relation_fields={
 'tie':{'sound':{**boolean,'default':True},'print':{**boolean,'default':True},'placement':placement},
 'slur':{'placement':placement,'line_type':{'type':'enum','values':['solid','dashed','dotted']}},
 'tuplet':{'actual':{'type':'positive_integer'},'normal':{'type':'positive_integer'},'bracket':boolean,'show_number':{'type':'enum','values':['actual','both','none']}},
 'beam':{'level':{'type':'positive_integer'}},
 'crescendo':{'placement':placement},'diminuendo':{'placement':placement},
 'pedal':{'value':{'type':'rational','minimum':'0','maximum':'1','phase':'per_anchor'},'line':boolean,'sign':boolean},
 'octave':{'octaves':{'type':'nonzero_integer'},'print':boolean,'sound':boolean},
 'lyric_extend':{'verse':{'type':'positive_integer'}}
}
required_relation_fields={'tuplet':['actual','normal'],'beam':['level'],'octave':['octaves'],'lyric_extend':['verse']}
for name, fields in relation_fields.items():
    attrs['relations'][name]['fields']=fields
    attrs['relations'][name]['required_on_start']=required_relation_fields.get(name,[])
attrs['state']['tempo']['syntax']='w|h|q|e|s, optionally dotted, then colon and positive rational BPM; e.g. q:120, q.:60'
attrs['state']['clef']['syntax']='G1..G2, F3..F5, C1..C5, percussion, TAB'
attrs['event']['notehead']['values']=['normal','x','diamond','triangle','slash','none']
attrs['event']['notehead']['meaning']='Core registered notehead token; other glyphs need a registered extension.'
rules=[
 ('S001','Every frame is explicitly closed by a single dot line before a new frame or !end.'),
 ('S002','Core frame times strictly increase. Equal time belongs to the same frame; multi-clock semantics require an extension.'),
 ('S003','Timing rationals are reduced with a positive denominator. Ordinary notes, rests and spacers have positive duration.'),
 ('S004','Aliases, object references, staff numbers and required extensions must resolve; IDs are unique in their declared namespace. Part and voice aliases are disjoint; score is reserved. External opaque object IDs use returned text-ID mappings.'),
 ('S005','Pitch spelling is authoritative. F#4 is not a request to print an accidental. No duplicate authoritative MIDI/spelling fields.'),
 ('S006','Missing voice rows mean no new event. Sustained notes derive from intervals; r is a rest and _ is an invisible spacer.'),
 ('S007','Voice identity is independent from staff. An event staff override affects only that event.'),
 ('S008','Chord membership is explicit; simultaneous notes in different voices are not silently merged. Chord member lengths may differ.'),
 ('S009','Written type/dots, per-note time modification and visible tuplet relations remain separate.'),
 ('S010','State scopes and types follow the registry. Conflicting simultaneous assignments without explicit ordering fail.'),
 ('S011','Relations resolve after reading the whole frame; continuations at an extracted boundary require supplied context.'),
 ('S012','A complete core score ends no earlier than all measured event tails and declared measure ends.'),
 ('S013','Fragments use local [0,span) onset coordinates, end=span, external voice bindings and no global state/measure declarations.'),
 ('S014','A preserve-duration region replacement must cover its declared voice intervals using notes, rests or spacers and cannot silently alter crossing relations.'),
 ('S015','Analytical comments/effect descriptions are independent annotation records, never intrinsic score marks unless intentionally authored notation.'),
 ('S016','Unknown unqualified fields are errors. Namespaced content must be declared and is marked interpreted or preserved-only.'),
 ('S017','Core coverage does not imply complete MusicXML semantic support. Conversion reports separately track structure, interpreted meaning, preserved payload and target limitations.'),
 ('S018','A change to an immutable revision is a new transaction. Stale edit or visual selection references are never silently retargeted.'),
]
basic='''!score 1.1 unit=q
!part p instrument=piano staves=2
!part v instrument=violin staves=1
!voice pr part=p staff=1
!voice pl part=p staff=2
!voice vn part=v staff=1
@0
= score tempo=q:120
= p.s1 meter=4/4 key=0 clef=G2
= p.s2 meter=4/4 key=0 clef=F4
= v.s1 meter=4/4 key=0 clef=G2
| p pm1 len=4 label="1"
| v vm1 len=4 label="1"
pr C4:1 id=n1 chord=c1
pr E4:1 id=n2 chord=c1
pr G4:1 id=n3 chord=c1
pl C3:4 id=n4
vn C5:2 id=n5
.
@1
pr D4:1 id=n6 chord=c2
pr F4:1 id=n7 chord=c2
pr A4:1 id=n8 chord=c2
.
@2
pr E4:2 id=n9 chord=c3
pr G4:2 id=n10 chord=c3
pr B4:2 id=n11 chord=c3
vn D5:2 id=n12
.
!end at=4
'''
triplet='''!score 1.1 unit=q
!part v instrument=violin staves=1
!voice vn part=v staff=1
@0
= v.s1 meter=4/4 key=0 clef=G2
| v vm1 len=4 label="1"
vn C5:1/3 id=a w=8 tm=3:2
~ tuplet u1 start=#a actual=3 normal=2
.
@1/3
vn D5:1/3 id=b w=8 tm=3:2
.
@2/3
vn E5:1/3 id=c w=8 tm=3:2
~ tuplet u1 end=#c
.
@1
vn r:2 id=r1 w=2
.
@3
vn F5:1 id=d w=4
~ tie t1 start=#d sound=true print=true
.
@4
| v vm2 len=4 label="2"
vn F5:1 id=e w=4
~ tie t1 end=#e
.
@5
vn r:3 id=r2 w=2 dots=1
.
!end at=8
'''
fragment='''!fragment 1.1 unit=q span=4
@0
mel C5:1 id=f1
.
@1
mel D5:1 id=f2
.
@2
mel E5:2 id=f3
.
!end at=4
'''
grace='''!score 1.1 unit=q
!part p instrument=piano staves=2
!part d instrument=drums staves=1
!voice pr part=p staff=1
!voice dr part=d staff=1
@0
= p.s1 meter=4/4 key=0 clef=G2
= p.s2 meter=4/4 key=0 clef=F4
= d.s1 meter=4/4 key=0 clef=percussion
| p pm1 len=4 label="1"
| d dm1 len=4 label="1"
pr D5:? id=g1 kind=grace anchor=#a order=-1 w=8 slash=true
pr C5:2 id=a w=2
+ #a articulation value=staccato
dr u.snare:1 id=hit1
.
@1
dr r:3 id=rest1 w=2 dots=1
.
@2
pr C4:2 id=b staff=2 w=2
.
!end at=4
'''
metadata='!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n!meta title="春日练习" description="两位作者的短练习曲。" authors=["林青", "Alex Chen"]\n!meta header="排练版" footer="仅供学习 · 2026"\n@0\n= score tempo=q:100\n= p.s1 meter=4/4 key=0 clef=G2\n| p m1 len=4 label="1"\nv C4:2 id=a w=2\n.\n@2\nv E4:2 id=b w=2\n.\n!end at=4\n'
examples=[('document-metadata',metadata),('ensemble',basic),('tuplets-and-ties',triplet),('replacement-fragment',fragment),('grace-percussion-cross-staff',grace)]
for name,body in examples: (ROOT/f'examples/sfs/{name}.sfs').write_text(body)
guide={
 'format':'ScoreStream','format_version':'1.1','status':'design-contract; no product implementation bundled',
 'profile':'core-measured','text_media_type':'text/plain; charset=utf-8',
 'text_extension':'.sfs','package_extension':'.sfp',
 'purpose':'Learn ScoreStream notation syntax before reading, generating or editing a score. This is the music-format standard, not the MCP tool-call schema.',
 'authoritative_files':{'grammar':'spec/scorestream.ebnf','language_descriptor':'spec/scorestream-language.json','parsed_ast_schema':'schemas/scorestream-ast.schema.json','agent_reference':'spec/agent-reference.md'},
 'core_shape':'A header, declarations, increasing @time frames, flat records, a dot to commit each frame, and !end at=<position>.',
 'lexical':{'encoding':'UTF-8','newline':'LF; readers may normalize CRLF','comments':'Whole lines beginning with ; after optional whitespace; no inline comments','string':'JSON-quoted string; JSON escapes; no physical newline inside a token','whitespace':'One or more spaces or tabs separate fields; canonical writer uses one space','numbers':'Attribute type comes from the registry. Musical rational values remain strings in the AST. No duration or pitch inheritance.'},
 'defaults':{'event_kind':'note for pitched or unpitched events','event_staff':'voice current declared/default staff','dots':0,'tm':'1:1 when absent; notation can remain unspecified','visibility':True,'no_voice_row':'no new event, not a rest','omitted_state':'unchanged; initially unset unless an application profile explicitly supplies a reported default','event_id':'optional only in authoring; committed canonical output assigns IDs and returns the mapping','written_type':'unspecified if w absent; rendering/export may choose a documented spelling and must report inference'},
 'scope_rules':{'score':['tempo'],'part':['instrument','transpose_chromatic','transpose_diatonic','transpose_octaves','sound_dynamics'],'staff':['meter','key','clef','transpose_chromatic','transpose_diatonic','transpose_octaves','sound_dynamics'],'voice':['staff'],'collision':'Part and voice aliases must be disjoint and cannot be score','precedence':'No generic inheritance. Staff transposition tuple replaces the part tuple; absent staff tuple uses part. Assign tuples atomically. Numeric dynamics: staff overrides part; sound=false is display-only.'},
 'record_order':'Frame records are collected before resolution. State applies at the frame position independent of line order; conflicting assignments fail. Source event order is retained within chord/grace groups. All event references are resolved after collection.',
 'additional_constraints':[
  'sound_dynamics is an exact nonnegative percentage of forte velocity 90. The bounded numeric playback profile rejects mixtures with sounding symbolic dynamics, hairpins, or attack-changing accents; it never substitutes printed labels.',
  'Transposition assignments contain chromatic, diatonic and octaves together; absent part tuple means zero, absent staff tuple uses the part tuple.',
  'tm_normal_type and tm_normal_dots require tm; actual measured duration and imported written rhythm remain distinct even if inconsistent, with a diagnostic.',
  'Measure targets are part/staff only. Labels are strings; actual length may differ from the nominal meter.',
  'Event-attached marks and relation endpoints occur in the anchor onset frame; frame record order does not affect resolution.',
  'Fragments carry local event IDs, no declarations/state/measure mutations, and no new required extensions; only external voice bindings and supplied context apply.',
  'Unknown extension interpretation must never be inferred from raw-source preservation.'
 ],
 'pitch':{'canonical':'Written pitch spelling','examples':['C4','C#4','Db4','C##4','C(+1/2)4'],'derived':['semitone number','relative interval','scale degree'],'accidental_mark_is_separate':True},
 'time':{'unit':'quarter note = 1','encoding':'reduced rational string','examples':['1','1/2','1/3','3/2'],'floating_point_timing':False,'frame_interval_rule':'events start at @t; their score intervals are [t,t+duration)'},
 'records':records,'vocabulary':attrs,'semantic_rules':[{'id':k,'rule':v} for k,v in rules],
 'extensions':{'required_declaration':'!requires namespace@version','unknown_required_behavior':'preserve for inspection if possible, reject semantic operations requiring that extension','unknown_unqualified_fields':'error','future_domains':['independent local clocks','unmeasured or graphic notation','registered uncommon notation']},
 'editing':{'fragment_header':'!fragment 1.1 unit=q span=<length>','coordinates':'local [0,span); bind aliases externally','accepted_replacement_policy':'preserve','cross_boundary_default':'reject unless an explicit supported relation operation resolves it','ripple':'not accepted in this core contract','identity':'use returned score_id, revision_id, selection_id and object IDs'},
 'examples':[{'name':n,'file':f'examples/sfs/{n}.sfs','source':b} for n,b in examples],
 'validation_boundary':'JSON Schema checks the parsed AST only. The lexer/parser enforces EBNF; the semantic validator enforces timing, references, scope, relation and conversion constraints.',
 'mcp_resources':{'reference':'scorestream://spec/1.1/agent-reference','grammar':'scorestream://spec/1.1/grammar','language':'scorestream://spec/1.1/language','ast_schema':'scorestream://spec/1.1/ast-schema'},
 'next_actions':['Inspect the target score and pin its revision.','Read a region and keep its frozen selection and boundary context.','Return a valid bounded fragment or typed domain operations.','Apply with expected_revision_id and idempotency_key; re-read after a conflict.'],
}
write('spec/scorestream-language.json',guide)

descriptor_schema={'$schema':'https://json-schema.org/draft/2020-12/schema','$id':'urn:scorestream:schema:format-reference:1.1','title':'Agent-facing ScoreStream language reference descriptor',
 'type':'object','required':['format','format_version','profile','records','vocabulary','semantic_rules','examples','validation_boundary','mcp_resources'],
 'properties':{'format':{'const':'ScoreStream'},'format_version':{'const':'1.1'},'profile':{'const':'core-measured'},
  'records':arr(obj({'prefix':s(),'syntax':s(),'purpose':s()})),
  'semantic_rules':arr(obj({'id':s(r'^S[0-9]{3}$'),'rule':s()})),
  'examples':arr(obj({'name':s(),'file':s(),'source':s()})),
  'vocabulary':{'type':'object'},'validation_boundary':s(),'mcp_resources':{'type':'object'}},'additionalProperties':True}
write('schemas/format-reference.schema.json',descriptor_schema)
quick={k:v for k,v in guide.items() if k not in ['vocabulary','examples','authoritative_files']}
quick['examples']=[next(example for example in guide['examples'] if example['name']==name) for name in ['ensemble','replacement-fragment']]
quick['common_attributes']={k:attrs['event'][k] for k in ['id','staff','chord','w','dots','tm','kind','anchor','order']}
write('mcp/examples/score_get_format.structured.json',{'ok':True,'data':quick})
print('Wrote grammar companion schemas, language descriptor and complete SFS fixtures.')
