# Standard orchestral identities and built-in synthesis

This slice registers **51 standard instrument identities**, including auxiliary orchestral instruments. **40 pitched identities resolve to 25 simple additive synthesis profiles** in both live playback and WAV export. **11 unpitched percussion identities are cataloged but have no drum renderer yet.** These are convenient audible approximations, not recordings, a professional orchestral library, system MIDI sounds or VST3 support. Shared profiles are stated below rather than counted as unique acoustic instruments.

## One catalog, independent sound engines

The authority is `crates/scorestream-core/src/instruments/catalog.json`. Rust exposes `instruments::instrument_catalog()` and exact `resolve_instrument(&InstrumentId)`, with typed `InstrumentFamily`, `InstrumentPitchKind` and `InstrumentDefinition`. The existing open `InstrumentId` remains intact: a missing catalog match is a custom identity, not an invalid score.

`scripts/generate_instrument_catalog.py` produces the desktop's literal ID union, metadata and exact lookup. Run it after catalog edits; `--check` detects stale generated output. Desktop tests also compare the generated definitions with the original JSON. The exhaustive `BUILT_IN_PROFILES` map in `desktop/src/playback/timbres.ts` requires an explicit renderer/support decision whenever the catalog gains an identity. Backend programs, patches, sample assets and channel assignment do not belong in this catalog.

Resolution does **not** rewrite saved SFS, part names, MusicXML instrument labels or playback routes. It does **not** infer transposition, octave, clef, playable range, tuning, drum map or articulation. `piccolo C4` still sounds C4 unless the score explicitly requests an octave change. `clarinet` does not silently mean B-flat. Existing explicit chromatic/octave transposition remains the Rust authority. For example, `transpose_chromatic=0 transpose_diatonic=0 transpose_octaves=1` explicitly supplies the octave; the identifier alone does not.

Aliases are exact and case-sensitive. All underscore identifiers also have a hyphen spelling, such as `double_bass` / `double-bass`. `violoncello` resolves to `cello`, `cor_anglais` to `english_horn`, `french_horn` to `horn`, `tenor_trombone` to `trombone`, and `celeste` to `celesta`. See the catalog for the finite full list. There is no case folding, prefix matching, fuzzy display-name matching or object-prototype lookup. `Piano`, `flute.extra`, `constructor` and arbitrary custom IDs preserve the previous neutral triangle fallback for pitched notes.

## Pitched coverage

| Family | Canonical identities | Built-in profile policy |
| --- | --- | --- |
| Bowed strings | `violin`, `viola`, `cello`, `double_bass`, `strings` | One shared `strings` profile; the existing violin/cello/strings signal is unchanged. |
| Flutes | `piccolo`, `flute`, `alto_flute`, `bass_flute` | Distinct piccolo profile; the other three share the unchanged flute profile. |
| Double reeds | `oboe`, `english_horn`, `bassoon`, `contrabassoon` | Four distinct spectral profiles. |
| Clarinets | `clarinet`, `eb_clarinet`, `bass_clarinet` | Clarinet/E-flat clarinet share a profile; bass clarinet has its own. |
| Auxiliary saxophones | `soprano_saxophone`, `alto_saxophone`, `tenor_saxophone`, `baritone_saxophone` | One explicit saxophone profile. |
| Brass | `horn`, `trumpet`, `cornet`, `alto_trombone`, `trombone`, `bass_trombone`, `tuba`, `euphonium` | Horn, trumpet/cornet, three trombones, and tuba/euphonium use four profiles. |
| Harp/keyboards | `harp`, `piano`, `celesta`, `harpsichord`, `organ` | Five profiles; existing piano spectrum and decay unchanged. |
| Tuned percussion | `timpani`, `glockenspiel`, `xylophone`, `marimba`, `vibraphone`, `tubular_bells`, `crotales` | Six profiles with note-age decay; crotales shares glockenspiel. |

The Beethoven Fifth finale additions `piccolo`, `contrabassoon`, `alto_trombone`, `trombone` and `bass_trombone` therefore have explicit routes. Pitched `timpani` notes have a struck-decay profile. This coverage does not imply complete import of that score, automatic written/concert-pitch conversion, natural brass/reed attacks, true inharmonic bells, vibrato, round-robin samples or instrument-specific playing techniques.

## Unpitched boundary

`snare_drum`, `bass_drum`, `crash_cymbals`, `suspended_cymbal`, `triangle`, `tambourine`, `tam_tam`, `wood_block`, `castanets`, `whip` and `claves` have explicit unpitched catalog identity and **no built-in percussion voice**. The existing Rust `percussion_unmapped` diagnostic still omits `u.*` events. Catalog membership does not invent a pitch or General MIDI drum key. Assigning an ordinary pitched note to one of these unsupported IDs keeps the legacy neutral fallback; that is not percussion playback. In particular, pitched timpani and mallet instruments are not in this unpitched group.

MusicXML's existing strict conversion profile is unchanged. This change does not add `<midi-instrument>`, multi-instrument parts, `<unpitched>`, `<transpose>` or unsupported instrument-reference conversion. Those require separate adapter work and fidelity tests; display position must never be mistaken for sounding drum pitch.

## Signal and compatibility contract

Live and offline outputs still use the same `scheduleBuiltInVoice` graph. Rust owns frequency, timing, velocity and expression. New profiles only select harmonics and a local amplitude envelope; they neither change notes nor add nodes. Waves retain the existing matched steady-state energy policy. Struck profiles evaluate decay at the original note age on seek; envelope end points do not extend note duration. The master compressor, short gates, voice cancellation and bounded WAV export contract are unchanged.

The catalog adds no dependencies or sound assets. No device selection, network lookup, audio-driver configuration, Open/Save behavior or CI files are changed.

## Verification

- `python3 scripts/generate_instrument_catalog.py --check`: catalog shape/uniqueness and generated-file agreement.
- `cd desktop && npm test`: catalog coverage, exhaustive profile decisions, legacy output, exact aliases, unknown fallbacks, graph lifecycle and existing UI/transport regressions.
- `cd desktop && node --experimental-strip-types --test tests/pcm-encoding.mjs tests/offline-wav-render.mjs tests/wav-export.mjs`: live/WAV graph parity for all 40 pitched identities plus the existing bounded export regressions.
- `cd desktop && npm run build`: complete TypeScript and production bundle.
- `cd desktop && npm run test:audio-render`: real OfflineAudioContext sample checks for all pitched identities, 25 distinct profiles, shared-profile equality, spectra, silence boundaries, velocity, expression and sought note age. This needs a launchable Chromium sandbox and is not physical sound-device listening.
- `cargo test --workspace --locked`: includes catalog tests, raw identity preservation, no instrument-name transposition, explicit transposition changes and continued unpitched omission. Run `scripts/check.sh` for the repository's full Rust gate.

Rust/Cargo is unavailable in the implementation workspace, so Rust tests, rustfmt, clippy, aggregate checks and native desktop builds were not run there. The direct Chromium command was blocked by the workspace's socket restriction; a separate permitted browser execution is required for rendered-signal verification. Do not interpret TypeScript/mocked graph passes as a native desktop or physical-speaker pass.
