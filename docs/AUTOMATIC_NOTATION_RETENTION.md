# Automatic articulation retention

The Rust conversion API accepts `MusicXmlOptions.articulations =
ArticulationProfile::Automatic`. This is a shared entry-point policy for callers
that should not ask a person to choose an internal notation representation.
The existing default and the three explicit profiles are unchanged in this
increment. Desktop and CLI callers must opt into this API; this document does
not claim that either user interface has been wired to it.

## Selection at the note boundary

Both streaming schema passes and semantic lowering use the same classifier.
Selection is per note, not per document and not a retry after a failed legacy
parse. Notes without articulation groups retain their existing mapping
contract; this is not a general lossless-XML or complete-notation profile.

For articulation-bearing notes, the legacy mapping is used only for a canonical
single wrapper without wrapper/group attributes, a single nonempty articulation
group, and distinct simple signs in existing writer order (staccato,
staccatissimo, tenuto, accent), with only the existing optional placement.
The wrapper may additionally contain ordinary tied stop/start endpoints, a
fermata and a single slur, in writer order, before the articulation group.
The family validators still validate all semantic payloads. Classification
never makes malformed or unsupported data valid.

Other supported articulation hosts use the existing authored mixed mapping.
This retains empty/repeated groups, wrapper identities and visibility, typed
attributes, sign order and mixed-family child topology. Unrelated canonical
notes in the same score keep their legacy representation.

The classification concerns structural retention, not lexical XML identity:
existing normalization of numbers and generated identifiers still applies.
Unknown elements/attributes, malformed XML, unsupported hosts, unsupported
timelines and geometry still produce errors with the existing source
diagnostics. No geometry or measure policy is silently enabled.

## Consumer readiness is independent

Canonical four-sign scores and the canonical tied-release combination retain
the same model, native SFS and playback plan as the explicit legacy reader.
Canonical fermata/slur combinations retain the same model; their existing
consumer policies remain in force.

A file accepted by the old reader is **not** necessarily canonical. The old
reader could reorder signs or flatten multiple wrappers. Automatic retention
preserves these noncanonical cases in authored records instead. Such a score
may now be saveable with exact structure but blocked by current preview or
audio capability guards. This is an explicit compatibility limitation, not
proof that all previously playable scores remain playable. In particular,
noncanonical legacy-accepted tied/articulation combinations are tested as
preserved with audio withheld, not silently projected back into simple marks.

Each automatically retained host has a `Preserved` diagnostic,
`articulation.automatic_retention`, with the source note path and range. It
explains why retention was required and explicitly does not promise preview or
playback. The existing score-wide `articulation.authored_only` diagnostic also
remains. Callers should keep import diagnostics with the imported document and
separately report actual consumer readiness.

## MXL and streaming

Plain XML and MXL use the same reader with the same options. For MXL, callers
first use `mxl::open(input)`, then pass the verified root's `into_reader()` to
`musicxml::read_reader_with_options`. Container verification and resource
behavior are unchanged. Tests exercise both stored and deflated roots.

## Verification scope

`automatic_articulations.rs` tests canonical model/SFS/playback compatibility,
mixed-profile parity, per-note rich retention, noncanonical legacy success,
source diagnostics, explicit-profile behavior and streaming parity.
`mxl.rs::automatic_notation_retention_matches_plain_xml_for_stored_and_deflated_roots`
covers the container path.

These are authored fixtures, not acceptance of Beethoven's complete original
score, desktop Open/Save, native audio, or new rendering/playback semantics.
