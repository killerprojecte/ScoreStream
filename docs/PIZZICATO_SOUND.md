# Explicit pizzicato sound state

This closed interchange profile preserves MusicXML `sound pizzicato="yes"` and
`pizzicato="no"` on standalone authored words directions. The words and their
visual staff/placement/offset remain independent of the sound control. No text
recognition is used: `arco` with explicit `yes` still means pizzicato.

The [MusicXML 4.0 sound specification](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/sound/)
defines yes as pizzicato and no as arco, affecting following notes. Sound child
offsets override direction offsets. The profile retains an exact measure-relative
sound position, including rational positions different from the words position.
Export includes both in divisions calculation and writes the sound offset explicitly.
Controls apply to subsequent attacks in the owning part, not just the visual staff.
Same-time contradictory controls are rejected rather than arbitrarily ordered.

SFS requires both `scorestream.words@1` and `scorestream.pizzicato@1`. The latter
adds closed `pizzicato` (yes/no) and `pizzicato_at` exact rational fields to the
owned words record. Missing capability, partial payload and unknown values fail.
Import and roundtrip do not require a sound route and do not create placeholder notes.
Other sound owners and combined pizzicato+tempo/dynamics attributes remain outside
this first profile; separate supported tempo and dynamic directions are unchanged.

## Sound realization boundary

There is no verified selectable pizzicato timbre route in the current renderer.
The model reports `pizzicato.not_realized`, not successful rendition. Playback
checks the actual sounding attack, including qualified grace attacks, and rejects
an active authored yes state with an actionable error. Rest-only passages, cues,
omitted attacks, a later no before an audible attack, and terminal unused yes do
not cause a blanket rejection. Existing sustained notes are not re-attacked.

Repeat projection stores the sound state independently of tempo and velocity.
A jump restores the destination checkpoint, including an originally unspecified
state, so a previous pass's yes cannot leak into earlier arco/unspecified notes.
No new default timbre, guessed duration, automatic conversion or size cap is added.

## Evidence and scope

Untouched pinned Beethoven Op.67 originals contain 24 such controls: I 8, II 0,
III 6, IV 10. All use standalone words (`pizz.` / `arco`), only the pizzicato sound
attribute, no staff/voice and no explicit offsets. Their actual cursors include
I m264 at 1/2 and III m105/m247 at 2, not merely measure beginnings. The existing
SHA-qualified 286 standalone words family projection now retains all 24 controls,
with exact XML/model/SFS/model/XML equality. This is family evidence, not a claim
that all original orchestral content imports or renders. Separate native-note
regressions prove meaningful playback boundaries and no phantom events.
