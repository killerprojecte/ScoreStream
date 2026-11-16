# Practice-only typed grouping and tuplet admission

The explicit practice profile may admit exactly `scorestream.part_groups@1` and
`scorestream.tuplets@1` after their existing closed decoders succeed. Neither was
added to the global passive-family set; ordinary/strict performance is unchanged.
Part groups retain engraved bracket/barline boundaries, not instrument or clock
routing. Tuplet child topology remains authored notation, not a second clock.

For the tuplet capability, every endpoint must have explicit measured duration,
written type and time-modification ratio; every time-modified event is checked
exactly against dotted written duration × normal/actual. Missing or inconsistent
evidence fails rather than changing time. Explicit normal-type/normal-dot notation
is retained and does not replace the authoritative measured interval. This matches
MusicXML's [time-modification semantics](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/time-modification/)
and [normal-type distinction](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/normal-type/).
The selected timing arithmetic budget also bounds the check; no score-size cap is
introduced. Ornament capabilities remain separately subject to the already selected
practice interpretation, never classified as passive tuplets.

Synthetic tests compare complete resolved events, tempos, end coordinate, hold
clock and grace windows with metadata present versus absent. The original source
is retained in the qualified receipt. Trace end/count agree. Unknown fields,
unknown capabilities and contradictory ratios reject; strict discovery remains
blocked and existing unsupported audible-articulation tests remain unchanged.

The exact original IV has2527time-modified events:2383eighth3:2 values of1/3quarter,
and144dotted-half6:4 values of2quarters with retained normal-eighth notation.
All pass the exact arithmetic evidence check without rewriting any event.

## Actual next boundaries

With verified source hashes and the explicitly selected TimGM6mb bank0/preset
routes, pure probes advance as follows:

- III: discovery/timing still reject `navigation.final_boundary_control`.
  The written final frame1122 contains16measure-owned dashes stops and18barlines.
  No record was omitted or moved to pass that boundary guard.
- IV: explicit-continuation practice discovery returns112grace occurrences and
  19fermata occurrences. Pure timing then rejects `grace.articulation_unsupported`.
  This change supplies no grace/principal articulation interpretation.

These are pure core results, not native mixer receipts, rendered audio or full
successful practice plans. Original timing-policy selections remain unchanged.
