# Default desktop MusicXML import

File > Open now offers SFS, MusicXML/XML and MXL. XML conversion uses the
`Automatic` articulation profile without a mode picker. The existing SFS draft
route remains available. This uses the existing bounded converter, not a claim
of complete MusicXML support.

Conversion happens off the UI thread. A successful conversion is held separately
until the frontend requests activation with the current revision. Failed or
cancelled conversion leaves the active score unchanged. Activation starts a new
workspace with empty history and an explicitly unsaved SFS origin, even if its
text matches a previous document. The old preview is cleared rather than shown
as the imported score. Import diagnostics remain visible across edits and
separately state retained data and rendering/playback limits.

Save still validates SFS and performs an atomic write. Its validated source
object is the same path used by native saves and file-level tests. SFS Open no
longer imposes its former separate 4 MiB file cap: a document accepted by shared
validation and saved by the editor can be read back as an SFS draft. This does
not remove the parser's structural validation and is not a claim of
unbounded-memory or large-score rendering performance. The destination must end
in `.sfs`; the original XML/MXL path and its existing hard-link/symlink aliases
are rejected. Cancelling save leaves the import unsaved. Opening an SFS draft
and discarding it restores the prior document's origin and import report.

The reader's existing strict geometry, timeline and unsupported-symbol policies
are unchanged. Rich notation can be preserved while the current renderer or
playback planner refuses realization; importing is not playback acceptance.

## Verification

The source-file helper is independently testable: XML/MXL equivalence, rich
retention, malformed inputs, unchanged SFS draft semantics, single-use
revision-checked activation, and destination alias refusal are covered.
Frontend controller tests cover imported unsaved state, equal-text imports,
failed activation, disposal before activation, stale-preview clearing, retained
reports and restoring provenance after discarding a subsequent SFS draft.

These checks do not constitute native file-dialog, full Tauri packaging,
Windows/macOS runtime, actual audio or complete Beethoven-score acceptance.

## Large-score regression

A generated valid MusicXML fixture with 4,800 measures and distinct long note
identifiers produces 4,765,239 bytes of valid SFS. The previous desktop Open path
rejected that exact saved source solely because it exceeded 4 MiB. The regression
now exercises XML conversion, shared source validation, the actual native atomic
save helper, SFS reopen and shared workspace validation. It checks exact source
bytes, unchanged XML, all 4,800 note events, and successful playback planning
for the reopened source. This is a synthetic format-scale regression, not full
Beethoven, native file-dialog, or audible-performance acceptance. Nine file-helper tests pass, including invalid drafts,
transactional activation and protected destinations.
