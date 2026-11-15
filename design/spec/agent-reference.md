# ScoreStream 1.1 — Agent Reference

Use this reference before generating or editing ScoreStream. It teaches the **score text language**; MCP tool argument schemas are separate. The current profile is `core-measured`. These are proposed contracts, not a claim of a running server.

## Read the shape

```text
!score 1.1 unit=q
!part p instrument=piano staves=1
!part v instrument=violin staves=1
!voice rh part=p staff=1
!voice vn part=v staff=1
@0
= score tempo=q:120
= p.s1 meter=4/4 key=0 clef=G2
= v.s1 meter=4/4 key=0 clef=G2
| p pm1 len=4 label="1"
| v vm1 len=4 label="1"
rh C4:2 id=a
vn G4:4 id=b
.
@2
rh E4:2 id=c
.
!end at=4
```

At `@0`, both instruments begin together. The violin continues through time 4; its absence from `@2` means no new onset. Each `.` closes one complete frame. A complete import/replacement also needs `!end` and successful semantic validation.

## Essential syntax

| Form | Meaning |
|---|---|
| `@3/2` | Start a frame at 1.5 quarter-note units, represented exactly |
| `rh C#4:1/2 id=n1` | C-sharp, half-quarter duration, stable text ID |
| `rh Db4:1/2 id=n2` | D-flat spelling is retained |
| `rh C(+1/2)4:1 id=n3` | Exact half-semitone alteration |
| `rh r:1 id=r1` | Rest |
| `rh _:1 id=s1` | Invisible spacer |
| `dr u.snare:1 id=d1` | Unpitched event |
| `= p.s1 meter=3/4` | Persistent staff state from this frame |
| `+ #n1 articulation value=staccato` | Intrinsic event marking |
| `~ tie t1 start=#n1 sound=true print=true` | Begin a relationship with its own ID |
| `~ tie t1 end=#n2` | End at the next written event |
| `| p pm2 len=3 label="2"` | Actual measure duration and independent display label |

Quote text using JSON rules: `text="dolce e cantabile"`. Whole-line comments begin with `;`. There are no inline comments; `#` belongs to pitches and references. Unknown unqualified fields are errors.

## Score metadata

Before frames, use `!meta title="Spring" description="A short study" authors=["Lin", "Alex"] header="Rehearsal" footer="Draft"`. Every field is optional. Authors are an ordered list of nonblank single-line names; the other fields are JSON-quoted plain text. The title is not a filename. Header/footer are separate running page text, without markup or template substitution. Description is editor/catalog context, not automatically printed. Legacy `subtitle`, `composer`, `lyricist`, `arranger`, `work_number`, `movement`, `rights` and `language` remain supported and are not inferred from authors. Repeated fields and metadata inside fragments fail. `topics=["syntax"]` includes the exact field types and a full metadata example.

## Rules to keep in context

1. **Quarter note = 1.** Use reduced rational strings such as `1/3`; do not emit decimal timing or JSON floating-point time. Every ordinary event states its own positive duration.
2. **One frame per score time.** Times strictly increase. Put all simultaneous onsets vertically in the same frame, and close it with `.`.
3. **Spelling matters.** `C#4` and `Db4` are distinct notation. MIDI numbers are derived views. Printed accidentals are separate marks.
4. **A voice is not a staff.** `staff=2` affects only that event; `= rh staff=2` changes its later default. Part and voice aliases cannot collide; `score` is reserved.
5. **State scopes are exact.** Tempo targets `score`; meter/key/clef target a staff; instrument targets a part. Do not assume generic inheritance. Omitted state remains unchanged or initially unset.
6. **Use explicit chords.** Simultaneous same-voice notes share `chord=c1`; retain their order and individual durations. Different voices are not silently merged.
7. **Preserve written rhythm.** `w=8 tm=3:2` plus duration `1/3` describes a common eighth-note triplet. `dots`, `tm_normal_type`, and `tm_normal_dots` retain written details. Visible tuplet groups are separate relations.
8. **Grace notes are anchored.** Use `:? kind=grace anchor=#a order=-1` in the principal's frame. Do not assign a fake measured duration to force a grace note onto the timeline.
9. **References must resolve.** Keep stable text IDs returned by the service. External API IDs may require the service's explicit text-ID mapping.
10. **Check boundaries.** Continuing notes, ties, tuplets, lyrics, and other relationships can cross a selected passage. Read the provided boundary context before replacing it.
11. **Unsupported means unsupported.** A preserved extension is not necessarily interpreted, editable, renderable, or playable. Follow the selected profile and reported capabilities.
12. **Analytical prose is separate.** Do not place an explanation of a passage's emotional effect into score marks unless it is intentionally printed notation. Future knowledge annotations have an independent store.

## Partial editing workflow

1. Call `score_get_format` if you have not learned this version/profile.
2. Inspect the score and pin the returned `revision_id`.
3. Read the target region. Keep the frozen selection reference, text-ID mappings, entry state, and boundary relationships.
4. Choose a typed operation for a small change, or a bounded fragment for replacing a passage.
5. Apply with `expected_revision_id`, the frozen selection, and an `idempotency_key`. Use a preview when useful; it is not an automatic extra approval requirement.
6. If there is a conflict, re-read and re-plan. Do not retarget old measure labels or coordinates to the new head by guessing.

Example replacement source:

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

Bind `mel` to the existing selected voice through the edit request. Local time begins at 0 and ends at `span`. Keep all event tails inside the span and cover the replaced voice interval with notes, rests, or spacers. Do not include part/voice declarations, state mutations, or measure mutations. Internal references are fragment-local; use typed relationship operations for external connections.

The supported replacement policies are `duration_policy="preserve"` and `boundary_policy="reject_unresolved_crossings"`. Ripple is not supported by this contract. The full tool schema defines the exact JSON request; do not invent raw JSON Patch paths into the score.

## More detail

The complete machine reference is `scorestream://spec/1.1/language`; grammar is `/grammar`; parsed AST schema is `/ast-schema`; this reference is `/agent-reference`. JSON Schema checks the AST structure. The parser and semantic validator additionally check text syntax, time, identity, references, scope, and music/edit constraints.

Complete package examples cover an ensemble, tuplets and ties, a replacement fragment, and grace/percussion/cross-staff events. Retain the exact format version and capabilities in every read/generate/edit workflow.

Part display labels: `!part f1 instrument=flute staves=1 display_name="Flute 1"` keeps the authored name separate from the instrument key. `display_name` is optional JSON-quoted plain text. Missing uses the instrument-label fallback; empty stays empty. It never changes sound routing.

## Authored navigation capability

`!requires scorestream.navigation@1` enables `+ <part> scorestream.barline measure=<measure-id> side=left|right` with typed style, repeat/count and ending/pass/display fields from the descriptor. The frame must equal that measure's exact boundary. This is authored-only interchange: current playback lowering rejects, and inspection warns that engraving/replay is unavailable. Strict conversion supports ordinary closed repeats; an unclosed ending is preserved for inspection with an explicit diagnostic, never silently closed or played once.
