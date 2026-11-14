# Isolated native smoke builds

Ordinary builds keep the production identity `ScoreStream` / `io.scorestream.desktop`. They do not register the `report_smoke_test` command. The default Vite build defines `__SCORESTREAM_QA_BUILD__` as false, so both smoke entry points and their dynamic test-runner chunks are removed.

Native QA is a deliberately separate build flavor. All of these are required:

1. Vite `--mode smoke`, selected by `build:smoke` or `dev:smoke`.
2. Cargo feature `smoke-test`.
3. Configuration overlay `tauri.smoke.conf.json`, which uses `io.scorestream.desktop.qa` and disables installer bundling.
4. An explicit writable `SCORESTREAM_SMOKE_REPORT` path at runtime.

The native startup guard rejects a smoke report request under any other identifier before loading MCP settings or creating a WebView. Tauri scopes application data/configuration directories to the application identifier; the QA identity therefore keeps test preferences and MCP settings separate from production. Use a disposable OS profile as an additional precaution when running interactive acceptance. Do not point a QA run at a real user's profile or data.

To prepare an isolated QA executable, with the pinned dependencies and native toolchain already available:

```sh
cd desktop
npm run desktop:build -- --features smoke-test --config tauri.smoke.conf.json --no-bundle -- --locked
```

The configuration overlay runs the smoke-mode frontend build automatically. The built executable is a QA application, not the ordinary product. Set `SCORESTREAM_SMOKE_REPORT` to an absolute path before launching it in an isolated test profile. Set `SCORESTREAM_V0_CONTROL_QA=1` only when the read-only control geometry observer is wanted instead of the automated smoke runner. Leave both variables unset for ordinary interactive QA.

The existing `smoke.ts`, native harness, geometry checks, and window checks remain available. QA sources and ordinary unit tests stay in the repository; production frontend assets must not include their entry points. The default CI uses `npm run desktop:build` without the smoke feature or overlay and remains an ordinary production build.

Configuration overlays and distinct application identifiers follow the [Tauri configuration guidance](https://v2.tauri.app/develop/configuration-files/). These build safeguards are not a claim that this newly integrated production-identity application was launched or accepted on every supported operating system.
