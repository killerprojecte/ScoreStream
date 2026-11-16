# Explicit authored staccato/accent playback

`plan_source_with_explicit_articulations(source,
ExplicitArticulationPolicy::SimpleStaccatoAccentV1)` is a separate source-qualified
Rust API. It validates the unchanged SFS document, decodes its complete authored
articulation payload and projects the actual ordinary/repeated route. The returned
`ExplicitArticulationPlayback` exposes the playback plan and a read-only qualified
performance containing the original document, full authored records and selected
policy. No archival wrapper, grouping, coordinates, style, placement or mixed-family
order is replaced with a legacy mark in the source.

The caller explicitly chooses the existing simple performer policy:

- **Staccato:** a half-duration gate, not an authored MusicXML duration. Notes and
  markers keep their notated coordinates; the sounding release is shortened.
- **Accent:** add 16 velocity steps under the existing symbolic/default-dynamics
  policy, then use the existing engine's clamping. Direct numeric sound dynamics
  remain authoritative: accent plus numeric sound controls still rejects rather
  than silently changing the supplied dynamics.

Typed temporary performance directions provide these meanings to the existing
expression engine. `articulation.explicit_policy` records the choice in the plan.
Every retained item is visited: other signs and repeated stacks need another
explicit interpretation and reject, rather than being flattened or silently
omitted. Empty wrappers remain in the retained source/audit. Mixed tied notation is
retained and may play; a staccato on the final tie segment shortens only the final
release while the chain remains one attack. An interior staccato or accent on a
tied continuation needs a rearticulation/continuity decision and rejects.

## Original-corpus relevance and remaining work

A read-only census of the four SHA-pinned original Beethoven Fifth XML files found
3512 staccatos (441 / 729 / 216 / 2126) and four accents (0 / 0 / 1 / 3). Every item
has authored `default-x` and `default-y`; one also has `placement`. No such item is
on a fermata anchor or a sounding tie anchor in these files. These authored
coordinates are preserved, not interpreted as a different articulation.

The executable tests use synthetic MusicXML with those actual attribute forms,
roundtrip through the shared model/SFS, and validate numeric controls, accents,
mixed-family ties and repeated visits. This is not complete original-score import,
a cleaned Beethoven excerpt or an audio recording.

The standalone route does **not** silently compose with grace/fermata APIs.
The separate [explicit joint-policy constructor](COMBINED_PLAYBACK.md#explicit-articulation-grace-and-synchronized-hold-composition) now composes all three selected interpretations for disjoint articulations. A source requiring fermatas still requires a selected hold
policy, and grace timing still needs a selected grace allocation. Authored slurs,
unmapped signs, unsupported MIDI intent and other independent semantic requirements
remain explicit blockers. Ordinary `plan_source` and all previous common-release,
synchronized and combined APIs still reject authored articulation semantics when
no articulation policy was selected. The explicit joint constructor carries this policy, full source audit and
tie/hold interaction through the shared qualified route; removing a capability
guard alone is not that composition.

```sh
cargo test --locked -p scorestream-formats --test qualified_articulation_playback
```
