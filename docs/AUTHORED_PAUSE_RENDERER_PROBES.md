# Authored breath and caesura: renderer qualification, not enabled preview

This checkpoint deliberately changes **no production renderer, importer, model,
transport or playback code**. Both pause families remain visible-preview refusals
in `project(view)`. The existing ten ordinary authored choices and validated
hidden wrappers remain unchanged. No simple-note whitelist, timestamp conversion
or glyph fallback is shipped.

`desktop/tests/authored-pause-renderer-probes.test.ts` is a dormant test-only
characterization suite. It feeds controlled native MEI to the installed
**Verovio 5.6.0-85e7620 / Bravura** renderer, then checks the actual sanitized SVG,
nonempty glyph paths, transforms and bounding boxes. It is not another importer
and does not convert user source. A passing characterization test can prove a
failure mode; it does not mean that ScoreStream supports the notation.

## Exact shape findings

| Authored breath shape | Actual pinned result |
| --- | --- |
| omitted | Native breath draws E4CE comma. MusicXML permits an application-dependent shape for empty content. |
| comma | E4CE has a real path. |
| tick | E4CF is absent from the pinned subset. |
| upbow | E4D0 is absent from the pinned subset. |
| salzedo | E4D5 is absent from the pinned subset. |

Native `<breath glyph.num="…">` draws E4CE regardless of those overrides.
A separate glyph-bearing caesura probe also confirms that the three unavailable
codes fall back to E4D1, rather than producing the requested paths. That fallback
is recorded as a failure, never accepted as breath rendering.

`breath altsym` linked to `symbolDef` containing a SMuFL `symbol` produces an
empty breath group, including for the available comma. The pinned
`DrawSymbolDef` draws SVG/graphic children, not the symbol child. Thus a valid
MEI vocabulary or a resolved symbol definition does not prove a visible glyph.
An independently packaged fixed font asset or a renderer change would be needed
for the unavailable forms; importing arbitrary authored SVG is not an option.

| Authored caesura shape | Verified explicit glyph number |
| --- | --- |
| omitted | E4D1 native normal |
| normal | E4D1 |
| thick | E4D2 |
| short | E4D3 |
| curved | E4D4 |
| single | F42C, the slanted single-stroke path shown by MusicXML 4.0 |

All six caesura cases have real paths with both explicit placements and both
stem directions. This establishes glyph availability only. The name
`caesuraSingleStroke` resolves to a **different E4D7 path** in this resource set:
its measured width is 51 font units, versus 224 for F42C. Future projection must
use a closed, pinned mapping, not assume that an optional name or Unicode code
produces the required shape.

Omitted placement is also probed separately: native breath is above the staff,
whereas native caesura straddles its top line. Explicit above/below is
staff-relative, not necessarily above/below every ledger-line note. Absence must
remain absent in the authored receipt; a display default needs an explicit
policy rather than silent source normalization.

## Horizontal blockers reproduced in real SVG

The problem is not just choosing a glyph. Native controls need a safe space after
the associated note or chord, clear of following notation and the barline.
Exact arithmetic on a musical timestamp does not establish that space.

- `breath startid` centers the comma on its host's notehead column. Its `ho="2"`
  probe has exactly the same SVG position as the version without an offset.
- A release-time `tstamp` places both breath and caesura at the next note's x
  origin. It does not reserve a gap before that note.
- A half-duration midpoint fails for a sixteenth note: comma origin **2744**,
  host notehead right edge **2806.4**. The comma remains over the host column.
- The pinned MusicXML reader uses an end-relative display convention, half a
  written-meter beat before release. Even this convention fails for a quarter
  chord containing C4–D4: comma origin **2939**, displaced head right edge
  **3000.4**.
- Testing the entire glyph, not only its origin, exposes another failure. In a
  four-quarter-note passage, the first thick caesura extends to **3416.36**, past
  the next note's x **3284**. The last extends to **5486.36**, past the final
  barline's x **5354**.

Coordinates above are internal SVG engraving units for the named test fixtures,
not screen pixels or universal spacing constants. The emitted receipts contain
all notehead and sign bounds, stems, dots and barline paths. Native SVG was also
rasterized for local visual inspection of the short-note and dense/final cases.
A horizontal column overlap is distinct from a 2D collision: a high glyph may
avoid touching the note while still being attached ambiguously or extending
beyond the final boundary. Neither should be hidden by an origin-only check.

The midpoint and end-relative candidates remain **test inputs only**. These
findings are why no increasingly narrow runtime host whitelist was retained.
Neither candidate changes authored event onset, duration or performance policy,
but neither qualifies as general after-note engraving either.

## What the successful local probes do establish

Separate controlled native fixtures verify:

- Distinct IDs and real paths for repeated breath/caesura controls.
- Independent explicit staff-relative placements and stem directions.
- Local pairwise glyph and stem separation for duplicate/mixed pause groups
  plus detached-legato's separate dot/line and accent components, on both sides.
- Dot-nearer-than-line geometry for the native compound components.
- A dotted non-displaced chord with separate native control references for its
  members, another voice, and a second staff; the tested control lies after the
  chord's dots and heads and uses the intended staff.

These are isolated geometry and association receipts. They do **not** establish
universal collision safety, authored global stacking order across control
families, a lossless MusicXML-to-MEI adapter, native UI selection behavior, or
end-to-end import acceptance. Source wrappers/groups and independently authored
order still belong in the existing typed receipt, not in the flat MEI layout.

## Remaining implementation and acceptance work

A proper follow-up should keep the existing typed authored hierarchy and stable
host/component identity, then implement a renderer-aware after-note placement
contract that uses actual note/chord/dot/stem extents and the following boundary.
It must reserve horizontal space or withhold the entire preview when no safe
layout exists, including dense polyphony, displaced heads, short notes, final
notes, and system/page boundaries. An upstream renderer fix is another route.
Do not replace that work with a new timestamp heuristic or handpicked host list.

For the missing breath shapes, package fixed, licensed, reviewed glyph assets
with verified metrics or qualify a renderer/resource upgrade. Never forward
source `smufl`, passive text, font names, coordinates or SVG as an unchecked
renderer override. The ordinary glyph paths must remain regression-tested.

Visible optional default/relative coordinates, font family/style/weight/size and
color still reject, including explicit zero/default-looking values: preserving
an attribute in the authored model while dropping it from the drawing is not
fidelity. Hidden wrappers still suppress all validated content without losing
it. Unknown, malformed or inapplicable fields still reject even when hidden.

Playback, pause duration, legacy/import defaults and audio interpretation remain
unchanged. No pause duration can be inferred from a displayed breath or caesura.

## Reproduction and evidence accounting

From `desktop`, using the existing pinned dependencies:

```
mkdir -p /tmp/scorestream-pause-probes
AUTHORED_PAUSE_PROBE_DIR=/tmp/scorestream-pause-probes \
  node node_modules/vitest/vitest.mjs run \
  tests/authored-pause-renderer-probes.test.ts \
  tests/authored-articulation-engraving.test.ts \
  tests/articulation-authored-fidelity.test.ts tests/articulations.test.ts \
  tests/render.test.ts
node node_modules/typescript/bin/tsc --noEmit
```

The nine new tests include every breath/caesura shape, omitted values, font
fallback checks, anchor failure receipts, local stacking probes and unchanged
runtime rejection for all visible shapes. The five-file focused run passes
**97 tests**. The optional output writes native MEI, sanitized SVG and JSON
geometry/input receipts; it is not a native-app screenshot or an authored
projection receipt. No network, font download or dependency install is needed.

Only five **rejection** receipts enter the notation coverage ledger. The existing
render profile remains partial because of its unchanged ordinary-sign support;
these probes add no positive runtime capability credit. Import/model/export
statuses and all playback withholding remain unchanged. Offline ledger checks
and all **20 ledger regression tests** pass; the new regression explicitly
guards that distinction. All **five exact named rejection receipts** also pass
independently. TypeScript checking passes.

No Rust tests/build, full frontend suite, Vite build/server, native packaging,
new MusicXML corpus import, acoustic test, push, pull request or CI run is claimed.

## Primary references

- [MusicXML breath values](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/breath-mark-value/)
- [MusicXML caesura values](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/caesura-value/)
- [Pinned breath/caesura drawing](https://github.com/rism-digital/verovio/blob/version-5.6.0/src/view_control.cpp)
- [Pinned symbol definition drawing](https://github.com/rism-digital/verovio/blob/version-5.6.0/src/view_graph.cpp)
- [Pinned caesura lookup and fallback](https://github.com/rism-digital/verovio/blob/version-5.6.0/src/caesura.cpp)
- [Pinned MusicXML pause import](https://github.com/rism-digital/verovio/blob/version-5.6.0/src/iomusxml.cpp)
- [Pinned Bravura glyph metrics](https://github.com/rism-digital/verovio/blob/version-5.6.0/data/Bravura.xml)
