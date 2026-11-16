# Ordered mixed notation v1 scalar transport

`!requires scorestream.notations@1` requires `scorestream.articulation@1` and a
complete score. It preserves mixed wrapper **topology**, not new semantics.
Existing typed articulation, ordinary tie, authored slur and fermata payloads
remain authoritative. No XML or arbitrary map is embedded.

| Kind | Required scalar fields |
| --- | --- |
| `scorestream.notations.event` | `event_index` |
| `scorestream.notations.child` | `event_index`, `wrapper_index`, `child_index`, `kind` |

All records target `#event` at the exact onset of the corresponding measured
pitched authored-articulation host. Indices are nonnegative integers. Event
indices are contiguous from zero and preserve association-list order. Child
indices are contiguous from zero **within each wrapper**. The wrappers, including
empty wrappers, are already represented by the authored articulation hierarchy;
this extension must neither synthesize nor omit wrappers.

The closed child choices and their additional required fields are:

- `kind=articulations group_index=N`: references the exact existing group in this
  wrapper, contiguous in group order. Every group must be referenced once.
- `kind=tied phase=start|stop`: references the corresponding ordinary sounding
  and notated tie endpoint on this host. A native `continue` relation expands to
  **two topology references**, stop then start; it is not MusicXML's unsupported
  graphical `<tied type="continue">`. Tie endpoint attributes/overrides reject.
- `kind=slur relation=ID phase=start|stop`: references the exact relation and
  endpoint. Requires the existing `scorestream.slur@1` contract. `stop` refers to
  the existing native relation's `end` phase. Relation IDs use native identifiers,
  not XML's local numbers or event references.
- `kind=fermata`: references the existing single event-local fermata.

`kind`, `phase` and `relation` are string scalars; the indices are integer
scalars. Fields from other choices and extra fields reject. Wrapper and group
source IDs and print-object values stay only in `scorestream.articulation.*`.

For each mixed host there must be exactly one topology event. It must contain at
least one supported non-articulation child, and include **every** supported tie,
slur and fermata endpoint on that host exactly once. Mixed articulation hosts
without topology reject. Legacy simple articulation overlap always rejects.
The order is preserved across wrappers as well as within them, and tied stop must
precede start. Unsupported or ambiguous endpoints are never inferred.

Public APIs: `notation_extension::encode_event` takes an event with pre-mapped
slur relation IDs plus the mapped host identity; `decode` reconstructs the typed
`NotationOrder` and validates its topology. Normal whole-document validation
also checks every existing family's own semantics. `notation_order::validate_score`
checks common-model references. Whole conversion runs these validations before
emitting output.

All current rendering and sounding routes remain explicitly blocked for this
capability and for its reserved marks even without the header. Unknown/future
versions fail closed. See [the implementation boundary](../../docs/MIXED_NOTATION_INTERCHANGE.md).
