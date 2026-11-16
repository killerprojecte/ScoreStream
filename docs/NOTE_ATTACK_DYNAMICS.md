# Note-local authored attack dynamics

MusicXML 4.0 defines note/@dynamics as a nonnegative NoteOn-velocity percentage
of forte90. It is not the separate `attack` timing attribute or persistent
`<sound dynamics>` scope state. [W3C note reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/note/).

`ScoreEvent.attack_dynamics`, `ScoreGraceNote.attack_dynamics` and the lowered
`ResolvedEvent.attack_dynamics` retain this exact optional rational value.
SFS uses an event-local `attack_dynamics` field. XML and native validation require
finite-decimal, nonnegative values on note owners, not rests/spacers. Missing is
not zero. Import has no new score-size or decimal-precision limit; values beyond
the current sound output range can be saved and exported without rounding.

## Qualified playback

The initial output policy covers measured pitched attacks, independently for each
chord member, when the existing loudness envelope is static through the actual
sounding interval. The explicit note value replaces that attack's inherited
loudness, without modifying any voice/staff/part assignment or following note.
Effective velocity is exact percentage × 9/10, represented by rounded nonzero
MIDI-relative velocity plus a constant corrective gain. Output outside 0–127 or
below representable floating-point precision fails rather than silently clamps.

An explicit zero produces no sounding note. Written/played event markers remain;
it is not emitted as a MIDI NoteOff command and does not stop an unrelated note.
This renderer policy also silences a qualified zero-valued tie-chain head.

A genuine tie-chain head overrides the one existing attack, retaining the normal
chain duration and no-retrigger behavior. An authored value on a continuation,
including stop+start, remains stored but raises
`note_dynamics.tie_continuation_unsupported`: no phantom attack or new sustain
value is invented. Printed-only ties are not confused with sounding continuity.

For nonzero values, changing underlying expression is rejected with
`note_dynamics.expression_unsupported`; this change does not guess the interaction
with a crescendo, transient accent, or changing scope dynamics. Grace notes and
principals whose actual attacks are moved by the explicit grace clock, cue notes,
and unpitched attacks remain stored but are outside this sound profile. Explicit
grace scheduling cannot silently bypass the owner check. Other unrelated events
and existing visual-only dynamic qualifiers retain their prior behavior.

## Evidence and limits

The four unchanged Hauptstimme Fifth originals have 682 note dynamics attributes,
all in movement I. Of these, 640 are untied, 16 start ties, 19 stop ties, and 7
stop and start ties. None are grace notes, chord members, or rests. Values range
from 54.44 to 132.22. The read-only corpus gate checks all four pinned SHA-256
hashes and projects each attribute onto a dedicated fixture preserving its tie
role. Every value survives typed/SFS/XML roundtrip; untied/head fixtures play,
continuation fixtures fail explicitly. This is not proof of complete-original
import/playback, particularly where the original underlying envelope changes.

Generated gates cover zero and absence, fractional effective loudness, independent
chord owners, later-note invariance, tie heads/tails, changing numeric dynamics,
range failures, invalid direct typed/native values, and grace preservation with
both ordinary and explicit-clock sound rejection.

Run `cargo test -p scorestream-formats --test note_dynamics`. For the original
audit set `SCORESTREAM_MXL_FIXTURES` and run
`cargo test -p scorestream-formats original_682_note_dynamics --lib -- --ignored --nocapture`.
