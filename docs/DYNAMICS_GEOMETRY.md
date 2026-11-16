# Authored dynamics coordinates

The existing printed-dynamic lane now retains optional `default-x`, `default-y`,
`relative-x`, and `relative-y` on the MusicXML `dynamics` element. Each coordinate
is an exact signed finite-decimal value in tenths. Missing values remain absent,
including when another coordinate is explicitly zero. Decimal spelling is
canonicalized mathematically, not rounded or retained as arbitrary XML text.

The four typed `ScoreDynamic` fields map to native SFS `dynamic` attributes
`default_x`, `default_y`, `relative_x`, `relative_y`. XML parsing, public typed
model validation, and core SFS validation reject non-finite or non-decimal
values. No whole-score or coordinate precision cap was added.

Coordinates never determine the dynamic symbol or sounding level. Printed
MusicXML dynamics remain `sound=false`; numeric `sound dynamics` retains its own
scope, onset and exact percentage. Existing native symbolic sound semantics are
unchanged. This is source-fidelity support only: the current reflow view does not
promise to place dynamic marks at these authored coordinates.

Only a single supported symbol is accepted. Multiple symbols, `other-dynamics`,
words, custom glyphs, unmodeled font/layout attributes, and geometry attached to
the wrong element still fail rather than being discarded.

## Original evidence

The four pinned Hauptstimme Fifth originals contain 755 / 757 / 522 / 1221
`dynamics` nodes (3255 total). All have authored coordinates. Of those, 0 / 55 /
62 / 157 contain unsupported composite `other-dynamics` children. The corpus gate
checks unchanged source SHA-256 values, proves 2981 supported single-symbol nodes
round-trip through the real typed parser, canonical SFS, and XML on dedicated
minimal family fixtures, and verifies all 274 unsupported nodes still fail the
schema gate. It does not rewrite, strip, or claim full import of the originals.

Generated tests cover negative/positive/zero/fractional coordinates, independent
presence, exact playback audio/timing equality (diagnostic provenance remains source-specific), malformed XML decimals, unsupported
children, and invalid direct typed/SFS input. Existing dynamics tests still cover
sound offsets, staff scopes, performer-policy defaults and numeric dynamics.

Run `cargo test -p scorestream-formats --test dynamics`. For the original audit:
set `SCORESTREAM_MXL_FIXTURES` and run
`cargo test -p scorestream-formats original_dynamics_geometry --lib -- --ignored --nocapture`.
