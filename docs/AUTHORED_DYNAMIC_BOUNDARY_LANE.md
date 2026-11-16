# Authored boundary dynamic statements

This adds lossless storage/import/export, **not a last-wins playback policy**.
It builds on the exact four-original inventory in DYNAMIC_BOUNDARY_FINDINGS.md.
The two original II/P6 and II/P8 cases retain printed p at 2/3 of label 217,
sound 44.44 at its 3/2 endpoint, and the next label 218's printed ff / sound
106.67 at the same absolute time. Neither value is discarded or shifted.

## Typed source lane

ScoreDocument.authored_dynamics holds AuthoredDynamic statements with:
- owning part and measure, encounter order among that measure's dynamic wrappers;
- exact source cursor relative to the owner, absolute visual and sounding time;
- one explicit printed sign and linked numeric percentage, independent staff,
  placement and four exact positional coordinates;
- optional direction offset in quarters, its optional sound flag, and optional
  nested sound offset in quarters. Absent, explicit zero and explicit no differ.

No raw XML, JSON string payload, label-to-volume conversion, synthetic musical
attack or guessed precedence is stored. Canonical export changes divisions and
lexical decimal spelling as existing adapters do, but retains exact values.
The semantics follow the official MusicXML [offset](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/offset/)
and [sound](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/sound/)
rules. Default-x remains independently authored visual geometry; it is not
converted to clock time.

The importer captures simple explicit sign + numeric sound wrappers. If a
numeric sound occurs exactly at its owning measure endpoint, that measure and
other captured measures with same-scope assignments at that exact boundary
enter the authored lane. All captured dynamic wrappers in each selected owner
move together, preserving encounter order. A measure must have a complete
capture of its printed and numeric dynamic statements; unsupported qualifiers,
other sound properties or disconnected standalone numeric statements are not
partially promoted. Ordinary directions elsewhere retain the existing lanes.
This is a closed supported family, not an arbitrary row-count limit.

The new scorestream.authored_dynamics@1 native extension uses individual flat
scalar fields on measure-targeted marks. A global index is contiguous; order
is contiguous within an owning measure. Marks live at the owning measure-start
frame, not at their sound's time. Both absolute times must equal the values
computed from owner start, cursor and the separate offset layers. The cursor
and sound may equal the owner's endpoint; printed positions remain inside the
owner for this profile. Every retained owner must connect to an explicitly
stored endpoint assignment. Owner/target/frame disagreement, duplicate/gapped
order or index, invalid scope, unknown fields and invalid exact values reject.

The legacy executable sound-state lane still requires one value per scope/time.
It is not relaxed. Source statements moved into the authored lane are not also
emitted as duplicate legacy states.

## Export and realization seam

XML output selects each statement by its authored measure, not by a half-open
absolute-time range. Each source wrapper keeps its linked printed/sound values
and offset layers. Balanced forward/backup cursor movement emits the wrapper
at its retained source cursor, then restores the exporter cursor before notes;
it adds no note or rest and does not change any note's timing. It preserves the
relative order of the captured dynamic wrappers, not byte-level XML order.
Part-final endpoint statements are retained rather than disappearing.

Playback explicitly fails with authored_dynamics.performance_required in both
written lowering and repeat/performed lowering. Preview likewise explicitly
withholds unsupported boundary statements instead of substituting an ordinary
dynamic or default gain. Imports report the preservation and performance seam.

Future execution must map both owners from written positions into each repeat
occurrence before applying a documented adjacent terminal-before-opening rule.
It must prove the owner adjacency and source ordering, keep tie-held attacks
unchanged, and handle final endpoints and other ambiguous cases separately.
None of those execution choices is made by this storage patch. Generic
last-wins, moving 44.44 to the printed p, and coalescing with 106.67 are forbidden.

## Targeted verification

- Formats authored_dynamics tests: exact XML/SFS/model roundtrips, independent
  absent/no/yes flags, nested sound offset zero, invalid ownership/time/fields,
  part-final endpoints, positive overrun rejection, multiple wrapper order,
  and explicit rejection through repeat-aware playback.
- Existing formats dynamics tests remain unchanged and pass.
- Ignored production walker original_two_boundary_pairs_preserve_linked_authored_statements
  verifies the entire original II SHA-256, inherited divisions 24 and actual
  original P6/P8 directions, then roundtrips both captured family projections.
  It is not a claim that all unrelated original score features import.
- Pure desktop test verifies explicit preview refusal without source mutation.

No full workspace suite or whole-original acceptance claim is included.
