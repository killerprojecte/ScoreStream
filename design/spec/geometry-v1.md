# Exact authored coordinates v1

`!requires scorestream.geometry@1` preserves MusicXML direct note `default-x`,
`default-y` and measure `width` values as typed signed rational tenths. Rational
denominators must have only factors 2 and 5, so every value has an exact decimal
representation. There is no geometry-specific digit or magnitude limit.

- `scorestream.geometry.event` targets `#event` at its exact onset. Optional
  `default_x` and `default_y` are rational string scalars; at least one is required.
  Measured notes, rests, chord members and grace identities remain distinct.
- `scorestream.geometry.measure` targets `#measure` at its exact onset. Required
  `part` is its owning part identifier and `width` is a rational string scalar.

A complete score and nonempty records are required. Unknown fields, kinds,
versions, duplicate hosts, dangling identities, foreign ownership, wrong onsets,
and nonterminating rationals reject. Absence is distinct from explicit zero.
Canonical XML export may normalize leading plus signs and trailing decimal zeros;
values and their event/measure attachments, not lexical spellings, round-trip.

CLI conversion and desktop MusicXML/MXL opening preserve these fields by default.
The low-level API remains strict by default. Explicit coordinate regeneration is
still available and remains a separately reported lossy policy. No relative
coordinates or other unsupported geometry become accepted by this change.

Validated geometry is visual metadata and has no effect on playback. Desktop
preview independently reflows notation and explicitly warns that authored positions
are not reproduced; neither that warning nor geometry alone withholds preview.
Other unsupported notation retains its existing capability checks.

Focused verification: `cargo test -p scorestream-formats --test geometry_retention`
and `desktop/tests/geometry-retention.test.ts`. The former covers XML/model/SFS/XML
retention, chord/grace identity, signed/zero/absent values, 91 decimal fractional
digits, hostile wire fields/ownership/onsets, strict/regenerate boundaries, and
identical geometry-on/off playback plans. This is not a claim that the entire
original Beethoven corpus imports: unrelated unsupported structures still block.
