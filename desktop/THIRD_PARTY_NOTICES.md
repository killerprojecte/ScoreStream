# Desktop dependencies and distribution notes

- **Verovio 5.6.0**, npm package `verovio@5.6.0`, unmodified upstream WebAssembly/ESM distribution. LGPL-3.0-or-later. Upstream: https://github.com/rism-digital/verovio/tree/version-5.6.0 . Build/source instructions: https://book.verovio.org/installing-or-building-from-sources/javascript-and-webassembly.html . The LGPL text and referenced GPL text are retained under `licenses/Verovio-COPYING.LESSER.txt` and `licenses/Verovio-COPYING.GPL-3.0.txt`.
- **Bravura**, the font embedded by that Verovio build. SIL Open Font License 1.1. Source and font attribution: https://github.com/steinbergmedia/bravura . License retained under `licenses/Bravura-OFL-1.1.txt`.
- **Monaco Editor 0.55.1**, MIT. Copyright Microsoft Corporation. The npm package contains `LICENSE` and `ThirdPartyNotices.txt`; source: https://github.com/microsoft/monaco-editor .
- **Tauri** Rust 2.9.4 / JavaScript API 2.9.0 / CLI 2.9.4. MIT or Apache-2.0. Source: https://github.com/tauri-apps/tauri .

`package-lock.json` pins the fetched artifacts and integrity hashes. The renderer is bundled locally, never fetched from a mutable CDN. The adapter identifies its version and selected Bravura font in the UI. The ESM module carries the upstream embedded font resources; ScoreStream does not modify the renderer or fonts.

The LGPL worker boundary does not by itself resolve distribution obligations. Before distributing installers, complete the license/source/relinkability review and ship required notices and corresponding source or an appropriate source offer. This development checkout and native smoke run are not a completed third-party distribution review. The design pack does not assign a project source license.
