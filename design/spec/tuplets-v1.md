# Authored tuplet endpoints v1

`!requires scorestream.tuplets@1` retains written MusicXML tuplet **endpoints**,
not inferred spans, ratios, nesting or performance. The narrow input shape is a
measured note/rest with one unadorned `<notations>` wrapper. Each `<tuplet>` is
empty, has `type=start|stop`, and may have an optional `bracket=yes|no` on start.
The supported stop shape has no bracket. Absence differs from explicit `no`.
Other attributes, nested tuplet notation payloads and additional wrappers reject
instead of being flattened.

`ScoreDocument.tuplets` holds one typed ordered child sequence per event. Children
are either an authored endpoint or a typed reference to an existing articulation
group, ordinary tied phase, authored slur endpoint or fermata. Those existing
families remain authoritative for their own semantics. References must cover all
siblings exactly once and agree with any existing mixed-notation topology.
Canonical simple articulation groups keep the existing playable simple model;
noncanonical groups require authored retention. This bridge does not promote a
canonical staccato to an unsupported performance family merely because a tuplet
endpoint is adjacent to it.

Native `scorestream.tuplets.child` marks target `#event` at the exact measured
onset. `index` is contiguous from zero within that event's one wrapper. `kind`
selects a closed scalar shape:

- `tuplet`: required `phase=start|stop`; optional boolean `bracket` on start only.
- `articulations`: required `group_index`, referencing that wrapper's exact group.
- `tied`: required `phase=start|stop`, referencing existing ordinary tie semantics.
- `slur`: required `phase` and `relation` identity.
- `fermata`: no extra fields.

Unknown fields/kinds/versions, missing capabilities, fragments, duplicate/gapped
indices, wrong hosts/onsets and incomplete/conflicting sibling references reject.
No arbitrary endpoint count or span-length ceiling is added.

Open, repeated or reversed printed endpoint sequences are retained as authored.
They are **not** certified as well-formed closed tuplets: no partner or nested
hierarchy is fabricated. Existing `time-modification`, written note type and
actual event duration are independent and unchanged. Qualified ordinary playback
ignores this visual endpoint metadata. Existing unrelated unsupported authored
families retain their own performance restrictions.

Endpoint metadata alone adds no preview rejection, and an otherwise supported
preview explicitly discloses that authored endpoints/brackets are not reproduced.
The existing renderer still rejects time-modification and nonordinary tuplet
rhythms; this bridge does not qualify their engraving. Playback has a separate
exact-rational capability and the verified ordinary playback cases stay usable.

## Verification and original-corpus bounds

- `cargo test -p scorestream-formats --test tuplets` verifies exact native/XML
  roundtrip, bracket absence/false/true, open/repeated endpoints, mixed sibling
  order, native corruption, and unchanged rhythm/audio including canonical simple
  articulation hosts.
- `desktop/tests/tuplets.test.ts` verifies unchanged preview MEI plus disclosure.
- With `SCORESTREAM_MXL_FIXTURES`, run ignored
  `original_tuplet_endpoint_projection_and_unsupported_siblings`. It verifies
  source SHA-256 and literal XML/MXL equality, then projects the endpoint family
  through typed native/XML roundtrip: 2,142 endpoints (II 420; IV 1,722), with
  bracket yes/no/absent counts 10/1,061/1,071.

That receipt is explicitly an **endpoint-family projection**, not a full score
or full note import. The 103 original hosts that also contain unsupported
ornaments remain rejected by the complete bridge; the test verifies that they
cannot lose those siblings silently. Other family validators can still reject
other context. No untouched original is claimed fully imported by this slice.
