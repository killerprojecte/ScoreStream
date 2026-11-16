# Exact authored coordinates v1

`!requires scorestream.geometry@1` preserves MusicXML direct note `default-x`,
`default-y`, direct augmentation-dot `default-x`/`default-y`, and measure `width` values as typed signed rational tenths. Rational
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

## Augmentation-dot coordinates

`scorestream.geometry.dot` targets the owning `#event` at its exact onset.
Required integer `index` is the zero-based position in that event's authored
augmentation-dot sequence. `default_x` and `default_y` are optional exact rational
tenths, with at least one present. The event must have an explicit written note
value and this index must be strictly below its existing written dot count.
The same event/index pair cannot occur twice. These are sparse coordinate records:
unstyled dots have no record, and gaps in indices mean existing unstyled dots,
not missing or synthesized dots. XML export traverses the existing dot count in
numeric index order; wire/model record traversal does not reorder children.

Direct `note/dot/@default-x` and `@default-y` are accepted only by the preserve
policy. The old strict and explicit note/measure regeneration policies retain
their former scope and reject dot coordinates. Dot count, note value, sounding
duration, normal-dot/time-modification, metronome dots, and note-level geometry
are independent and unchanged. No relative coordinate, color, placement, child
content, or other graphics field is implicitly accepted. Grace, rest and chord
member dots retain their own event identities. Desktop warns and reflows rather
than pretending these optical coordinates are rendered.

`cargo test -p scorestream-formats --test dot_geometry` covers sparse ordered
children, signed/zero/absent coordinates, canonical XML/SFS round trips, native
wire and public-model invalid owner/index/onset/duplicate checks, explicit policy
boundaries, arbitrary precision, and identical geometry-on/off playback.
`desktop/tests/geometry-retention.test.ts` checks unchanged MEI and source plus an
explicit warning with dot geometry present.

### Pinned original field inventory

Run `python3 scripts/audit_beethoven_dot_geometry.py <originals-directory>`.
The audit verifies the exact SHA-256 values shared with the navigation audit,
reads only direct note/dot descendants, and does not rewrite source material.

| Movement | Styled dots | default-x values | default-y values | SHA-256 |
|---|---:|---:|---:|---|
| I | 17 | 17 | 17 | `5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b` |
| II | 937 | 937 | 937 | `c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a` |
| III | 928 | 928 | 928 | `788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98` |
| IV | 1709 | 1709 | 1709 | `cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298` |

All observed dot attributes in these originals belong to this exact two-field
family. The pinned-coordinate sample tests exercise one real pair per movement.
This is field-family evidence, **not whole-score import acceptance**; unrelated
unsupported structures can still prevent a document from being returned.

The optional `--cli <scorestream-binary>` audit also runs the real converter on
each untouched XML original. It requires the direct dot-coordinate diagnostics
to consist of exactly the two preserved-field audits, with occurrence counts
matching the XML inventory, independently of whole-score acceptance.

Verification on the pinned originals with this bridge: XML/MXL diagnostic parity
holds for all four movements and no partial SFS is returned. Remaining diagnostic
counts are I: 801, II: 989, III: 349 (not truncated), IV: 1024 (truncated). These
include unrelated tuplet, wedge, words, ornaments, dashes and other unimplemented
families; the full-corpus command intentionally still returns a failing status.
