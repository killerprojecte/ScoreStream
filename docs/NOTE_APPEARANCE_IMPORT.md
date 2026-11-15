# Authored stems and full-measure rests

This bounded bridge preserves exact MusicXML note fields through the common
`ScoreDocument`, SFS 1.1, canonical text/JSON and MusicXML export. It is not a
complete-score import or audio acceptance claim.

## Independent fields

- `ScoreEvent.stem` and `ScoreGraceNote.stem` retain `up`, `down`, `none` or
  `double`. Absence stays absent. The enum is independent of written note type,
  pitch, grace timing and measured duration. SFS uses its existing `stem` field;
  previously valid native SFS stems, including rest stemlets, stay valid.
- `ScoreEvent.measure_rest: Option<bool>` maps only the explicit MusicXML
  `rest/@measure` yes/no attribute. SFS adds the boolean `measure_rest` event
  attribute. Absence, explicit false, and true remain distinct.
- True is valid only on a measured rest that begins at its owning explicit
  measure boundary and spans its exact **actual** length. It does not replace
  actual length with written meter, manufacture `w=1`, or convert an ordinary
  rest merely because it fills a measure. Explicit written spelling is retained
  independently, even when it differs from elapsed rest time.
- Native validation resolves part/staff measure ownership and voice staff state,
  including state at the same onset independent of record ordering. An explicit
  event staff overrides the voice default. Fragments cannot assert full-measure
  identity without the owning measure context; `measure_rest=true` is rejected.
- Old common-model JSON omitting these fields remains valid. No imported source
  XML or foreign markup is added to the common model.

Only attribute-free `stem` leaves and the `rest` measure flag are newly mapped.
Stem coordinates/colors, rest display-step/octave, unknown descendants, beam,
accidental, notehead and hidden-note attributes remain blocking unless separately
implemented. Coordinate regeneration still requires its unchanged explicit
three-path policy; stems and rest identity are preserved, never regenerated.

## Preview and performance

The desktop projects supported up/down stems, including uniformly marked chord
members, and regular-meter full rests to MEI `mRest`. Its generated XML ID retains
the existing original-event identity map. Real pinned Verovio tests verify stem
geometry, the full-rest glyph, and exact 0 → 3 → 6 measure timing in 3/4.

The preview explicitly withholds stemlets, none/double, mixed chord stem
indications, stem directions on stemless durations, full rests with explicit
written spelling/dots/graphic sizes, and irregular full rests. In particular,
pinned Verovio can give an irregular mRest-only measure zero renderer duration;
this bridge does not disguise that by inventing spaces or nominal time. The
common/XML/SFS model still preserves these valid authored cases exactly.

Measured event roles and performance duration do not change. Tests compare actual
playback plans with and without the new appearance fields. Imported grace and
fermata fidelity guards are unchanged. A hidden rest is not deleted time: all
80 source hidden rests still fail explicitly at their unsupported visibility
attribute rather than being turned into absent events.

## Pinned original evidence

The ignored production-decoder inventory checks all four unchanged original
XML fingerprints and all 68,559 notes. It proves 44,313 stems (27,319 down,
16,994 up), including all 237 genuine graces, and 11,840 explicit full rests.
Every full rest starts at zero and matches its active written-meter duration;
none has a written type, and 52 retain an independent fermata. Movement totals
are stems 10,029 / 7,102 / 5,939 / 21,243 and full rests 3,197 / 2,152 /
3,630 / 2,861. The same untouched original inputs are still rejected for other
unimplemented semantics. No source tags are trimmed to obtain acceptance.

Small clearly identified source fragments exercise end-to-end conversion:
I/P1/m1 full rest and I/P1/m262 and m19 stems. Separate regression fixtures
exercise I/m342's distinction between a full rest and ordinary half rests of the
same duration, ordinary pickup rests, true graces, chord members, fermatas,
explicit irregular extents, absent fields, direct AST/JSON, and unknown fields.

```sh
cargo test --locked -p scorestream-formats --test note_appearance
SCORESTREAM_MXL_FIXTURES=/path/to/original-inputs \
  cargo test --locked -p scorestream-formats --lib appearance -- --include-ignored
cd desktop && npm test && npm run build
```

Definitions: [MusicXML stem](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/stem/)
(including stemlets) and [MusicXML rest](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/rest/)
(full-measure identity versus staff placement).
