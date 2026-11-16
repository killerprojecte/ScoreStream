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

Save still validates SFS and performs an atomic write. The destination must end
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
