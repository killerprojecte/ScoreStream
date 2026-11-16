# Authored note lyrics v1

`!requires scorestream.lyrics@1` preserves note-owned written lyrics as lyrics,
including texts that happen to look like musical analysis. It does not create or
populate a separate analytical-commentary module.

The supported MusicXML shape is one `<syllabic>` followed by one unformatted
`<text>` per `<lyric>`, with optional `number`, `color`, `default-x`, `default-y`,
`relative-x` and `relative-y` attributes. Multiple lyric entries on a note retain
their authored order. No verse identity, style default or coordinate is inferred.

The normative [MusicXML lyric reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/lyric/)
defines `number` as an optional NMTOKEN verse identifier, not a numeric counter.
This profile retains the identifier string and validates Unicode XML NameChar
syntax. Syllabic values are `single`, `begin`, `middle`, `end`. Text keeps decoded
XML whitespace and characters exactly, including empty text. Color preserves its
hexadecimal spelling. Coordinates are signed exact finite-decimal tenths without
an extra precision ceiling. Absent coordinates differ from explicit zero.

## Typed native representation

`ScoreDocument.lyrics` contains unique event-owned groups of ordered typed lyric
entries. Group list order is not semantic; order within each event group is.
Each native `scorestream.lyric` mark targets the exact owning `#event` at its
onset and carries required `index`, `syllabic`, and quoted `text`. Optional scalar
fields are `number`, `color`, `default_x`, `default_y`, `relative_x`, `relative_y`.
Per-event indices are unique and contiguous from zero; native record order is
not used as a substitute. Chord members and grace identities remain distinct.

Unknown fields/kinds/versions, empty groups, duplicate owners/indices, index
gaps, wrong hosts/onsets, invalid colors/verse identifiers, illegal XML characters
and nonterminating rational coordinates reject. XML forms involving elision,
extend, multiple text runs, formatting, missing syllabic, or other attributes are
not silently flattened or discarded. They remain explicitly unsupported.

Validated lyrics are passive for playback. The current preview does not draw
lyrics and explicitly discloses that omission; lyric presence alone does not
withhold valid musical preview or audio. XML export retains all supported fields
and uses XML character references where needed to retain whitespace.

## Verification

- `cargo test -p scorestream-formats --test lyrics`: typed XML/SFS/XML retention,
  ordered verses, text/whitespace, style absence/zero, malformed and unsupported
  forms, host/onset/index validation, chord/grace ownership, 91-digit decimals,
  and identical audio plans with/without lyrics.
- `desktop/tests/lyrics.test.ts`: preview MEI remains unchanged and reports the
  omission once without modifying the source snapshot.
- Opt-in `original_lyric_nodes_roundtrip_with_owner_and_order`, with
  `SCORESTREAM_MXL_FIXTURES` and `--ignored --nocapture`, checks untouched source
  SHA-256, literal XML/MXL equality, and all 593 original lyric nodes through the
  production decoder/exporter. Movement counts are 225/92/114/162, including all
  seven default-x and two relative-x cases.

This receipt is not a whole-original import claim. Unrelated notation can still
block full import. It also does not claim lyric engraving has been implemented.
