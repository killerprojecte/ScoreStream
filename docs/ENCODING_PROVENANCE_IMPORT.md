# Authored encoding provenance and declared supports

The typed `ScoreDocument.provenance` preserves a narrow, descriptive subset of
MusicXML identification metadata. It is **not** an assertion that ScoreStream
supports the producer's declared elements, an instruction to fetch a source, or
a playback policy.

## Closed model

- `encoding: Option<Vec<EncodingEntry>>`: absence differs from an explicitly
  empty encoding container. Entries retain authored order and repetition.
- `EncodingEntry::Software { text }`: literal XML text, including an empty string,
  Unicode, leading/trailing whitespace and escaped XML characters.
- `EncodingEntry::EncodingDate { date }`: authored calendar-date spelling,
  validated as an XML date without timezone. No current/local date is inferred.
  Leap days, month lengths and XML Schema year syntax are checked without a
  fixed year-size limit; permitted XML whitespace remains preserved.
- `EncodingEntry::Supports { supported, element, attribute, value }`: required
  yes/no declaration and XML NMTOKEN element, with optional NMTOKEN attribute and
  token value. A value requires an attribute. Absent value differs from explicit
  empty value; no declaration is looked up as an adapter capability.
- `miscellaneous_empty: bool`: explicit empty miscellaneous container versus
  absence. Substantive miscellaneous fields are not discarded or wrapped as XML.

The official [encoding](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/encoding/)
shape permits ordered repetitions. The official
[supports](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/supports/)
element is empty and describes the producer's coverage. The
[encoding-date datatype](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/yyyy-mm-dd/)
is a timezone-free XML date. This bridge maps only software, encoding-date and
supports, the children found in the pinned originals. Encoder, encoding-description,
unknown children/attributes, substantive miscellaneous and mixed container text
remain explicit errors. No raw XML, arbitrary property map or new dependency is
introduced.

Existing score metadata, including ordered authors, rights and source description,
keeps its prior representation. The new optional field is absent in legacy JSON;
existing native files without the extension retain their prior behavior.

## Native transport and boundaries

`!requires scorestream.provenance@1` carries one `scorestream.provenance` header
at score time zero, targeted to `score`. Its `encoding_present` and
`miscellaneous_empty` booleans retain the container distinctions. Ordered
`scorestream.provenance.entry` marks use contiguous unique indices and a closed
software/encoding_date/supports discriminator with exact per-kind scalar fields.
Records can arrive in different traversal order without changing authored order.
Missing headers, foreign targets/onsets, fragments, unknown versions/kinds,
noncontiguous/duplicate indices and inapplicable fields reject.

Canonical SFS/JSON/XML round trips preserve values, entry order, repetition and
presence. XML serialization escapes passive text/attributes. Core playback
explicitly ignores validated producer metadata; complete playback plans are
identical with and without it. Preview does not put these claims into MEI or use
them to enable rendering. A claim that an unsupported element is supported never
makes an actual instance of that element accepted.

## Original evidence

All four untouched originals contain `MuseScore Studio 4.7.3`, followed by an
encoding date, followed by five supports rows in this order:

1. accidental, yes
2. beam, yes
3. print / new-page / yes, yes
4. print / new-system / yes, yes
5. stem, yes

Every original has explicit empty miscellaneous. Movement II's date is
`2026-07-14`; I/III/IV retain `2026-07-11`. These are authored producer claims,
not dates supplied or corrected by this importer.

| Movement | XML SHA-256 |
|---|---|
| I | `5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b` |
| II | `c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a` |
| III | `788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98` |
| IV | `cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298` |

Verification commands:

- `cargo test -p scorestream-formats --test provenance`
- `SCORESTREAM_MXL_FIXTURES=<originals> cargo test -p scorestream-formats --lib original_encoding_claims_and_empty_miscellaneous_roundtrip_in_authored_order -- --ignored`
- `desktop/tests/provenance.test.ts`

The original projection test uses the production XML walker, checks all four
fingerprints, validates each actual identification subtree, and round-trips its
complete typed encoding claims through SFS and XML in a minimal musical context.
It does not strip the full source or claim that unrelated whole-score import
blockers have been resolved.
