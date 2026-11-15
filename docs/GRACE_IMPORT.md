# Authored grace notes: a typed, unmeasured bridge

The bounded MusicXML profile now preserves ordinary pitched before-principal
grace groups through MusicXML → `ScoreDocument` → SFS 1.1 → common model →
MusicXML. This is notation and explicit timing-intent preservation, **not grace
playback or whole-original-score import**. No fixed grace duration is invented.

## Separate identity, placement, writing and timing

`ScoreDocument.graces` is a separate optional collection of `ScoreGraceNote`.
Each grace has a stable event ID, a measured principal event ID, consecutive
negative order `-N..-1`, pitch, optional written rhythm, optional slash and typed
`GraceTiming`. It does not contain a measured `duration`. Its principal supplies
part, measure, voice, staff and exact written onset. Measured `ScoreEvent` duration
and all following score-time positions remain unchanged.

The current profile requires a contiguous group immediately before a pitched,
non-chord principal in the same voice and staff. Cross-voice/staff groups,
after-principal groups, grace chords, rests, silent cues, ties and other notation
on a grace note explicitly reject. Intervening directions, backup/forward or
state changes also reject rather than assigning a guessed anchor. Missing
principals and malformed regular notes without duration are errors.

The MusicXML grace element has no duration. Its source fields are retained as:

- `slash`: absent, `yes` and `no` remain three different states.
- `steal-time-previous` / `steal-time-following`: exact percentages, each 0–100;
  absent does not mean zero or a chosen default.
- `make-time`: exact nonnegative source value divided by active MusicXML
  divisions, retained as quarter-equivalent **timing intent**. Explicit zero
  remains distinct from absence. Only this field requires divisions at the grace.
- Note type, graphic size, dots and time-modification are written notation,
  independent of both measured and performed duration. Graphic `grace-cue` size
  does not convert a grace into an ordinary or silent cue note.

Import does not infer a playing policy from a slash or written note type.
Combinations of explicit timing fields are preserved without interpreting their
interaction. Negative make-time is outside this implemented profile. Export
includes make-time denominators in its exact divisions LCM, subject to the same
1,000,000,000 bound as measured events. Alterations/percentages require exact
finite decimals; every emitted grace number must fit the reader's 64-character
numeric bound. Unrepresentable output rejects before returning any XML, without
rounding. Different source divisions may be normalized, not the authored value.

## Required fidelity, without changing legacy grace playback

The bridge reuses the existing native SFS grace event syntax and adds one typed
metadata mark per imported grace, even when all timing fields are absent:

```sfs
!requires scorestream.grace@1
; within the principal's frame
v D5:? id=g kind=grace anchor=#principal order=-1 w=16 slash=false
+ #g scorestream.grace steal_following=25/2
```

The complete score must declare `scorestream.grace@1`. Every grace in this profile
has exactly one metadata mark at its own onset, even an empty mark. The validator
checks identity, anchor frame/voice/staff, pitched non-chord roles, consecutive
negative order, metadata field types/ranges and capability pairing. Unknown
fields, unmarked graces, orphan marks and duplicate requirements reject. The
capability cannot be introduced by a fragment.

Both conversion directions report non-error `preserved / grace.authored_only`.
Core validation reports `grace.realization_unavailable`; it does not invent time.
Sounding lowering independently rejects `grace.performance_unsupported` whenever
this required capability is present. A fermata or navigation policy does not
authorize grace timing. Desktop preview explicitly withholds unsupported grace
layout. No grace is silently omitted, converted to a normal attack or counted as
successfully rendered audio in this route.

Existing native SFS grace without the new capability retains its existing
warning/omission playback behavior. Conversion of that legacy form remains
unsupported (`grace.required_fidelity`), rather than silently changing its
contract. Generic source edits, canonicalization and workspace pitch edits retain
the new capability, grace fields and marks, including through undo/redo.

## Evidence from the untouched Beethoven sources

The ignored corpus test reads the four pinned originals through the production
incremental XML reader and grace metadata decoder. It checks complete file hashes,
all source grace metadata, absence of duration, immediate principal voice/staff,
and one- or two-note group order. It does **not** strip other authored children.

| Movement | Genuine grace notes | Slash yes | Slash absent | Groups | Two-note groups |
| --- | ---: | ---: | ---: | ---: | ---: |
| I | 0 | 0 | 0 | 0 | 0 |
| II | 13 | 7 | 6 | 10 | 3 |
| III | 128 | 4 | 124 | 66 | 62 |
| IV | 96 | 32 | 64 | 64 | 32 |
| Total | 237 | 43 | 194 | 140 | 97 |

All 237 are pitched, genuinely unmeasured and before ordinary principals, with no
explicit time-steal/make-time fields. Their missing timing policy stays missing.
The full originals still reject on other unsupported metadata and notation,
including authored stems, beams, accidental glyphs, slurs and layout. The
end-to-end tests are separately identified small fixtures, not substitute scores.
No sampled whole-score audio or native desktop visual acceptance is claimed.

Run:

```sh
cargo test --locked -p scorestream-formats --test graces
SCORESTREAM_MXL_FIXTURES=/path/to/untouched/originals cargo test --locked \
  -p scorestream-formats original_beethoven_grace_inventory -- --ignored
```

## Official definitions

- [MusicXML grace element and its timing/slash attributes](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/grace/)
- [MusicXML note content: grace versus measured/cue duration](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/note/)
- [MusicXML percent](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/percent/)
- [MusicXML divisions](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/divisions/)
