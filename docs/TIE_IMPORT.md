# Ordinary MusicXML ties through the shared score

This is a strict stepping-stone toward original orchestral score import, not a claim that Beethoven's original full scores now convert. Original files still contain other unsupported musical and engraving domains. No notes or symbols are stripped to make conversion pass.

## Model and exact mapping

`ScoreDocument.ties` contains typed `ScoreTie` records with a stable `RelationId` and an ordered list of event IDs. An ordinary tie means both sounding continuity and printed notation. The notes remain separate events with their original written pitches, durations, staff/voice identities, and measure membership. No XML nodes, attribute dictionaries, merged-note replacements, or renderer payloads enter this model.

MusicXML `<tie>` describes sound and `<tied>` describes notation. The initial profile requires matching start/stop phases in both. A middle note can stop one tie and start the next; the notated stop must precede the start. The importer resolves each link after collecting the typed events, so XML encounter order around `backup` cannot reverse musical time.

Pairing uses part, voice, exact effective sounding pitch, exact preceding tail time, and the local `tied` number (default 1, range 1–16). All endpoints at a boundary are paired together. Only unique one-to-one same-staff pairs are consumed, followed by at most one remaining cross-staff pair. Converging/diverging unisons and multiple residual cross-staff choices fail; destination-ID or XML encounter order never chooses among competing sources. Effective pitch comes from the written pitch plus each staff's inherited chromatic/octave transposition. Enharmonic spellings and changes of written pitch under an exact compensating transposition retain their spelling.

MusicXML's number is a local disambiguator rather than a globally persistent relation ID. Import explicitly reports its normalization to stable common relation/event identity. The SFS bridge preserves that identity and all anchors through existing `~ tie start/continue/end` records. It adds no SFS syntax or arbitrary attributes. XML export regenerates local endpoint numbers and reports the loss of SFS chain-ID grouping; individual note IDs and the directed tie graph remain intact. More than 16 concurrent indistinguishable same-voice/pitch links fail export rather than reuse a conflicting number.

Public-model validation rejects dangling IDs, repeated/empty relation identity, fewer than two anchors, rests, pitch changes, noncontiguous timing, cross-part/cross-voice links, and duplicate or branching sounding graphs. Cross-staff ties are valid when sounding pitch and timing agree.

## Explicitly unsupported

- Print-only or sound-only ties, including contradictory sound/notation endpoints. The broader SFS notation engine has separate flags, but this first conversion profile does not pretend those forms have been verified end to end.
- Repeat-conditioned `time-only` sound ties, starts followed by stops on the same note, ties into/out of repeats or codas, `let-ring`, and layout `continue`. The latter is an engraving continuation, not a new sounding note in a chain.
- Endpoint XML IDs, placement/orientation/Bezier/style attributes, or other engraving data without a verified common/SFS representation. Unknown notation children still fail with an element-specific diagnostic.
- Unresolved/ambiguous ties, cross-voice pairing, or unpitched ties. The importer does not invent missing endpoints or discard notation.

Every rejected mapping returns a diagnostic and no partial score. The original symbol data is not silently removed. The new `ties` JSON field is optional on read, omitted when empty, and rejects unrecognized fields just like the rest of the common model.

## Playback proof and limits

The CC0 fixture `ties-transposing-chain.musicxml` is an original three-bar test miniature, not a Beethoven excerpt. It covers a transposing clarinet chain, two numbered links, an explicit transposition reset, and changed written pitch with equal sounding pitch. The path is MusicXML → common model → SFS text → the actual `scorestream_app::playback::plan_source` compiler. It produces one attack at MIDI 67 while retaining all three written event intervals and all three bars.

No playback policy is changed. At the existing default 120 BPM and 92% final-note gate, the three quarter notes produce one 1.46-second sounding note, with the two internal bar boundaries fully sustained and the final written boundary at 1.5 seconds. An additional test applies the existing explicit tenuto articulation to the final SFS note and obtains the full 1.5-second sound. This is not a per-bar shortening and no continuation is reattacked.

A compressed MXL integration test also verifies the same miniature through the integrity-checked archive root and streaming reader. Further tests cover common/SFS relation identity, SFS `continue` chains, local numbering, same-pitch unisons, voice and part isolation, reverse XML encounter order, independent staff transposition, enharmonic pitch, ambiguous/gapped/dangling endpoints, public-model corruption, and CLI no-partial-output behavior. This slice does not certify desktop engraving, sample-library realism, repeat performance, or successful full-original import.

```sh
cargo test --locked -p scorestream-formats --test ties
cargo test --locked -p scorestream-cli --test ties
cargo test --workspace --locked
```

## Primary references

- [MusicXML sound tie](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/tie/)
- [MusicXML notated tied](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/tied/)
- [MusicXML number-level and document order](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/number-level/)
