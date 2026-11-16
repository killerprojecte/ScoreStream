# Beethoven Fifth acceptance

The corpus is the four untouched MusicXML root documents in the Hauptstimme
MXL archives linked in [the navigation notes](REPEAT_TRAVERSAL_PROTOTYPE.md).
Place each archive and its extracted root document in the same directory:
`Beethoven_Op.67_1.mxl`, `Beethoven_Op.67_1.musicxml`, and so on.

Run the same exact-source check against the build being evaluated:

```sh
cargo build --locked -p scorestream-cli
python3 scripts/check_beethoven_import.py target/debug/scorestream /path/to/originals
```

The check pins the four XML payload hashes and requires XML/MXL parity.
It invokes the real CLI, checks nonempty identical SFS on success, and reports
adapter diagnostics on failure. When the adapter truncates diagnostics, counts
are for the messages emitted, not counts across the corpus. The command fails
until all four movements import. It never rewrites an input or treats a
regenerated layout as a valid substitute.

Import success does not establish a complete performance. Separately verify
the navigation route and exact timing; interpreted fermatas including II m123,
III m257 and IV m446; sustained ties and rest-only holds; the transition from
the third movement into the fourth; and end-to-end playback and WAV export.
The timing fixtures are not substitutes for playback of the four originals.
