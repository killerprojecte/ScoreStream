# Source checkpoint: official desktop controls

This branch checkpoints the exact verified desktop source tree from 2026-11-14 on published commit `46c795fb9b69db3212212a58133d7e6635157b71`. It is independent of the SFS metadata and MusicXML branches and contains no new playback/audio/sampler implementation.

The current status is described in [Official control checkpoint](SHADCN_OFFICIAL_CHECKPOINT.md). Older architecture documents describe earlier UI states; the current source and that checkpoint report take precedence. The preserved preview product name and application identifier remain `ScoreStream official controls` and `io.scorestream.shadcn-official`; no untested identity normalization was made.

## Included source

- The complete current React frontend, test suite, local shadcn components, licenses and source provenance, pinned package manifest and lockfile, and build configuration.
- The native window chrome source, required window capabilities, existing native configuration and its lockfile.
- Four intentional removals of the replaced vanilla entrypoint/shell/Inspector/MCP views. No unrelated baseline file was removed. Baseline scripts and previously published documentation images were retained.

## Verification boundary

The source producer recorded 134 passing frontend tests across 17 files, successful TypeScript/Vite production build, and a successful offline locked native Linux build. These checks were run against the preserved preview tree. This checkpoint operation only rechecked source hashes, 20 component provenance hashes, package/lock agreement, text structure and Git scope. It did not rebuild, download dependencies, or launch a native window.

No native display, interaction or pixel acceptance was performed for this stage. macOS/Windows runtime acceptance remains pending. Nine existing whitespace warnings in five source/test files were retained to keep the tested source exact: trailing whitespace in `preferences.css` and `i18n.ts`, plus extra EOF blank lines in `window-controls.css`, `inspector.test.ts` and `mcp.test.ts`. `git diff --check` reports them.

Build output, cache, installed dependencies, generated native schemas, historical launchers and reference screenshots are not added by this commit. Build/test log files and the Apple color screenshot references mentioned in the original report are preserved outside this source repository rather than committed as artifacts.

## Reproduce when the development toolchain is available

```sh
cd desktop
npm ci
npm test -- --maxWorkers=1
npm run build
cd ..
cargo build --locked --manifest-path crates/scorestream-desktop/Cargo.toml
```

These commands describe the supported local build, not a claim that compilation or native QA was repeated during source preservation. This branch does not add or run CI.
