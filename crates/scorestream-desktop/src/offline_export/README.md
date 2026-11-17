# Explicit Linux offline rehearsal export (native bundle validation pending)

The Linux host now registers `offline_export_reserve`, `offline_export_run` and
`offline_export_cancel`. The File menu action is shown only after the native
`offline_export_supported` command reports Linux support. The ordinary playback
transport, existing Export WAV and non-Linux command sets remain unchanged.

This source checkpoint has a thin real UI and native command implementation, **not
an accepted Tauri bundle**. Local GTK/WebKit development dependencies and the rfd
registry cache are missing. Native compilation and dialog/window integration must
be checked on the existing Linux GitHub runner before release.

## Owner and file authority

Reserve checks Tauri's injected `WebviewWindow` is `main`, then captures native
workspace identity. IPC supplies only the expected revision and a generated
operation reference, never owner strings, policy paths, bank paths or output paths.
Run requires an explicit `disclosed_rehearsal_v1` mode. The UI explains it before
calling run. The existing WAV GTK chooser implementation is shared, not copied;
it selects a local policy JSON, local SF2, then a new WAV destination, parented to
that native window. Cancellation or stale results cannot authorize later dialogs.

The chosen Config must match the captured source SHA and bank SHA/palette. Only
the existing trusted FluidSynth/bank checks authorize native rendering. Nothing is
downloaded or bundled, and no real user score or bank is selected automatically.
The UI displays pending/cancelling/success/failure, and unsubscribes on teardown.
A successful native commit is not mislabeled cancelled if cancel loses the race.

## Explicit playback scope

Run calls existing `Audition::compile_practice` with the same closed policies used
by the verified source-bound rehearsal path: written notes without wedge, dashes,
ornaments or slur sound; tied continuation NoteOn inert; unmarked release before
selected holds; authored NoteOn plus retained relative trajectory; initial GM CC7
centered local mixing. The local policy supplies exact holds/graces/palette and
resource choices. Per-note dry gain curves remain intact, shared pizzicato samples
remain disclosed, and native event quantization remains 64 frames. This is not an
assertion of fully faithful playback and is not a new renderer.

A strict joint Config alone is **not** enough for this dry path: the actual strict
fixture fails `Dry admission requires source mixer`. This negative is retained as
a regression; the UI does not advertise a false strict dry export. Ordinary strict
library behavior remains unchanged.

## Cancellation and publication ordering

`state.rs` and `lease.rs` contain the exact dependency-free host lifecycle. One
window has one active generation; stale chooser/finally/cancel cannot affect a new
generation. The run flag also rejects duplicate runs of the same reservation.
Every desktop source mutation, including same-byte apply, preview draft, undo,
redo, demo reset and imported workspace activation invalidates while holding the
workspace mutex. Navigation, close and exit also revoke the lease.

Native reserve/preflight/publication compare a borrowed `Workspace::source_revision`
(source, revision, dirty flag and draft sequence), without building a full score
view. Draft sequence also catches same-byte operations through the shared MCP
workspace. MCP edits may not stop a worker immediately, but the native authority
is rechecked before publication; dirty or changed source is rejected. No front-end
revision assertion alone authorizes publishing.

Lock order is workspace then export lease. Compilation, native preflight, PCM
rendering, validation, flush and fsync run outside those locks. At final commit,
the same atomic WAV writer passes an opaque `PendingWav` into the host gate;
`Leases::publish` invokes only its no-clobber commit under the short lock. Existing
files and concurrent destinations are never overwritten. Both success and failure
retire the lease. Failed/stale output drops the private temporary file.

Cancellation before the gate wins and prevents output. Publication first wins
and is not retroactively undone. The actual-file gate tests cover cancel, same
text/new revision and reload after the final atomic-bool check, then confirm no
output/temp residue; commit-first preserves bytes and a concurrently created
other-owner destination remains unchanged.

## Verification boundary

- Dependency-free harness imports exact host/lease modules, not copies.
- App test checks borrowed source identity, dirty drafts, same-byte apply and
  returning to original text at a newer revision.
- UI controller tests cover explicit initiation, late reserve cancellation,
  commit-winning cancellation, duplicate start, chooser cancel and native errors.
- Audio test uses actual local trusted TimGM/FluidSynth and the source-bound
  rehearsal session/gate, with nonzero WAV, exact readiness and frame size.
- TypeScript typechecking and Rust parser checks do not substitute for Tauri
  typechecking or real GTK interaction.

The nested desktop lock adds only the local audio package and its edge; all
registry versions/checksums already exist. `cargo metadata --no-deps --offline
--locked` succeeds, but full offline resolution cannot find cached rfd. The
existing desktop-bundles Linux runner installs GTK3, WebKit2GTK4.1, SSL, appindicator,
librsvg, pkg-config and build tools before locked native build. Use it for the next
bundle acceptance; do not install dependencies or report a local bundle pass here.


## Source-bound policy draft and validation

The same dialog now offers **Generate policy draft** and **Validate edited policy**.
Both reserve the same native window/source/revision lease; native choosers alone
select their paths. Draft JSON is written outside locks to a private temporary file,
then no-clobber published under the same source/owner gate. It never replaces a
user's policy, changes the score, chooses a bank or starts audio.

The draft contains exact source SHA, part/instrument identifiers and authored MIDI
facts. Program/bank/preset, hold/grace choices, gain and rendering options remain
null. These are explicitly unchosen, not suggested defaults. Instructions show the
existing hold/grace shapes and accepted enums; no new policy interpreter exists.
The metadata/instructions are explanatory only and cannot authorize compiler routes.

Validation accepts existing Config JSON or this closed draft envelope. It checks
both source hashes, lists null paths, then uses the existing source mixer and
practice discovery once routes are explicitly filled. Grace identities and their
principal coordinates are included alongside fermatas; final-ending continuation
is used only when explicitly selected. The real practice compiler remains the
final check for complete policy fields, group membership and timing. Validation
never opens a bank or creates a WAV, and `policy_compiled` does not mean native
bank verification. UI shows the actual report for inspection/copying.

A filled draft is accepted directly by the existing export path and both CLI
examples, so users do not have to extract a hidden subobject. Empty drafts cannot
compile or export. Actual local six-note fixture testing verifies an explicitly
filled draft through the same native preflight/atomic WAV path, not a mock engine.
The newly added native commands still require subsequent Tauri CI acceptance;
this increment does not trigger or claim another bundle build.

## Backend availability is checked before export file selection

Linux platform support does not imply FluidSynth is installed. The export command
now runs `backend_probe::installed` on its blocking worker before the policy, bank
or destination chooser. It uses the unchanged trusted fixed-system-path loader,
checks the actual FluidSynth 2 ABI, loads the exact dry-group symbol table and
queries the physical output-group range. It opens no bank, synth/audio device or
MIDI driver, and does not render. The status is not a reusable source/bank authority;
normal preflight and export checks still run independently.

Failure yields `OFFLINE_BACKEND_UNAVAILABLE` with an actionable UI explanation;
typed unavailable errors encountered later receive the same safe classification.
The platform-supported menu remains visible, so draft generation and validation
continue to work without the native library. Nothing is auto-installed, bundled,
downloaded or added to loader search paths. Existing loader override rejection is
covered in an isolated child-process test; the actual installed ABI probe is a
separate explicitly run native test, not a mocked missing-library claim.
