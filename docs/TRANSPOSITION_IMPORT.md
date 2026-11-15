# Typed transposition import

This is one semantic slice, not full Beethoven or full MusicXML import support. The four focused fixtures retain selected pitch, key, division, duration and transposition values from Hauptstimme's Beethoven Fifth. They deliberately omit other notation and replace surrounding part identity with simple test identifiers. They are not original full-score import evidence and must not be presented as such.

## Preserved meaning

`ScoreDocument` keeps `StaffState.transpose` alongside the written key, clef and meter. `Transposition` stores exact rational chromatic displacement, optional diatonic spelling displacement and integer octave displacement. It does not rewrite the written note or key. The existing SFS notation lowering adds chromatic displacement plus twelve times the octave displacement exactly once when finding sounding pitch.

MusicXML `<transpose>` at a measure's start supports:

- Required `<chromatic>`, including exact finite fractional semitones.
- Optional `<diatonic>`; omission remains `None`, not an invented zero interval.
- Optional `<octave-change>`; omission means no additional octave displacement.
- Optional `number` selecting a valid staff. Without `number`, one identical state is assigned to every declared staff.
- Later measure-start changes, inheritance over measures without a new assignment, and explicit zero resets.

The written `<key><fifths>` remains unchanged. Concert key is not inferred from a natural-horn display key, from instrument names or from playback transposition. For example, the source E-flat horn uses written key 0 with displacement -9 semitones. This does not authorize replacing its printed key with a guessed concert key.

MusicXML export writes the typed interval, preserves optional diatonic absence, and emits explicit per-staff assignments. Chromatic values require an exact terminating decimal of at most 32 places and 64 characters including sign and decimal point. A rational such as 1/3 can remain exact in the common model and SFS, but MusicXML export fails rather than rounding it.

## Explicit boundaries

- MusicXML `<double>` is not supported. It describes an additional doubled sounding part, not another transposition offset. It remains a hard diagnostic.
- Mid-measure state changes are not represented by this measure-start profile. MusicXML and SFS adapters report an error, rather than applying a change in document order or moving it to a boundary.
- SFS staff-scope transposition maps both ways using the existing required chromatic/diatonic/octave tuple. SFS part-scope transposition remains unsupported by this bridge; the original SFS parser and playback model still support it.
- MusicXML without `<diatonic>` round-trips through MusicXML and the common model. SFS export explicitly rejects that case because its required tuple cannot distinguish omission from an authored zero.
- Unknown attributes, unknown children, malformed numeric values, missing chromatic values, repeated values, invalid staff numbers and conflicting assignments remain errors. No broader tags are ignored to admit a full score.
- This slice changes no SFS wire syntax, playback policy, renderer, desktop file chooser, archive loader or audio device behavior.

## Focused acceptance

`cargo test --locked -p scorestream-formats --test transposition` covers:

| Fixture | Source pitch and part | Transposition | Sounding pitch | Written key |
| --- | --- | --- | --- | --- |
| transpose-bb-clarinet.musicxml | Movement I, P5, first pitched note A4 in measure 1 | diatonic -1, chromatic -2 | G4, MIDI 67 | -1 |
| transpose-eb-horn.musicxml | Movement I, P9, first pitched note E5 in measure 18 | diatonic -5, chromatic -9 | G4, MIDI 67 | 0 |
| transpose-piccolo.musicxml | Movement IV, P1, first pitched note C5 in measure 1 | diatonic 0, chromatic 0, octave +1 | C6, MIDI 84 | 0 |
| transpose-contrabass.musicxml | Movement I, P18, first pitched note G3 in measure 1 | diatonic 0, chromatic 0, octave -1 | G2, MIDI 43 | -3 |

The tests execute MusicXML → common model → SFS → the real playback-plan function, as well as XML/common/XML and SFS/common/SFS round-trips. They assert written note/key preservation, sounding MIDI notes, multi-staff isolation, inherited state, later reset, fractional intervals and the fail-closed boundaries above. The existing application already realizes the same SFS tuple; the tests ensure the new adapter reaches that path rather than applying another shift.

Source score values were inspected from `data/Beethoven,_Ludwig_van/Symphony_No.5,_Op.67/{1,4}/Beethoven_Op.67_{1,4}.mxl` in [Hauptstimme](https://github.com/MarkGotham/Hauptstimme), pinned repository tree `8f677853c45b74ca0afcd79677e5379c06c82354`. The embedded scores declare the score CC0 and annotations CC-By-SA. These fixtures contain the cited musical values, not the annotation layer.

Primary semantic reference: [MusicXML 4.0 transpose](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/transpose/).
