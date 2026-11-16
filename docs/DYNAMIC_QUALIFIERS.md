# Ordered dynamic qualifiers

One standard dynamic symbol may have one unstyled plain-text `other-dynamics`
child immediately before or after it. `DynamicQualifier { text, position }`
retains exact decoded text, whitespace and order, independently of the four
coordinate fields and explicit numeric sound dynamics. Text is not restricted
to an observed vocabulary and is never interpreted by the converter. It must be
nonblank. This is a typed text slot, not an XML payload or custom glyph mapping.

The corresponding SFS `dynamic` attributes are paired `qualifier_text` and
`qualifier_position=before|after`. Neither may occur alone. Native schema and
semantic validation reject incomplete pairs, invalid order labels and blank text.
XML export rejects illegal control characters. Multiple native glyph children,
repeated text runs, standalone text without a native glyph, nested markup and
styled text remain unsupported instead of being merged or dropped.

## Playback and preview boundary

MusicXML printed dynamics remain explicitly `sound=false`, with numeric `sound dynamics` in its existing separate lane. Such qualifiers do not block playback:
all audio/timing data remains unchanged, and the playback plan reports
`dynamic_qualifier_not_realized`. The lowered typed dynamic direction still
retains the exact qualifier. No expressive trajectory, articulation or loudness
change is inferred from words such as `dimin.` or `dolce`.

Native SFS marks with a qualifier and `sound=true`, including omitted sound that
defaults to true, fail with `dynamics.qualifier_performance_unsupported`. This
boundary applies to written, navigation, explicit grace, explicit fermata and
combined active profiles, and to expression resolution from a typed score.
There is no new expressive policy in this change.

Editor validation is successful but warns with `dynamics.qualifier_unrealized`:
the current reflow preview does not render the qualifier. Saving/importing is
not presented as expressive performance or page-rendering equivalence.

## Evidence

The pinned originals contain exactly 274 qualifiers: movement II 55, III 62,
IV 157. Authored before-symbol strings are `sempre `, `sempre più `, `più `,
`dimin. `; the after-symbol string is ` dolce`. The updated original-family gate
checks all four original hashes and 3255 total dynamics nodes, including exact
text/order/geometry roundtrips for every qualifier and the explicit playback
diagnostic. The originals are read-only. This is a family-level projection on
minimal fixtures, not a claim that complete original import is finished.

Generated gates test before/after ordering, whitespace/newlines, canonical SFS,
XML roundtrip, numeric-sound invariance, sound=true/default rejection, nonblank
text and paired wire validation, unsupported structures and illegal XML text.
