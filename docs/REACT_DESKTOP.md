# React desktop architecture

The desktop frontend is now a React application. `main.tsx` creates one root; `App.tsx` composes the window header, panes, status strip and controlled dialogs. There is no string-template shell or Inspector/MCP DOM view class.

- `app/workbench.ts` is an observable, UI-independent session controller. Rust is still the only parser and committed-score authority. Draft validation and Verovio rendering remain sequence-checked, so late work cannot publish over a newer edit. API adapters can be injected in unit tests.
- `SourcePane.tsx` owns a narrow Monaco mount and disposes the editor/model on unmount. `ScorePane.tsx` owns a narrow sanitized SVG mount from the existing Verovio worker. Everything around those two third-party rendering surfaces is React-managed.
- `Inspector.tsx` owns field drafts, pending/error state and focus restoration. One accepted property does not overwrite another staged value. Event identity changes reset the local fields.
- `PreferencesDialog.tsx` and `McpSettings.tsx` use controlled shadcn/Radix Dialog, Tabs, Select, Switch and labelled form controls. The existing MCP controller retains stale-response and repeated-action guards. The app owns polling and cleanup. Access keys remain session-only.
- `WindowControls.tsx` and `lib/window.ts` separate operating-system chrome/operation handling from the shell. The Tauri builder configures AppKit overlay title bars on macOS and app-owned controls on Windows/Linux. Platform behavior must be verified on that actual platform before claiming support has been tested.
- Source/score/Inspector headers share a 44 px row and 16 px content inset. Inspector property fields reserve matching text/unit/action columns. Sidebar widths remain purpose-specific rather than artificially equal. Below 850 px, all three panes remain accessible through explicit compact view buttons.

## Component provenance

`desktop/shadcn-provenance.json` records official shadcn new-york registry URLs and hashes of fetched component definitions. Source components are checked in under `src/components/ui`. Changes to the registry source are limited to local import resolution and an optional `DialogContent.showCloseButton` for localized dialog chrome. Tailwind 3, compatible with these components, is configured through PostCSS. Direct npm dependencies are version-pinned and transitive versions are locked.

## Existing backend boundaries

This migration does not change the published native File Open/Save commands. Open imports a file as a private draft that the user reviews and applies. Save uses the existing native file picker. The menu deliberately does not claim a differentiated Save As/current-file-overwrite contract. The separately proposed native file-workspace changes are not included.

The playback planner exists in Rust but has no integrated audio adapter in this frontend. Preferences does not invent working audio-device controls.

## Validation

Run `cd desktop && npm test -- --maxWorkers=1` and `npm run build`. Native validation uses the [isolated smoke build flavor](NATIVE_SMOKE.md), including the `smoke-test` feature, explicit frontend smoke mode, separate QA identifier, and writable `SCORESTREAM_SMOKE_REPORT` path. The React smoke harness checks the real native backend/rendering flow and geometric assertions after React commits. Browser fixtures, if used, validate layout only and are not native operation evidence.
