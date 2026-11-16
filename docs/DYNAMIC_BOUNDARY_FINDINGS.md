# Dynamic boundary investigation (no behavior change)

Base: `97cc16defb4045ece97c320faf428bb140c3343e`.
This evidence commit does **not** remove the boundary rejection or establish a
playback precedence rule. It identifies why changing `>=` to `>` alone loses
information and cannot make the supplied original import valid.

## Authoritative semantics and exact original facts

MusicXML's [offset](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/offset/)
default for the sound flag is no. An absent flag offsets display, not sound.
A nested [sound offset](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/sound/)
overrides the direction sound offset. Default-x independently controls visual
appearance; the logical display offset below is not an inferred pixel position.

Both II/P6 and II/P8, ordinal 218 / label 217:
- inherited divisions **24**;
- source cursor **3/2** quarters, equal to the measure length;
- direction offset **-20** divisions, absent sound flag, no nested sound offset;
- logical printed p position **2/3**, numeric sound position **3/2**;
- explicit sound percentage **44.44 = 1111/25**;
- next measure ordinal 219 / label 218 opens at the same absolute time **326**
  with printed ff and sound **106.67 = 10667/100**.

P6 source measure starts at line 37778; its terminal direction is near line
37842. The two numeric assignments have the same part scope and exact time,
not merely nearby rounded times. Neither is at part end or score end. Moving
44.44 to 2/3 would change the supplied sound instruction. The printed p has
its own exact default-x/default-y/relative-y and must remain in label 217.

The full byte-pinned four-file inventory is
`BEETHOVEN_DYNAMIC_BOUNDARIES.json`, reproducible with:

    python scripts/audit_beethoven_dynamic_boundaries.py ORIGINALS --check docs/BEETHOVEN_DYNAMIC_BOUNDARIES.json

It uses Fraction throughout and records cursor, inherited divisions, source
child order, raw offsets/flags, separate locations, geometry, owning measure,
part end, and exact same-scope sound collisions.

Results:
- I: 755 printed/numeric directions, zero outside/terminal cases; seven equal
  sound-position groups, all agreeing percentages.
- II: 757 directions, exactly two sound-terminal cases above; two conflicting
  equal-position groups. No printed endpoint/cross-boundary case.
- III: 522 directions, no boundary cases; one equal agreeing group.
- IV: 1221 directions, no boundary cases; four equal agreeing groups.
- No negative/beyond-end dynamic locations, no part-final boundary assignment.
- Start-of-measure assignments remain distinct inventory entries. Source end
  and next start are matched by exact part-relative coordinates, not labels.

For I only, ordinal/label 268 has encoded extent 10 versus nominal 2 in all
18 parts. This audit explicitly records that existing irregular-bar evidence
and uses encoded source extent for inventory positions, not an importer
resize policy. II has no such exception, so time 326 needs no exception.

## Existing representation is insufficient

1. ScoreDynamic and ScoreSoundDynamics have absolute time + part/staff but no
   owning measure, shared source-direction identity, or encounter order.
2. Formats normalization sorts by scope/time, rejects differing same-time
   percentages, and deduplicates agreeing assignments. This is suitable for
   a unique executable state lane but not for all authored statements.
3. SFS writes numeric sound as state at a frame/scope. Its unique-state model
   cannot preserve these two assignments as separate authored events.
4. The XML exporter enumerates half-open measure ranges. Simply accepting an
   endpoint moves its origin to the next measure; final-part endpoints would
   disappear. Numeric sound is additionally emitted separately from visual
   marks in the part-scoped case, losing original sibling association.
5. Model validation itself excludes part-end times. This is separate from the
   read-side measure-boundary restriction.

The focused production-helper test parses the actual original directions,
verifies inherited divisions/cursors and both exact times/percentages, proves
that current normalization reports sound_dynamics.conflict, and verifies SFS
refuses rather than discards either assignment. No last-wins behavior is added.

## Scoped follow-up design, not implemented here

A separate typed authored dynamic-direction lane is the safe next unit:
- owning part + measure, source encounter index and exact cursor;
- optional printed dynamic payload with current independent geometry/qualifier;
- original direction offset presence/value/sound tri-state;
- optional numeric sound payload and nested offset presence/value;
- staff/placement and validated ordered wrapper structure.

Encode these as declared typed SFS records with owner/index links, not legacy
state duplicates or opaque XML. XML export must select by authored owner and
retain both source groups (including terminal group and following opening),
not route solely by half-open absolute time. Before claiming lossless support,
verify export/read of source cursor, grouping, offsets and exact positions.

Execution may separately define an explicit narrowly checked adjacent-measure
terminal-before-opening rule: same exact boundary/scope; terminal belongs to
previous measure, opening belongs to immediately next measure; no other
same-time assignments; stable source order preserved in authored lane. The
opening assignment can control new attacks while held ties stay unchanged.
This must be a documented projection from preserved authored data, not generic
last-wins normalization. Same-measure conflicts, mixed scopes, displaced or
nested sound offsets, part/score endpoints and any wider ordering remain
separate cases until tested. The generic legacy unique-state lane stays intact.

This reaches model, core capability validation, SFS, XML exporter scheduling,
and playback projection. It is not a safe one-condition importer patch.
