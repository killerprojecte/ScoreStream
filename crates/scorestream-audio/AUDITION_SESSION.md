# Source-bound Linux audition session

`audition_session` is compiled only on Linux. Both `audition` and
`dry_admission_probe` now import this one library implementation; there is no
example-local compiler or copied rendering engine. The closed JSON configuration,
source SHA, policy selection, preset/mixer binding and existing strict defaults are
unchanged. Desktop default Play and its WebAudio WAV export are not altered.

## Host-owned lifecycle

1. Capture the editor's exact source bytes. The host explicitly obtains policy and
   existing SoundFont selections through its own trusted UI, not a renderer-supplied
   arbitrary filesystem IPC path. Deserialize `Config` and call the existing
   `Audition::compile_practice` with the explicitly selected interpretation policies.
2. The session owns a copy of the source and its opaque compiled plan/palette.
   `verify_snapshot(current_source)` rejects even an added newline. It does not
   observe the editor automatically: the host must supply its actual current
   snapshot and reject stale operation/revision completions.
3. `prepare_dry_snapshot(current_source, bank, cancel)` uses the existing native
   dry engine through authoritative voice exhaustion and returns `DryReadiness`.
   Its bank path and readiness observation are private. It has no Deserialize or
   public constructor. `report()` is immutable display data; keep the real object
   in native state rather than reconstructing authority from WebView JSON.
4. `render_prepared_snapshot(current_source, &readiness, output, cancel)` repeats
   source checks before output I/O and uses the same verified atomic renderer.
   Cancellation and changed source leave no output; successful replay must match
   all native readiness observations before no-clobber publication. A different
   session/policy cannot substitute an incompatible readiness report.

The host supplies a cancellation flag for each operation. Native probe/replay
checks it throughout; there is no new engine or background thread here. Compilation
itself remains synchronous and is not claimed to be preemptible. For a desktop
blocking worker, retain the session/readiness natively and map errors to a
path/source-safe frontend representation (e.g. convert errors inside the worker
rather than sending a non-Send boxed error across a runtime boundary). Session and
readiness objects themselves are checked Send + Sync by tests.

Existing CLI convenience methods remain thin wrappers for compatibility. Their
JSON readiness-file workflow still freshly revalidates the full native observation;
it is not the recommended desktop ownership mechanism.

## Remaining desktop integration, deliberately not placeholders

The current desktop `wav_export_begin` returns a bare serializable strict plan for
WebAudio PCM. That is not a `TechniquePlaybackPlan` authorization and cannot be
relabelled as one. A real native UI integration still needs an explicit Linux-only
choice, policy/bank selection and disclosure, window-owned state/cancel token,
current revision guards, native save chooser and completion/progress handling.
It must not accept frontend PCM as evidence that the native dry plan was rendered.
No new Tauri command, menu, generic file player, CSP relaxation, external score,
SoundFont distribution or cross-platform audio capability is introduced here.

## Verification scope

Focused library/contract tests plus the existing audition tests cover unchanged
compiler policy behavior and source capture/cancellation before I/O. One small
six-note fixture uses the installed exact TimGM bank, two source routes, real
pizzicato/grace/hold policy and complete native readiness. It verifies changed
source rejection, pre-cancelled export, successful nonzero PCM and exact WAV size,
retained readiness equality, and existing-file preservation. No full movement is
rendered or downloaded for this change. Linux examples compile; the dry CLI now
has a non-Linux unavailable stub, but Windows/macOS builds or playback are not
claimed tested in this Linux-only toolchain.
