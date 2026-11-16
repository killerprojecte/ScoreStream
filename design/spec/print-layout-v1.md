# Authored print layout v1

`!requires scorestream.print@1` retains a deliberately closed MusicXML `<print>`
profile at a complete score's measure boundary. It does not implement pagination.

Each `scorestream.print` mark targets `#measure` at that measure's exact onset and
names the actual owning `part`. There is at most one per measure. The `layout`
choice is closed:

- `none`: an empty authored print element, with no dimensional fields.
- `system`: required exact rational tenths `left_margin`, `right_margin` and
  `top_system_distance`, representing one system-layout with system-margins and
  top-system-distance children in schema order.
- `staff`: required positive integer `staff` naming an existing staff in the
  owning part, and exact rational tenths `staff_distance`.

Optional boolean `new_page` and quoted text `page_number` retain absence, explicit
false, empty labels, and authored label whitespace independently. Page numbers
are labels, not guessed numeric indices. Dimensions allow signed exact finite
decimals without a geometry-specific magnitude or precision ceiling; absence is
not synthesized as zero. XML export normalizes decimal spelling without changing
values and uses character references for label whitespace where necessary.

The public typed model, native validator and adapters reject unknown fields,
unknown versions/kinds, mixed layout variants, duplicates, dangling identities,
foreign parts/staves, wrong onset, nondecimal rationals and illegal XML label
characters. XML rejects mid-measure print changes, multiple prints, new-system,
page-layout and other unimplemented print children rather than dropping them.
Only the explicitly described complete system/staff shapes are supported.

These records are passive for playback. Preview continues its independent reflow
and explicitly warns that authored print settings are not reproduced. Source and
XML export remain lossless for these typed fields. Other unsupported notation
keeps its own capability checks.

## Verification

- `cargo test -p scorestream-formats --test print_layout` exercises typed XML/SFS
  roundtrip, signed and zero values, absent/false flags, labels/whitespace,
  malformed/foreign fields, wrong ownership/onset, and unchanged playback.
- `desktop/tests/print-layout.test.ts` requires identical preview MEI plus an
  explicit disclosure without modifying the source snapshot.
- Opt-in `original_print_nodes_decode_and_export_without_field_loss` reads all
  four pinned original XML files through the production streaming walker, checks
  SHA-256 and literal MXL/XML equality, then decodes and exports each original
  print node. The receipt covers 3,355 nodes: 170 system and 3,185 staff layouts.
  Set `SCORESTREAM_MXL_FIXTURES` to the original corpus directory and run the test
  with `--ignored --nocapture`.

The original-node receipt is not a whole-score import claim. Credits, directions,
other geometry and musical notation can still block an untouched full import.
