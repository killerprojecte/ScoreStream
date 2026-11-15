# Initial instrument identity and MIDI intent

This is a bounded conversion capability, not a sample library, MIDI device connection, or claim of complete orchestral MusicXML support.

## Contract

`ScorePart.instrument` remains the canonical catalog identity used by existing routes. Its optional `instrument_metadata` stores a distinct, typed initial declaration:

- `source_id` is the document-scoped MusicXML `score-instrument` ID. `name` and optional `abbreviation` are exact Unicode instrument labels, separate from the part's display name.
- Optional `sound` retains the exact MusicXML standard instrument-sound identifier. The importer uses a finite reviewed mapping to the existing catalog; it does not infer identity from a MIDI program, language, substring, family prefix, transposition, or octave.
- Optional `midi_device` retains its text, optional instrument IDREF and optional port. An empty device element is different from no element. A missing port remains unassigned, never port 1.
- Optional `midi_instrument` retains its required IDREF and optional channel, program, volume and pan. Channel/port are 1–16 and program is 1–128 as written in MusicXML. These are source values, not backend zero-based assignments or automatic General MIDI bank selection.
- Volume is an exact rational percentage from 0 through 100. Pan is exact degrees from −180 through 180. Neither is a note velocity, score dynamic, nor pre-quantized MIDI CC value. XML decimals convert exactly, without floating-point intermediates. Non-terminating rationals survive SFS but MusicXML export rejects rather than rounds them.

The first profile supports one initial score-instrument and at most one device and one MIDI-instrument per part. All present IDREFs must address that part's sole source ID. Source IDs use the same supported ASCII XML-ID subset as the importer/exporter. Invalid/duplicate IDs, mismatched references, multiple declarations/devices, note instrument selectors, mid-score MIDI changes, banks, kits, virtual instruments, elevation and all other unmapped fields fail closed. This is not a full MusicXML XSD implementation.

## SFS representation

A complete document declares `!requires scorestream.instrument@1` and uses one typed `+ <part> scorestream.instrument` mark at `@0` per part with metadata. The required fields are `source_id` and `name`. `device` string presence retains the device record, including `device=""`; `midi_ref` presence retains the MIDI record, even without a program. Numeric fields cannot appear without their respective records. The language descriptor and AST schema specify field types; semantic validation additionally checks ownership, uniqueness, references, catalog consistency, and ranges. Unknown versions and empty capability declarations reject.

Example record:

```
+ P1 scorestream.instrument channel=12 device="" device_ref="P1-I1" midi_ref="P1-I1" name="B♭ Clarinet" pan=0 program=72 sound="wind.reed.clarinet.bflat" source_id="P1-I1" volume=393701/5000
```

A part label such as `display_name="Clarinet I"` remains independent. Source identity is preserved through MusicXML → shared model → SFS → shared model → MusicXML, even when SFS identifiers require normalization. Retained source IDs are reserved when generated SFS or XML IDs are allocated.

## Catalog projection and playback boundary

The reviewed sound mapping covers the 17 identifiers present in the four pinned Beethoven Fifth source payloads: flute, piccolo, oboe, clarinet, B-flat clarinet, bassoon, contrabassoon, natural horn, C trumpet, alto/tenor/bass trombone, timpani, violin, viola, cello and contrabass. B-flat clarinet, natural horn, C trumpet and tenor trombone map to broader existing catalog identities; the raw subtype remains in metadata and conversion reports the broader projection. An absent sound can use an exact catalog ID/alias or exact catalog display label. An explicitly unknown sound never falls back to name/program guessing. This finite bridge does not imply every sound in the MusicXML list is implemented.

Plain identity/label metadata preserves existing catalog playback, pitch transposition, and explicit later SFS instrument states. Such later states remain unsupported by the shared-model MusicXML adapter rather than being dropped. Directly changing a shared model's canonical route while retaining conflicting source metadata is invalid; callers must explicitly reauthor/remove the metadata.

Any authored MIDI device or instrument record, including an empty record, blocks current playback with `instrument.performance_unsupported`. Current synthesized and sampled paths do not realize these source ports/programs/volume/pan. Silently playing a default patch would violate the retained intent. No OS device is opened and no external destination is selected by import. A future backend must select an explicit route interpretation, resolve library/program compatibility and treat missing values as unassigned. Identity alone does not promise a distinct or realistic sample.

## Verification scope

Ordinary tests exercise typed JSON and XML/SFS semantic round trips, all 17 sound mappings, unchanged explicit transposition, absent/empty/zero distinctions, boundary ranges, IDREF and unknown-field rejection, source-ID collisions, and playback gating. Existing generated playback fixtures change diagnostic source positions only because the canonical SFS includes an additional requirement and mark; notes, clocks, gain, routes and policies remain identical.

An opt-in test reads all four **unchanged** original XML payloads, pins their SHA-256 values, and validates/round-trips each instrument declaration via production mapping functions. It verifies 77 declarations (18/18/18/23), 17 sound identifiers, 27 names containing spaces and the exact port histogram: absent 2, port 1: 47, port 2: 24, port 3: 4. All source volumes remain 78.7402 and pans remain 0. This metadata-only audit does not remove unsupported notation or claim whole-score import/rendering.

```
cargo test --locked -p scorestream-core --test instrument_metadata
cargo test --locked -p scorestream-formats --test instrument_metadata
SCORESTREAM_MXL_FIXTURES=/path/to/pinned/inputs cargo test --locked -p scorestream-formats original_beethoven_77_instrument -- --ignored
```

## Normative references

- [MusicXML score-instrument](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/score-instrument/), [instrument-sound](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/instrument-sound/) and [sound identifier list](https://www.w3.org/2021/06/musicxml40/listings/sounds.xml/).
- [midi-device](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/midi-device/), [midi-instrument](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/midi-instrument/), [program](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/midi-program/), [channel](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/midi-channel/), [volume](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/volume/), [pan](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/pan/).
- [transpose](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/transpose/): written-to-sounding pitch remains explicit and independent of instrument identity.
