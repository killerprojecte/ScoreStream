# Contextual desktop and engraving refinement

## Status

This second UI pass is a **candidate awaiting native visual acceptance**. The first-pass native screenshots and 34-check report remain historical evidence for that first pass, not screenshots or runtime verification of this candidate.

On 2026-11-13, 33 frontend tests, TypeScript/Vite production bundling, and both normal and smoke-feature Linux executable builds passed. The expressive sample was also validated by the Rust CLI. Native interaction and screenshot capture were unavailable: the desktop-control service failed before initialization, and the isolated X11 test display could not create local sockets. No native UI2 smoke result is claimed. No dependencies were downloaded and no AppImage was rebuilt or published.

## Design decisions

The changes follow Apple's desktop interaction hierarchy rather than copying platform decoration:

- One compact document toolbar replaces stacked file/action bars. File controls, view choices and edits have distinct, restrained groups.
- The inspector describes the current selection. A note has aligned Pitch/Duration rows and a small Location group. Technical event identity is in a collapsed disclosure. There is no duplicate large pitch heading or decorative selection card.
- With no selected note, the inspector shows document properties and playback tempo. Clicking blank paper, pressing Escape on a focused engraved event, or moving the source cursor to a non-event line returns to Document.
- Enter commits the active property; Escape discards only that uncommitted field. Committing one property preserves the other staged value. Same-note selection survives mutation and undo/redo. Rejected values have local error feedback.
- The score uses a plain page, moderate title and actual engraving. Chrome uses system UI fonts. SVG music typography is left to the notation renderer.
- Engraved events have focusable, labelled keyboard targets. Existing source/score mapping, latest-only preview, dirty-draft protection, Apply, undo/redo and native open/save routes remain in place.

Primary references inspected:

- [Apple HIG: Panels](https://developer.apple.com/design/human-interface-guidelines/panels)
- [Apple HIG: Toolbars](https://developer.apple.com/design/human-interface-guidelines/toolbars)
- [Apple HIG: Typography](https://developer.apple.com/design/human-interface-guidelines/typography)
- [Pages for Mac: Format sidebar](https://support.apple.com/en-gb/guide/pages/tane9d8593b0/11.1/mac/1.0), including Apple's actual sidebar and document-window images
- [Logic Pro: Inspector](https://help.apple.com/logicpro/mac/9.1.6/en/logicpro/usermanual/chapter_3_section_6.html), including its contextual property-row image

## Musical semantics

`= score tempo=q:104` is a performance state. It is displayed as ordinary metadata in Document, not invented as a printed score mark.

An explicit SFS metronome record, such as `+ score metronome beat=q bpm=104 text="Moderato"`, is projected as an MEI tempo. The beat is a genuine Bravura SMuFL metronome glyph with an embedded music font, not a Unicode approximation or HTML string above the score. Dotted beat units and exact fractional rates are retained. Printed tempo does not overwrite playback tempo.

Explicit dynamic records use MEI dynamics and Verovio's engraved glyph paths. Event-targeted marks retain their event association for selection; scoped marks use exact metric placement. Unsupported marks, attributes or non-terminating scope timestamps produce visible projection warnings rather than silent deletion or rounding. Original SFS remains authoritative.

The grand staff has one instrument label and a brace, rather than repeated instrument names on both staves and in HTML. Staff meter/clef/key are read from exact staff scope, matching the SFS contract's lack of implicit scope inheritance.

The new `examples/first-light-expressive.sfs` preserves the original notes while adding an explicit Moderato quarter-note = 104 mark and mp/mf dynamics. The ordinary First light example remains unchanged.

Notation references: [MEI tempo](https://music-encoding.org/guidelines/v5/elements/tempo.html), [MEI dynam](https://music-encoding.org/guidelines/v5/elements/dynam.html), [SMuFL metronome marks](https://smufl.formats.music/latest/tables/metronome-marks.html), and [Verovio SVG output](https://book.verovio.org/toolkit-reference/output-formats.html).

## Modules and tests

- `src/inspector/view.ts`: contextual markup, field staging, keyboard handling and local errors.
- `src/inspector/format.ts`: exact location and performance-tempo formatting from typed Rust DTOs.
- `src/render/marks.ts`: explicit mark projection, exact timestamps, glyph selection and warnings.
- `src/render/project.ts`: staff grouping and notation projection. No JavaScript SFS parser is introduced.
- `tests/inspector.test.ts`: metadata, exact beats, labels, field staging and keyboard-state tests. Its minimal field harness does not claim browser rendering coverage.
- `tests/marks.test.ts`: MEI semantics, unsupported cases, staff scoping and actual installed Verovio SVG glyph output.
- `src/smoke.ts`: expanded real-WebKit assertions for selection, staged fields, focus, keyboard edits, local errors, explicit tempo/dynamics and restoration of the original example. These new assertions are **not yet executed** in UI2.

## Remaining acceptance

1. Run the smoke-feature executable on the provisioned native Linux desktop and retain a passing report.
2. Capture the original First light example at the first-pass 1180 × 812 client viewport and 100% zoom, with both Document and selected-note inspector views.
3. Open the expressive example and inspect the metronome and dynamics at native resolution, including a close-up. Verify real font loading and alignment in WebKit.
4. Manually check note click, Enter/Escape, Undo, staged second-property retention, split/score/inspector controls and native file actions. Refine any remaining visual issues before calling UI2 visually accepted.
5. Windows and macOS native builds/runtime remain unverified. Platform-neutral architecture and font fallbacks are not a substitute for those tests.
