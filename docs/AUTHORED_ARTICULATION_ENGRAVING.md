# Bounded authored articulation engraving

This extends the opt-in `scorestream.articulation@1` Workspace preview, not the
MusicXML importer default or playback. The complete authored family remains in
its existing Rust model and scalar SFS wire format. No Rust files, playback gates,
desktop import defaults or renderer dependency versions change.

## Capability matrix

The shared `render/authored-articulations.ts` adapter has an exhaustive 17-choice
inventory. All visible cases below require a measured pitched host and no authored
appearance/coordinate fields. Missing placement remains missing in the receipt.

| Authored choice | MEI / pinned Bravura SVG | Accepted placement |
| --- | --- | --- |
| accent | `acc`, E4A0 / E4A1 | absent, above, below |
| strong-accent | `marc` plus orientation override, E4AC up / E4AD down | absent, above, below; orientation independent |
| staccato | `stacc`, E4A2 / E4A3 | absent, above, below |
| tenuto | `ten`, E4A4 / E4A5 | absent, above, below |
| detached-legato | separate `stacc` then `ten` components | absent, above, below |
| staccatissimo | `stacciss`, E4A8 / E4A9 wedge | absent, above, below |
| spiccato | `spicc` plus stroke override, E4AA / E4AB | explicit above or below only |
| stress | `stress` plus glyph override, E4B6 / E4B7 | explicit above or below only |
| unstress | `unstress` plus glyph override, E4B8 / E4B9 | explicit above or below only |
| soft-accent | `acc-soft`, ED40 / ED41 | absent, above, below |
| scoop | refused | no jazz-line geometry adapter |
| plop | refused | no jazz-line geometry adapter |
| doit | refused | no jazz-line geometry adapter |
| falloff | refused | no jazz-line geometry adapter |
| breath-mark | refused | no fully qualified shape/after-note adapter |
| caesura | refused | no fully qualified shape/after-note adapter |
| other-articulation | refused when visible | no passive-text/custom-glyph rendering policy |

This means **seven ordinary choices with automatic or explicit placement, three
more with explicit placement, seven refused**. It is not full 17-choice engraving.
All ten visible choices have exact above/below SVG glyph assertions. The seven
with automatic placement are also tested in simultaneous upper/lower voices.

### Renderer-specific boundaries, not aliases

Strong-accent's omitted MusicXML type has visual default up, regardless of
placement. The receipt preserves omitted versus explicit up/down; only the MEI
projection selects the up glyph for absence. Explicit down above the note and
explicit up below the note are both verified, with both stem directions.

Plain Verovio `stress`/`unstress` artics draw no glyph. Plain `spicc` chooses
E4A6/E4A7, rather than the explicitly stroke-shaped E4AA/E4AB. The closed adapter
uses verified SMuFL overrides. Verovio does not switch a fixed override when
its automatic layout chooses the other side. Therefore these three choices
require explicit placement; no stem heuristic or side guessing is added.

Detached-legato uses one authored item with two deterministic component IDs,
`-stacc` and `-ten`. The dot is nearer the note and the line farther out on either
side. Separate native MEI components allow renderer-owned automatic placement.
The receipt retains the single compound identity, distinct from two separately
authored signs. Repeated compounds and mixed accent/compound stacks retain all
component IDs and geometry. Deprecated `ten-stacc` is not used. A multi-value
`artic` with a composite glyph override would duplicate the whole combined glyph
in Verovio 5.6.0, so that representation is deliberately avoided.

## Typed hierarchy and identities

The adapter consumes Workspace records, never SFS or MusicXML source text. It
reconstructs the registered event/notations/group/item hierarchy using indices,
not frame traversal order. It checks versions, required parents, contiguous
indices, duplicate paths, exact target/onset agreement, mapped pitched hosts,
source-ID uniqueness and mixed-family boundaries before producing MEI. Preview
is limited to 10,000 authored hierarchy records. Future kinds/fields/versions and
missing capabilities fail closed.

`project(view).authoredArticulations` is the typed source-to-render receipt:

- It retains event order, wrappers, groups, empty objects, repeated signs,
  source IDs, print flags, optional placement and choice-specific data.
- Each original leaf has `ss-aa-E-W-G-I` identity. Its `renderIds` identify each
  actual SVG object. A compound has two component IDs; a hidden leaf has none.
- All rendered IDs map to the original event in the existing selection map.
  Independent chord members, simultaneous voices and a second staff retain
  their own anchors. No sign is copied to an unmarked neighboring note.
- Source IDs remain source IDs in the receipt; they are not reused as synthetic
  SVG IDs. No source text or identity is interpolated into renderer attributes.
- Sibling MEI artics retain leaf order. Actual SVG tests check near-note-to-outward
  stacking, duplicate separation and repeated/empty group/wrapper boundaries.

MEI's flat note children are **not** claimed to serialize MusicXML wrapper/group
topology losslessly. That topology stays in the typed receipt and unchanged
source. The adapter does not invent spacing between source containers or a
new visual bracket for them. Source item order controls ordinary stacking.

## Hidden objects, appearance and refusal

Absent `print_object`, explicit true and explicit false remain distinct. A false
notations wrapper intentionally produces no visible glyphs and no phantom
selection IDs. Its complete validated hierarchy remains in the receipt. This
also permits known unrenderable signs inside hidden wrappers, because the author
explicitly requested that nothing be printed. Unknown schema or malformed
fields still fail, including when hidden.

Visible default/relative coordinates, font family/style/weight/size and color
remain unsupported. Even explicit zero or a default-looking font value refuses
rather than being normalized away. Visible text, custom glyph names, line shapes,
lengths, dash parameters and unsupported gestures refuse at the specific event,
wrapper, group and item. There is no partial preview after such a failure.
Hidden passive text is retained exactly, never interpreted as markup, displayed
as a substitute symbol or passed to Verovio. Glyph names from source are never
used as overrides; only the adapter's closed mapping is used.

The existing actual Rust-produced all-17 fixture remains unchanged. It now fails
at its specific unplaced spiccato object, rather than the blanket capability
header. It is not relabeled as a supported full score. Separate tests use explicit
Workspace fixtures for accepted engraving combinations; no new full MusicXML
corpus import or new Rust snapshot-generation run is claimed.

## Pause-family qualification

The follow-up [pinned pause renderer probes](AUTHORED_PAUSE_RENDERER_PROBES.md)
cover every named and omitted breath/caesura shape. They establish concrete
font and after-note geometry blockers, not new runtime capability. The ten
ordinary choices and all hidden-wrapper behavior above remain unchanged.

## Verification

Pinned **Verovio 5.6.0 / Bravura** tests inspect real SVG: exact glyph references,
nonempty definitions, deterministic object/component IDs, correct note ownership,
above/below geometry, automatic polyphonic placement, duplicate and compound
stacking, and hidden glyph absence. MEI generation alone is not acceptance.

Run scoped frontend regressions and TypeScript from `desktop`:

```
node node_modules/vitest/vitest.mjs run \
  tests/authored-articulation-engraving.test.ts \
  tests/articulation-authored-fidelity.test.ts tests/articulations.test.ts \
  tests/accidental-appearance.test.ts tests/note-appearance.test.ts \
  tests/slur-fidelity.test.ts tests/beam-fidelity.test.ts \
  tests/marks.test.ts tests/render.test.ts
node node_modules/typescript/bin/tsc --noEmit
```

**Final local checks:** 221 frontend tests passed across nine files (48 in the
new authored suite); TypeScript and 19 offline ledger regressions passed. Eleven
exact named frontend ledger receipts passed independently. These are scoped
results, not a full frontend or Rust suite rerun.

Optional `AUTHORED_ARTICULATION_EVIDENCE_DIR` writes ordinary-family above/below
SVGs and the polyphonic test's exact SVG and typed input/projection receipt to an existing local directory. These are
renderer outputs, not native-app screenshots. Offline notation ledger validation
and exact named frontend evidence are recorded separately from historical Rust
results. No Rust rebuild, production Vite build, native packaging, untouched
external-score acceptance, acoustic test, remote push, pull request or CI result
is claimed for this milestone.

## Primary references

- [MusicXML strong-accent](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/strong-accent/)
- [MusicXML detached-legato](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/detached-legato/)
- [MusicXML spiccato](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/spiccato/)
- [MEI articulation vocabulary](https://music-encoding.org/guidelines/v5/data-types/data.ARTICULATION.html)
- [MEI artic and glyph attributes](https://music-encoding.org/guidelines/v5/elements/artic.html)
- [SMuFL articulations](https://smufl.formats.music/latest/tables/articulation.html)
- [SMuFL articulation supplement](https://smufl.formats.music/latest/tables/articulation-supplement.html)
- [Verovio implementation caveats](https://book.verovio.org/toolkit-reference/mei-support.html)
