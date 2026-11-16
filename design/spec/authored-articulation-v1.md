# Authored articulation v1 scalar transport

`!requires scorestream.articulation@1` opts a complete SFS score into authored-only
articulation fidelity. It grants no unrestricted rendering or performance semantics;
desktop preview uses the separate bounded adapter described below. The common
model uses `AuthoredArticulations` with the closed `ArticulationVersion::V1` value.
Legacy native `articulation` marks retain their existing independent contract.

Each authored record is an event-targeted mark at the measured pitched host's
exact onset. Four registered kinds encode the ordered hierarchy:

| Kind | Required indices | Other fields |
| --- | --- | --- |
| `scorestream.articulation.event` | `event_index` | none |
| `scorestream.articulation.notations` | `event_index`, `wrapper_index` | optional `source_id`, `print_object` |
| `scorestream.articulation.group` | `event_index`, `wrapper_index`, `group_index` | optional `source_id` |
| `scorestream.articulation.item` | all preceding plus `item_index` | required `sign`, typed display fields |

Indices are nonnegative integer scalars, contiguous from zero in each parent.
They preserve event order independently of chronological frame order and preserve
wrapper, group, item, and duplicate-sign order. Empty wrappers and groups have
explicit records. An event must contain at least one group somewhere; additional
empty wrappers are retained. An empty top-level capability is rejected.

`source_id` is an optional authored XML source identity, never a generated native
mark ID. Source IDs must be unique across wrappers/groups, use the supported ASCII
XML ID subset, and not collide with native part, voice, measure, event, chord,
relation or mark identities. `print_object` is an optional boolean; omission is
not normalized to true. All other optional fields below use string scalars.

Common item fields: `placement`, `default_x`, `default_y`, `relative_x`,
`relative_y`, `font_family`, `font_style`, `font_size_points`, `font_size_css`,
`font_weight`, `color`. Coordinates and point sizes use exact rational strings;
font sizes are either points or a CSS size, never both. Values and omission are
preserved as typed data, not inferred engraving defaults.

The closed sign vocabulary is accent, strong-accent, staccato, tenuto,
detached-legato, staccatissimo, spiccato, scoop, plop, doit, falloff, breath-mark,
caesura, stress, unstress, soft-accent and other-articulation.

- Only strong-accent accepts `orientation` (up/down).
- Only scoop/plop/doit/falloff accept `line_shape`, `line_type`, `line_length`,
  `dash_length`, `space_length`; the last two use exact rational strings.
- Only breath-mark/caesura accept `shape`, using their distinct closed enums.
- Only other-articulation accepts required `text` (including explicit empty text)
  and optional `smufl`. They are passive authored text and glyph names, not markup,
  executable instructions, a validated glyph resource, or performance intent.

Unknown fields, inapplicable sign-specific fields, wrong scalar types, unknown
kinds/versions, missing parent records, duplicate or gapped indices, mismatched
hierarchy targets/onsets, ambiguous or ineligible hosts, and mixed legacy
articulation/tie/slur/fermata notation on the same host fail validation. The
family-only v1 wrapper model intentionally cannot flatten mixed-family wrappers.

Core lowering/performance still reject both the capability header and actual
reserved authored mark kinds, including header bypass attempts. No sound policy
is inferred. Desktop preview uses the separate bounded typed adapter documented
in [Authored articulation engraving](../../docs/AUTHORED_ARTICULATION_ENGRAVING.md).
It validates the complete hierarchy and rejects unsupported visible objects;
header-only, missing-header and future-version inputs remain fail-closed. Its
ordered receipts preserve wrappers, groups, hidden objects and compound-component
identity separately from renderer-specific MEI. The four existing simple native
articulation signs remain unchanged.

Public core APIs: `articulation_extension::encode_event` encodes an ordered typed
event with its caller-mapped native ID; `decode` reconstructs and validates the
complete authored family. The common-to-SFS adapter validates the score, emits
these records, and validates the resulting native document. The SFS-to-common
adapter runs native validation before decoding and common validation afterward.

A separate `scorestream.notations@1` capability can supply a validated mixed-family
child topology for these wrappers; this does not silently broaden articulation v1
alone. See [Mixed notation v1](mixed-notation-v1.md).
