# Expression semantics, first typed-model slice

This is a performance-policy contract over the existing SFS 1.1 model. It adds no source syntax and does not claim a MusicXML/MCSZ importer or expanded rendering support.

## Resolution decisions fixed before implementation

1. A sustained dynamic is a replacing assignment, not an additive effect. Its interval is half-open: `[position, next assignment in the same scope)`. The final assignment ends at score end. At an exact boundary, the new assignment wins.
2. Scope specificity is `score < part < staff < voice < event`. The active most-specific assignment wins. A narrower assignment does not become permanent merely because it was once active: when a finite narrower interval ends, resolution uses the ancestor's value at the current position, including changes made while it was shadowed.
3. A dynamic targeted at an event remains event-local: its interval is the event's measured interval. It neither updates neighboring notes nor silently becomes voice-wide. An event mark can supply a hairpin endpoint level, but its own event-local assignment still has the higher scope rank. This is an explicit SFS playback policy, not a claim that every imported notation format has identical anchoring rules.
4. A hairpin is a bounded replacing dynamic curve, not a permanent velocity offset or multiplier. Current SFS anchored relations infer the start event's voice scope. The active interval is `[start onset, terminal anchor onset)`. The terminal anchor is excluded. An explicit endpoint dynamic has its normal scope interval from that endpoint onward; without one, the current underlying dynamic resumes.
5. A later explicit dynamic in the hairpin's same scope ends its effective interval at that assignment. A temporary narrower assignment shadows it only for that narrower interval; after the interval, the still-active ramp resumes at its current value. A broader dynamic does not destroy a narrower ramp, but supplies the current fallback after the ramp ends.
6. Same-scope overlapping hairpins are ambiguous in this initial policy and are rejected with object-specific diagnostics. Adjacent hairpins do not overlap because endpoints are half-open. Conflicting simultaneous sustained assignments in one scope are rejected rather than resolved by source order. Identical duplicates are idempotent.
7. An explicit start/end dynamic determines a hairpin's boundary level. Without an explicit end level, the policy uses a reported ±24 velocity-step target. Intervening same-scope dynamic terminates the ramp without silently treating that dynamic as a target at the original endpoint.
8. `sfz` is a transient attack envelope: it does not reset the sustained level to mf. `fp` is a compound forte attack followed by piano in the applicable scope. A normal sustained dynamic and a transient attack can coexist because they occupy different effect domains. Conflicting same-scope sustained assignments still fail.
9. Multiple attack accents select the strongest attack requirement, never sum arbitrary effects. Release-gate articulation is independent from dynamic level; a staccato + accent note is both shorter and accented. Multiple release constraints choose the shortest explicit gate; tenuto only replaces the ordinary gate.
10. Printed metronome signs and arbitrary words remain typed notation data, not unannounced playback tempo commands. Unknown required semantics are refused by SFS validation. Known directions with no realization must produce an explicit capability/diagnostic result rather than raw foreign markup passthrough.

## Required fixtures

- Same-staff p then f: p expires at f, no addition and no retained p offset.
- Score/part/staff/voice/event hierarchy with an ancestor change while an event-local interval is active: after that event, the newer ancestor value is used.
- Crescendo with explicit endpoints, later same-voice dynamic mid-span, bounded endpoint without persistent target, adjacent crescendo/diminuendo, and conflicting overlaps.
- sfz over p followed by the next note: sustained level remains p; fp produces forte attack and piano sustain.
- Accent + staccato, duplicate marks, reordered independent frame records, exact rational boundaries and existing v1.1 roundtrip compatibility.

The resolved model is format-neutral. Future MusicXML and MCSZ adapters must map their own anchoring/scope semantics into explicit model fields; they must not hand renderer-specific markup to the playback engine. Those adapters are not implemented in this slice.
