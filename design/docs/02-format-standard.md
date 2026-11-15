# ScoreStream Format Standard

**Proposed version:** 1.1. **Initial semantic profile:** `core-measured`. **File extension:** `.sfs`. **Encoding:** UTF-8. **Time unit:** quarter note = 1.

This chapter is normative within this design proposal. The EBNF specifies text structure; the language descriptor specifies machine-readable rules and vocabulary; the AST schema specifies the parsed JSON representation. A conforming implementation needs all three layers. The package QA utility covers the supplied fixtures and a stated subset of these semantics; it is not the production parser.

## 1. Model and notation

A score consists of declarations followed by increasing shared-time frames. All events beginning at one time are written vertically within the same frame, regardless of instrument. An event's duration is explicit. Missing rows do not represent rests. A dot on its own line closes the frame.

```text
!score 1.1 unit=q
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
```

The bass `n4` spans `[0,4)`. It remains active at frames 1 and 2 without being repeated. At frame 0, three piano chord members, the bass event, and the violin event start simultaneously. This is temporal alignment, not a fixed-width table with an empty column for every instrument.

## 2. Lexical rules

The grammar is `spec/scorestream.ebnf`. Readers accept LF and may normalize CRLF to LF. Canonical writers emit LF, one space between fields, no trailing whitespace, and a final newline. Whitespace inside a quoted string is content. Numeric formatting is locale-independent.

| Element | Syntax and rule |
|---|---|
| Text identifier | ASCII letter followed by ASCII letters, digits, `_`, or `-` |
| Exact rational | Integer or `numerator/denominator`; denominator positive; canonical fraction reduced |
| Positive duration | Strictly positive rational, for example `1`, `1/2`, `3/2` |
| Quoted string | JSON string escaping, such as `text="dolce"`; physical newlines inside a token are forbidden |
| Atom | Unquoted token without whitespace, quotes, or control characters |
| Boolean | `true` or `false`, only where a field is registered as boolean |
| Event reference | `#` followed by a text event identifier |
| Comment | Whole line beginning with `;` after optional whitespace |
| Attribute | `name=value`; duplicate names in the same record are invalid |

There are no inline comments. In particular, `#` is part of pitches and event references. A parser must use a lexer that respects quoted strings; splitting on spaces or `#` is insufficient. JSON strings must decode to valid Unicode scalar values, including correct surrogate-pair handling.

The registry determines an attribute's type. `w=4` has a note-type token, `staff=2` has an integer, and `text="4"` has a string. Do not globally guess types from token spelling. Rational values remain strings in JSON AST/API representations so that clients do not round them through floating-point numbers.

Timing values such as `2/4`, `0/3`, and `-0` are not canonical. Canonical forms are `1/2` and `0`. Readers may accept unreduced lexical fractions only in a normalization mode that reports the normalization; strict conformance validates reduced form. A slash denominator of zero and a negative measured duration are errors.

## 3. Headers, declarations, and identity

| Record | Meaning |
|---|---|
| `!score 1.1 unit=q` | Start a complete score |
| `!part p instrument=piano staves=2 display_name="Piano solo"` | Declare part alias, instrument key, and staff count; optional JSON-quoted plain display label is independent of sound routing |
| `!voice pr part=p staff=1` | Declare stable voice alias and its initial display staff |
| `!meta title="Example" composer="Example Author"` | Intrinsic score metadata |
| `!requires vendor.feature@1` | Require a versioned semantic extension |
| `!end at=4` | Close the document at a declared score position |

Declarations occur before frames. A voice's part must exist; its staff number is one-based and must be within that part. A complete core score must contain at least one declared part and voice when it contains events. Empty documents may be used only if the application explicitly supports an empty-score state.

Part and voice aliases must be disjoint. `score` is reserved and cannot be either. Unsuffixed scope tokens are therefore unambiguous. Staff scope `p.s2` means staff 2 of part `p`; it is not a voice alias and not a persistent external staff ID.

Event IDs are unique across the score's event namespace; measure IDs are unique across its measure namespace; relation IDs are unique across its relation namespace; chord IDs are unique across its chord namespace. References are typed by their field. A relation ID cannot be reused for another relation kind. A chord ID denotes one attack group, not every recurrence of a harmony.

External object IDs returned by the API are opaque. The application may use UUIDs or another representation that is not a legal text token. It must return and preserve mappings between external IDs and SFS text IDs/aliases. Agents must use those mappings rather than copying an arbitrary external ID into source text.

Authoring a new complete score may omit event IDs. Import assigns them and returns canonical text with IDs. Canonical reads and bounded edit fragments carry IDs. Original source IDs are provenance values and must not be confused with newly assigned ScoreStream identities.

`instrument` is a symbolic instrument identity or catalog key. It is not an implicit MIDI program, transposition, tuning, staff type, or percussion map. Those properties must be explicitly represented in supported domain fields/extensions. No convention silently turns `clarinet` into a B-flat transposition.

All metadata fields are optional. String-valued fields are `title`, `description`, `header`, `footer`, `subtitle`, `composer`, `lyricist`, `arranger`, `work_number`, `movement`, `rights`, and `language`. `authors` is an ordered JSON array of plain names, for example `!meta title="Spring" authors=["Lin", "Alex"]`. Authors must be nonblank, single-line strings without control characters. Existing role fields stay independent; never infer a role from the author list or duplicate a composer into it.

`title` is the explicit score title, never a filename or product-name fallback. `description` is catalog/editor context and is not automatically printed. `header` and `footer` are deliberate running page text, distinct from title and with no template interpolation. Every text field is plain Unicode text, not raw HTML, XML, MEI or formatting instructions. Consumers escape for their output format. Strings may be empty. Legacy JSON-escaped text remains readable: control characters other than tab/LF/CR are preserved with a portability warning, and renderers/exporters must diagnose characters they cannot represent instead of silently dropping them. Missing fields remain absent; an empty `authors` list means none and is omitted canonically. There is no placeholder title.

Metadata must precede frames and is forbidden in fragments. Multiple `!meta` lines are accepted, but each field may occur only once, even if identical. Canonical output uses one `!meta` line with lexically ordered fields, JSON-escaped strings and a compact author array preserving order. Per-field source ranges are retained internally for diagnostics, not inserted into the wire AST. This is an explicitly approved development-stage extension of **1.1**, not a new format version. Analytical prose about musical passages remains separate.

## 4. Frames and stream completion

A frame starts with `@` followed immediately by a nonnegative rational score position. Core frame positions strictly increase. All records at equal time belong to a single frame. The first frame may start after zero, but that does not create implicit rests before it.

Within a frame, records are collected before reference resolution. State assignments apply at that position independent of whether their text line precedes a note. Conflicting assignments to the same state property and scope at the same time are errors. Input event order is retained for chord-member and grace-group ordering; a writer must not sort those members by pitch and destroy their order.

The closing `.` is the parser's frame boundary. It allows an incremental parser to publish a complete frame. It is **not** a durable database commit or permission to display half of a user transaction. An incomplete frame at EOF is an incomplete draft and cannot be committed. The explicit `!end` closes a complete document and checks that the declared end is no earlier than all measured event tails and measure ends.

A streaming application can validate completed frames incrementally, while retaining unresolved forward references until the required boundary. A replacement operation or full-score import commits only after its complete document, required context, and semantic validation succeed. User-facing previews distinguish incomplete drafts from committed revisions.

Canonical writers use a deterministic order: declarations in stable part/voice order; frames by rational time; measure records; scoped state records; events in declared voice order while retaining chord/grace order; point marks; relation records. Relations and event-attached marks are resolved using identities, not their position in that textual ordering.

## 5. Events, pitch, rests, and duration

The basic event record is:

```text
voice pitch:duration [attribute=value ...]
```

| Example | Meaning |
|---|---|
| `vn C4:1 id=n1` | Written middle C, one quarter-note unit |
| `vn C#4:1/2 id=n2` | C-sharp in octave 4, half a quarter unit |
| `vn Db4:1/2 id=n3` | D-flat, preserving different spelling |
| `vn C(+1/2)4:1 id=n4` | C raised by half a semitone |
| `vn F(-1/3)5:1 id=n5` | Exact third-semitone lowering |
| `vn r:1 id=r1` | A notated rest |
| `vn _:1 id=s1` | An invisible temporal spacer |
| `dr u.snare:1 id=d1` | Unpitched event using a declared/known percussion identity |

Written pitch is `(step, alteration, octave)`. Plain accidentals include `#`, `##`, `b`, and `bb`. The parenthesized exact alteration supports microtones beyond those abbreviations. An omitted alteration is zero. Canonical writers prefer ordinary accidental spellings for integral changes of one or two semitones, and use parentheses for other changes. Signed octaves are permitted; range limits belong to a target capability profile, not an implicit MIDI clamp.

A numeric pitch projection is derived and must declare whether it is written or sounding pitch. It does not replace the written spelling. A printed courtesy accidental is attached separately, for example:

```text
+ #n2 accidental value=sharp visible=true cautionary=true
```

That mark does not change `n2`'s pitch. If an edit changes pitch, its accidental policy must either preserve a still-consistent mark or recompute it using notation context. Silent divergence between pitch and a printed glyph is not acceptable.

Ordinary notes, cue notes, rests, and spacers have strictly positive measured duration. The interval of an event at `t` with duration `d` is `[t,t+d)`. Grace events have `?` duration and their own rules below. An absent voice row means no new event; it does not mean a rest, tie, silence, or a repeated note.

Common event attributes also include boolean `visible` (default true), `stem=up|down|none|double`, and `notehead=normal|x|diamond|triangle|slash|none`. Other notehead glyphs need a registered extension. `slash` is boolean and applies to grace notation. Hidden events retain their musical identity and duration.

### 5.1 Written rhythm

| Field | Type | Meaning |
|---|---|---|
| `w` | Note-type token | Written note type: `1`, `2`, `4`, `8`, through `1024`, or `breve`, `long`, `maxima` |
| `dots` | Nonnegative integer | Written augmentation dots; default 0 |
| `tm` | Positive integer pair | Actual:normal time modification, such as `3:2` |
| `tm_normal_type` | Note-type token | Explicit normal note type in a time modification |
| `tm_normal_dots` | Nonnegative integer | Explicit normal augmentation dots in a time modification |

`tm_normal_type` and `tm_normal_dots` require `tm`. They map to `written.time_modification.normal_type` and `.normal_dots` in typed edit operations. A visible tuplet bracket and number are a relation, separate from these per-note values.

For an ordinary written eighth-note triplet with default normal notation, `w=8 tm=3:2` corresponds to `(4/8) × (2/3) = 1/3` quarter units. Written dots multiply the undotted value by `2 - 1/2^dots`. Explicit normal notation must also be retained and interpreted according to its feature semantics; do not discard it merely because a simple ratio seems sufficient.

Actual measured duration remains authoritative for score time. Imported written fields and measured duration may disagree. Preserve both and emit a diagnostic; do not silently rewrite the source. An omitted `w` means unspecified spelling. A renderer/exporter may choose a documented spelling and report that inference, but cannot claim imported engraving fidelity for a spelling that was never supplied.

### 5.2 Chords and voice identity

Use a shared `chord` ID on simultaneous notes in one voice. Chord members must have the same onset and voice, but may have different durations. Their source/member order is retained; the first member is the serialized head. A target adapter applies its own representability constraints and reports any projection. A group cannot mix ordinary notes with rests or spacers.

Different voices that start together are separate voices unless explicitly grouped by a supported notation relationship. Do not turn vertical simultaneity into an arbitrary chord. One voice may contain successive attacks while an earlier chord member still sustains; active-note queries use intervals rather than assuming a single active event per voice.

Voice identity is independent of display staff. `staff=2` changes only the event's display staff within its part. It does not rename the voice or change later events. `= pr staff=2` changes that voice's default staff from the frame position onward. The default and event override are explicit different mechanisms.

### 5.3 Grace and cue events

```text
pr D5:? id=g1 kind=grace anchor=#a order=-1 w=8 slash=true
pr C5:2 id=a w=2
```

The grace event and its principal anchor occur in the same score-time frame. Its order is nonzero: negative values precede the principal and positive values follow it in the grace ordering. Ordering values are unique per anchor attack group; members of an explicit grace chord may share the group's order. Grace notation consumes no measured score duration. Allocation of performed time belongs to a later performance policy.

Grace anchors must resolve to an appropriate non-grace event. Grace chains, rest anchors, or unspecified ordering are rejected unless a declared extension defines their semantics. A slash is a printed grace attribute, not a universal instruction to shorten the note by a fixed number of milliseconds.

`kind=cue` marks a measured cue event. Its printed size and eventual performance policy must be kept distinct; the core does not silently route every cue note to playback. The ordinary event kind is `note` when omitted for pitched/unpitched events.

## 6. State and measures

State records use `= scope field=value ...`. They persist until changed at that same semantic scope. Omission leaves state unchanged; initially unspecified fields remain unset. There is no universal score-to-part-to-staff inheritance system.

| State | Allowed scope | Core value |
|---|---|---|
| `tempo` | `score` | Beat token and positive rational rate, such as `q:120` or `q.:60` |
| `meter` | Staff | `4/4`, `6/8`, additive `3+2/8`, or `none` |
| `key` | Staff | Signed fifths count or `unset` |
| `clef` | Staff | Common G/F/C line token, `percussion`, or `TAB` |
| `staff` | Voice | One-based default staff number within the voice's part |
| `instrument` | Part | Explicit instrument identity |
| `transpose_chromatic` | Part or staff | Exact chromatic written-to-sounding interval |
| `transpose_diatonic` | Part or staff | Diatonic interval as integer |
| `transpose_octaves` | Part or staff | Additional octave displacement as integer |

Transposition is a tuple. When assigned, all three fields must be present; zero is explicit. A staff tuple overrides the part tuple as a whole; if absent, the part tuple applies. Missing part transposition means zero. This is a specific musical rule, not generic scope inheritance. Clef octave displacements, specialized tunings, and richer transposition models require a registered extension until explicitly standardized.

Core tempo beat tokens are `w`, `h`, `q`, `e`, and `s`, optionally dotted. They mean whole, half, quarter, eighth, and sixteenth note respectively. The rate is beats per minute of that unit. Thus `q.:60` is 60 dotted quarters per minute. Printed metronome text is a separate mark; performance tempo curves and metric modulation need declared semantics beyond a scalar assignment.

Core clefs permit G on lines 1–2, F on lines 3–5, C on lines 1–5, `percussion`, and `TAB`. Meter `none` removes a conventional signature; it does not make measured duration disappear or introduce an independent clock. More complex mixed signatures and genuinely unmeasured notation require a feature module.

A measure record has the form:

```text
| p pm1 len=4 label="1"
```

Its start is the frame position; `len` is actual elapsed score duration. `label` is a display string, not identity. `implicit=true` can identify a pickup/implicit measure. The target is a part or a staff; `score` and voice targets are invalid for measures. A part measure applies to that part's staffs. If staff-local measures are used for a polymetric profile, the same staff must not have conflicting overlapping part/staff measure ownership.

Meter and actual measure length are separate. A pickup may have `len=1` under `4/4`. A core measured profile uses the one global rational clock even when parts have different measure boundaries. Independent free-running local clocks are a future extension. Imported empty, irregular, hidden, or non-controlling measures require explicit coverage checks; do not force every measure to equal the displayed time signature.

## 7. Intrinsic marks and relationships

A point mark uses `+ target kind attributes`. The target is an event reference or an allowed musical scope. Event-attached marks appear in the anchor's onset frame. Scope-attached marks occur at the current frame time. Optional `id` gives a stable mark identity in canonical text; committed reads must preserve the application's mark mapping.

| Kind | Principal fields | Allowed targets |
|---|---|---|
| `accidental` | `value`, `visible`, `cautionary`, `parentheses`, `editorial` | Pitched event |
| `articulation` | `value`, `placement` | Event |
| `dynamic` | `value`, `placement` | Scope or event |
| `lyric` | `verse`, `text`, `syllabic`, `language` | Event |
| `words` | `text`, `placement`, `language` | Scope or event |
| `metronome` | `beat`, `bpm`, `text` | Scope or event |
| `harmony` | `root`, `kind`, `bass`, `text` | Scope or event |

Text fields are strings. `verse` is a positive integer. Boolean fields use boolean tokens. `placement` is `above` or `below`. `syllabic` is `single`, `begin`, `middle`, or `end`. Core accidental values are `natural`, `sharp`, `flat`, `double-sharp`, `flat-flat`, `quarter-sharp`, and `quarter-flat`; other glyphs require a registered vocabulary. Core articulations include `staccato`, `staccatissimo`, `tenuto`, `accent`, `strong-accent`, and `breath-mark`. Dynamic values include ordinary `pppp` through `ffff`, `mp`, `mf`, `sfz`, and `fp`; special text can be an explicit `words` mark without pretending to have defined performance semantics.

Harmony roots/basses are spelled pitch classes. The initial harmony vocabulary includes `major`, `minor`, `dominant`, `major-seventh`, `minor-seventh`, `diminished`, and `augmented`. Complex chord degrees, figured bass, and alternative systems require a richer registered module. A text label can be retained without being claimed as interpreted harmony.

Relationships use an explicit ID and event anchors:

```text
~ tie t1 start=#a sound=true print=true
~ tie t1 end=#b
```

| Kind | Important attributes | Span meaning |
|---|---|---|
| `tie` | `sound`, `print`, `placement` | Link distinct written notes with validated sounding continuity |
| `slur` | `placement`, `line_type` | Explicit start and final event; crossing slurs allowed |
| `tuplet` | `actual`, `normal`, `bracket`, `show_number` | Written group ending with its final member |
| `beam` | `level` | Ordered written members; hooks need an explicit feature module |
| `crescendo`, `diminuendo` | `placement` | Notated wedge span; expression realization is separate |
| `pedal` | `value`, `line`, `sign` | Identified pedal span with explicit continuation points |
| `octave` | `octaves`, `print`, `sound` | Explicit notated/sounding octave displacement |
| `lyric_extend` | `verse` | Melisma/lyric extension with explicit end |

Each closed core relation has one start, one end, and zero or more ordered continuations. The record occurs at the anchor's frame. The relation's type is stable. Attributes that define the relation belong on its start; repeated immutable attributes must agree. Pedal control values at continuations are per-anchor values. References are resolved after frame collection. No stack nesting assumption is allowed, because musical relationships can cross.

Core tie defaults are `sound=true` and `print=true`; an importer must supply explicit differing values when source sounding and printed ties differ. A sounding tie requires compatible sounding pitch and end/start continuity under the declared profile. Notation-only ties can set `sound=false`. A tie is not a single merged note: retaining separate note identities preserves barline spelling and editing targets.

Tuplet `actual` and `normal` are positive integers; start requires both. `bracket` is boolean, and `show_number` is `actual`, `both`, or `none`. Beam `level` is a positive integer. Octave `octaves` is a nonzero integer; `print` and `sound` are booleans. Pedal `value` is an exact rational from 0 through 1; `line` and `sign` are booleans. Slur `line_type` is `solid`, `dashed`, or `dotted`. These basic fields are a declared initial vocabulary, not a claim that all source-specific variants are already standardized.

An extracted region may have crossing relations. It must carry boundary context separately rather than presenting dangling relation records as a complete standalone score. A complete score cannot silently leave open ties, tuplets, or other required endpoints.

## 8. Bounded fragments and partial edits

A replacement fragment uses local time and explicit length:

```text
!fragment 1.1 unit=q span=4
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
```

The edit request binds `mel` to an existing voice ID. Fragment event IDs are local to that insertion and map to returned stable IDs after commit. The header does not declare new parts or voices. A fragment has `end=span`, all measured onsets in `[0,span)`, and event tails within the span. The core replacement profile requires coverage of each declared replacement voice interval using notes, rests, or explicit spacers. Chord member overlaps do not count as separate elapsed time.

Fragments inherit declared entry context from the frozen selection. They cannot change global state or measure objects in v1.1. They cannot introduce new required extensions. Their internal event references resolve locally; reconnection to external relationships uses typed transaction operations. This keeps a fragment small while preserving a well-defined boundary.

`replace_range` uses `duration_policy="preserve"` and `boundary_policy="reject_unresolved_crossings"`. The span length must equal the replaced interval. A voice binding must refer to an authorized voice within the resolved selection. Unselected voices remain untouched. Ripple replacement is not a v1.1 option.

The typed edit schema also defines `set_pitch`, `set_duration`, `move_event`, `delete_events`, `remove_relation`, and `retarget_relation`. Exact fields are in `schemas/edit.schema.json`. A retarget operation can name a fragment event by operation index and local ID, allowing a crossing relationship to be repaired within the same atomic command. Referencing an uncommitted guessed global ID is not supported.

Every edit identifies `score_id`, `expected_revision_id`, a frozen selection reference, and `idempotency_key`. A successful command returns a new revision, mappings, impacts, and diagnostics. A conflict does not reevaluate the selector on the latest revision automatically. The agent must read the current score and form a new valid request.

## 9. Extension and preservation rules

Unknown unqualified attributes or kinds are errors. A semantic extension has a versioned namespace, declared with `!requires`. Namespaced attributes use `x.namespace.field`; namespaced mark and relation kinds use `namespace.kind`. Extension registries specify their own fields, targets, meaning, validation, canonicalization, and conversion behavior.

An implementation that does not understand a mandatory extension can preserve and inspect the document if its preservation path is safe. It must not claim semantic support or perform operations that require interpreting that extension. Core operations unaffected by a preserved attachment require an explicit impact/validity decision; “unknown” is not automatically “safe to keep after any edit.”

New record prefixes require a grammar/version change or an explicit transport contract. They cannot be smuggled into a core parser as arbitrary ignored lines. Independent clocks, arbitrary graphic notation, navigation execution, special instrument systems, and detailed engraving are planned feature families; their appearance in a roadmap does not make their syntax implemented.

Complete source recovery uses the archival package with original bytes and attachment mappings. A plain `.sfs` file containing only interpreted core events is not a byte-identical backup of arbitrary MusicXML. If a source feature cannot be interpreted or safely preserved, import/export must report or reject it under the selected fidelity policy.

## 10. Canonicalization and compatibility

Canonicalization preserves musical semantics, object identities, chord/grace ordering, and declared notation fields. It reduces rational values, emits stable field/record order, and assigns IDs when necessary. It does not rewrite enharmonic spelling, quantize rhythms, merge ties, fill every gap with rests, or regenerate authored lyrics.

Text comments and formatting can be retained in an editor concrete syntax tree. They are not musical knowledge annotations and are not required semantic contents of the normalized AST. A source-preserving edit can keep trivia when practical; a canonical export may normalize it. Original source recovery remains an archival function.

An implementation must reject an unsupported major/minor format version unless it has an explicit compatible reader. The output reports the exact version and profile it produced. Application capability reports are independent from format version: two conforming parsers can have different renderer or target-export support.

## 11. Validation layers

| Layer | Responsibility | Example failure |
|---|---|---|
| Lexer/parser | EBNF and token formation | Unclosed quote, malformed pitch, missing frame dot |
| AST schema | Structural shape and primitive constraints | Wrong record type, absent field, JSON float used for time |
| Language semantics | Identity, references, time, scopes, groups, extension rules | Unknown voice, duplicate ID, frame order error, invalid grace anchor |
| Edit semantics | Frozen revision, interval coverage, authorized bindings, crossing relations | Replacement too long, stale selection, dangling tie |
| Adapter capability | Source/target fidelity and feature support | Nonterminating alteration cannot be represented exactly in a target field |
| Product integration | Durable commits, UI state, rendering, playback | Old render displayed over a newer revision |

The `S001`–`S018` rules in the language descriptor summarize essential semantics. A JSON Schema pass is necessary for an AST-based API, but it does not prove those rules or musical quality. Diagnostics should carry stable lowercase codes, severity, source range/object IDs, concise explanation, and a recovery suggestion when one exists.

## 12. Agent discovery contract

`score_get_format({})` selects the current supported format/profile and returns a practical quick reference inline. The response includes record syntax, exact time/pitch rules, defaults, semantic constraints, complete examples, and versioned resources. More detailed selection returns the full descriptor. The agent does not need to infer the DSL from MCP input-schema property names.

Resources include:

- `scorestream://spec/1.1/agent-reference`
- `scorestream://spec/1.1/grammar`
- `scorestream://spec/1.1/language`
- `scorestream://spec/1.1/ast-schema`

The primary format artifacts are `spec/scorestream.ebnf`, `spec/scorestream-language.json`, `schemas/scorestream-ast.schema.json`, this chapter, and the complete fixtures. If an artifact conflicts with another, the package must be corrected before a format release. The runtime must not silently invent a precedence rule to hide a contract inconsistency.

### Independent part labels

`!part` accepts optional `display_name="Flute 1"`. It does not change `instrument`, pitch, or playback. Absence uses the instrument as a presentation fallback; `display_name=""` preserves an explicitly empty label. Text is preserved, including Unicode, whitespace and escaped quotes, and must contain only XML 1.0 characters. Renderers and exporters escape the label as text, never execute it as markup. JSON AST parts omit an absent field; explicit null and non-string values are invalid.

### Registered authored navigation capability

`scorestream.navigation@1` registers the namespaced `scorestream.barline` mark. Its part target and `measure`/`side` fields locate an authored boundary exactly; optional typed style, repeat/count and ending/pass/display fields preserve its meaning. The normative field registry and `docs/SFS_NAVIGATION.md` define field pairing, identity, limits and failure behavior. Unknown readers must reject the mandatory capability. Current conversion supports ordinary closed-repeat roundtrip; performed lowering rejects rather than playing the written sequence once. An unclosed source ending remains unclosed and requires a separate explicit continuation decision, not a default import repair.
