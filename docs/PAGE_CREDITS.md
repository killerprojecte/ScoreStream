# Authored page credits

The focused MusicXML profile preserves ordered page-credit text independently
of score metadata, page layout, and audio. It does not claim a page renderer.

Supported shape: `<credit page="...">`, optional single `credit-type`, followed
by exactly one `credit-words`. Text is preserved, including whitespace, Unicode,
and decoded XML references. Optional `default-x` / `default-y` are exact signed
finite decimals; numeric `font-size` is positive. `justify` accepts left, center,
right; `valign` accepts top, middle, bottom, baseline. Missing fields remain
missing, distinct from explicit page 1, zero coordinates, or explicit alignment.
Page numbers use positive u32; a larger number fails rather than truncating.
Decimal values are preserved mathematically, not their lexical spelling.

Images, links/bookmarks, multiple credit types or text runs, relative coordinates,
font families/styles, named font sizes, and other unmodeled attributes still fail.
There is no foreign XML payload and no silent stripping of unmodeled children.

`ScoreDocument.credits` contains typed `Credit` values. SFS requires
`scorestream.credits@1` and one `scorestream.credit` mark per credit, targeting
`score` at time zero. Explicit zero-based contiguous indices preserve authored
order even when records are canonicalized. Every mark carries text; page, type,
coordinates, font size and alignment are optional fields. Decoding validates the
extension, host, onset, index bijection and value domain. An empty required
extension is invalid. Existing documents without credits retain their old shape.

## Evidence and limits

The pinned Hauptstimme Beethoven Fifth originals contain 42 / 33 / 45 / 62
credits (182 total). Each has page, one type and one words element. Each movement
has title, subtitle and composer once; rights counts are 39 / 30 / 42 / 59.
Only the four composer credits omit font-size. The ignored corpus gate reads the
unchanged originals, checks their pinned SHA-256, lowers every credit through the
actual typed parser, and round-trips those credits on a dedicated minimal score
through canonical SFS and XML. This is a credit-family audit, not proof that the
complete originals import: other unsupported families remain explicit errors.

Generated integration gates cover exact whitespace/entities, signed decimals,
absence versus zero, ordering, malformed wire and unsupported structures. A
playback-plan comparison proves adding these credits changes no generated sound.
Existing parser/token budgets remain; no whole-score byte cap was added.

Run `cargo test -p scorestream-formats --test credits`. For the read-only corpus
check, set `SCORESTREAM_MXL_FIXTURES` to the original XML/MXL directory and run
`cargo test -p scorestream-formats original_beethoven_182_credits -- --ignored --nocapture`.
