# Bounded ordered mixed-family interchange

`MusicXmlOptions.articulations = ArticulationProfile::AuthoredMixedV1` is an
explicit input-only opt-in. It accepts the existing authored articulation family
on measured pitched notes together with the existing ordinary ties, authored
slur endpoints and single fermata at the **same event**. `LegacySimple` and
`AuthoredV1` keep their previous contracts. This is not the desktop/CLI default,
full MusicXML notation support, new engraving, or a new performance policy.

## One semantic model, explicit source topology

`ScoreDocument.notation_order` is an optional closed version-1 typed topology.
It references existing typed payloads rather than duplicating them:

- The event and ordered wrapper list refer to `authored_articulations` hosts.
  Wrapper ID and absent/yes/no `print-object` stay in that hierarchy.
- An `articulations` child references a group index in that exact wrapper.
  Empty/repeated groups, duplicate signs and their attributes remain unchanged.
- A `tied` child references the host's ordinary start or stop endpoint. The
  existing tie graph remains the only source of sounding continuity.
- A `slur` child references its stable relation ID and start/stop endpoint.
  Distinct parallel slurs and independent endpoint geometry do not coalesce.
- A `fermata` child references the existing single event-local authored sign.

Every group and supported semantic endpoint must appear exactly once. Missing,
duplicate, wrong-host, foreign, reordered group or dangling references reject.
The wrapper count must match, including empty wrappers. Ordinary tied stop must
still precede tied start across the host's complete wrapper sequence. No raw XML,
free-form attributes, display defaults or sound recipes enter this model.

The XML importer resolves local slur numbers with transient source positions;
only typed relation references survive. Export allocates slur numbers against
the **actual authored endpoint order**, including open-before-close order where
that order needs an additional local number. It never sorts the children to
make numbering convenient. Source wrapper/group IDs are reserved before generated
relation identities; topology references follow any reported normalization.
SFS preserves the event-association list order independently of frame order.
XML necessarily follows its voice/note serialization and reports a normalized
top-level association order while retaining each host's exact child order.

## Separate sound and print semantics

`<tie>` sound endpoints and `<tied>` notation endpoints continue to require a
matching ordinary pair. A hidden notations wrapper is preserved as a print
choice; it does not silence a sounding tie or convert a graphical curve into
one. Print-only/sound-only ties, `continue`, `let-ring`, start-before-stop repeat
shapes, `time-only`, endpoint layout/style and other previously unsupported tie
forms still reject. A let-ring is never joined to the next same-pitch note.
This checkpoint does not supply route-aware repeat tie or articulation playback.

## Native contract and runtime boundary

`scorestream.notations@1` is mandatory alongside `scorestream.articulation@1`.
It transports scalar event and ordered child records, referencing the existing
authored hierarchy and ordinary tie/slur/fermata records. See the
[wire specification](../design/spec/mixed-notation-v1.md). Unknown versions,
unknown/inapplicable fields, wrong scalars, gapped/duplicate indices, mismatched
target/onset, missing parents/capabilities and extra/missing endpoints fail closed.
Native tie endpoint overrides, even explicit `sound=true` / `print=true`, remain
outside this bounded mixed wire bridge; no flags are stripped during conversion.

All ordinary and qualified performance lowering routes reject the new capability
and actual reserved marks independently, including missing-header bypasses.
Desktop projection likewise refuses the complete mixed preview, before emitting
partial MEI. Header-only, future-version and raw-mark bypasses also refuse.
No optical receipt, sound test or pause/legato realization is claimed by an
interchange test.

## Exact remaining gaps

- This is a topology bridge for **articulation-bearing measured pitched hosts**,
  not a universal notations container. Wrappers with only tie/slur/fermata and
  no articulation group on the host retain their previous bounded profile.
- Rest, unpitched and grace articulation anchors remain unsupported. Existing
  slurs may still end on a separately supported grace event, but no mixed
  articulation topology is admitted there.
- Ornaments, technical marks, tuplets in notations, editorial footnote/level,
  additional wrapper attributes and arbitrary extensions remain unsupported.
- Existing slur and fermata field/anchor/number limits remain. Only one fermata
  per event is supported. Source local tie/slur numbers are disambiguators,
  not persistent identities, and are explicitly normalized as before.
- Existing exact-decimal budgets and ASCII source-ID limits remain. No rounding,
  note deletion, endpoint repair or metadata removal is used to make import work.

## Evidence

The committed `mixed-authored-notations.musicxml` is a new synthetic three-note
study, not an edited external score. Its JSON preview snapshot is generated by
the real MusicXML → common model → SFS → Workspace pipeline. The checked-in
frontend test verifies that its complete mixed preview is withheld unchanged.
Tests cover 24 permutations of four independent source children, parallel slurs,
stop/start ties across separate wrappers, a hidden wrapper, an additional empty
wrapper, empty/repeated articulation groups, repeated signs, exact geometry,
source-ID collisions, chord/staff/voice isolation, JSON/native/XML roundtrips,
association-order normalization, malformed topology, and unchanged old profiles.

Focused commands:

```sh
cargo test --offline --locked -p scorestream-core notation_
cargo test --offline --locked -p scorestream-formats --test mixed_notations
(cd desktop && npm test -- tests/mixed-notations.test.ts)
python3 scripts/notation_coverage.py check
```

Deliberate snapshot regeneration only:

```sh
UPDATE_MIXED_NOTATION_FIXTURE=1 cargo test -p scorestream-formats \
  --test mixed_notations actual_mixed_import_snapshot_keeps_notation_without_enabling_preview_or_sound
```

No untouched Beethoven or W3C 32c acceptance, platform packaging, production
playback or complete notation coverage is implied.

### Contract maintenance boundary

`design/tools/notation_order_contract.py` is a targeted, importable installer for
the pinned current canonical descriptor and AST schema. It checks exact required
beam/slur/articulation contracts and the 11 existing constraint ordinals before
adding mixed notation at index 11. Missing/stale/drifted prerequisites reject
without mutation; the installer and JSON schema have independent offline tests.

Do not run the historical `build_format_reference.py` to regenerate this checkpoint.
That full generator predates existing beam/slur/articulation contracts and would
overwrite unrelated canonical data. It is deliberately not wired to the mixed
installer. Repairing the full legacy generator is a separate task; only the
guarded targeted installation is covered here.

## Local verification checkpoint

Tested on Linux with Rust 1.100.0, not the declared minimum toolchain:

- Core/formats/app/codec: **622 passed, 14 existing ignored**.
- MCP package: **25 passed**, including both capability-discovery contracts.
- Actual MCP stdio: **18 schema-validated cases passed**; quick default stays
  below the existing 18,000-byte bound.
- Frontend: **426 passed across 35 files**; TypeScript passed.
- Mixed ledger: **15 exact evidence cases passed**; offline catalog check,
  **20 ledger regressions**, and **4 guarded contract/schema tests** passed.
- The actual mixed Workspace document passes the native AST JSON Schema.
- Scoped rustfmt on all 20 changed Rust files and `git diff --check` passed.
- Clippy on the five affected packages and all targets passed `-D warnings` with
  only the existing `manual_is_multiple_of` baseline exception allowed.

A full-workspace test attempt exhausted shared temporary disk during linking,
**before running tests**; it is not a full-workspace pass. Only this task's
regenerable format build artifacts were cleaned, then the package checks above
were rerun. No legacy full-contract regeneration, new production Vite build,
platform package, remote branch, CI or deployment result is claimed here.
