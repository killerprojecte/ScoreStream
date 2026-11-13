# Desktop validation checkpoint

## Updated bundled-runtime acceptance

On 2026-11-13, the Linux x86_64 native application **ran successfully** using a self-contained AppDir with official Debian 13 WebKitGTK 2.52.6-1~deb13u1 libraries and matching helper processes. The desktop host has neither `/usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.1.so*` nor the system `WebKitNetworkProcess`; process provenance confirms helpers ran from the AppDir. No system libraries/settings were changed and no sandbox-disable variable was set. This is not a complete sandbox security audit or a clean virtual-machine compatibility matrix.

All **15 native smoke assertions passed** with 20 rendered notes: real WebKit/Verovio output, event selection and source-line mapping, live uncommitted draft preview, draft isolation, disabled dirty-draft mutation, last-good SVG preservation, stale banner, stale-job rejection, Apply, undo, redo, revision conflict rejection, and typed pitch mutation. Actual native editor/score screenshots were captured.

Two issues found during native testing were fixed:

1. The smoke test compared `innerHTML`, which changes legitimately when editing clears selection highlighting. It now verifies that the exact SVG DOM node is retained.
2. `mutate()` read the form value after the busy-state refresh restored the original pitch. It now captures the input before entering the transaction. The native typed-pitch assertion passes after the fix.

The assembled unsigned AppImage (about 223 MiB) was also launched successfully with `--appimage-extract-and-run` on the same host without system WebKitGTK; its normal executable rendered the editor and score, and a separate screenshot was captured. This tests the final AppImage extraction/launch path without requiring FUSE, but does not test FUSE mounting.

The normal executable and smoke-feature executable build; frontend 8/8 tests and TypeScript/production bundling pass after these changes. The normal executable is used for the packaged artifact; the smoke command is not enabled there. Packaging instructions, runtime-source provenance, license/update caveats and release limitations are in [desktop/packaging/README.md](../desktop/packaging/README.md).

Scope: Debian 13 x86_64 desktop, X11, bundled runtime. No claim is made for older glibc baselines, ARM, Wayland, Windows, macOS, file-picker cancel/overwrite, durable-history persistence, audio, or signed public releases. Public distribution still needs a full corresponding-source/license review and clean-target release acceptance.

## Earlier direct-executable attempt (superseded by bundled-runtime success)

Date: 2026-11-13. Host: Linux x86_64, Debian 13. Toolchain: Rust 1.99.1, Node.js 24.22.1, npm 11.9.0.

## Passed

- `cd desktop && npm test -- --maxWorkers=1`: 8 tests passed. Covers exact rational rhythm, stable SVG identity mapping, escaping, refusal of unsupported projection cases, and latest-only render scheduling.
- `cd desktop && npm run build`: TypeScript check and Vite production bundle passed. Vite reports the expected large Monaco/Verovio chunks and externalized Node `module` import from Verovio. This is a bundle result, not proof that WebKit executed the bundle.
- `cargo build --locked --offline --manifest-path crates/scorestream-desktop/Cargo.toml --features smoke-test`: native Linux debug executable built successfully after correcting the runtime version mismatch below.
- `cargo build --locked --offline --manifest-path crates/scorestream-desktop/Cargo.toml`: normal native Linux debug executable built successfully.
- `cargo fmt --manifest-path crates/scorestream-desktop/Cargo.toml --check`: passed.

The Linux build used locally extracted Debian GTK/WebKit development libraries through `PKG_CONFIG_SYSROOT_DIR`, `PKG_CONFIG_PATH`, `LIBRARY_PATH` and `LD_LIBRARY_PATH`. This verifies linking, not installation on a clean end-user system.

## Dependency correction

The initial lockfile paired `tauri-runtime-wry 2.9.3` with `tauri-runtime 2.11.3`. Compilation failed on an unimplemented `eval_script_with_callback` trait method and an incompatible `Sync` bound. Pinning `tauri-runtime = "=2.9.2"` in the desktop manifest, and updating its independent lockfile, restores the compatible API. No dependency source was patched.

## Native runtime attempt: blocked before UI

The smoke-feature executable was launched on the existing Linux desktop (`DISPLAY=:0`), not in a browser mock. WebKitGTK aborted before the score editor became visible:

```text
Unable to spawn a new child process: Failed to spawn child process
“/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/WebKitNetworkProcess”
(No such file or directory)
```

The helper binaries exist in the build's extracted sysroot, but this WebKit library resolves its child processes from the system installation path. No system WebKitGTK runtime is installed there. The environment does not offer `sudo`. An attempted virtual display was also unavailable because the execution context rejected Unix socket creation; the existing desktop was used instead.

Consequently:

- No native smoke JSON assertions passed or failed: the WebView never reached them.
- No genuine ScoreStream window screenshot was captured. A browser mock or unrelated desktop image must not be presented as one.
- Linux native runtime acceptance remains pending on a system with properly installed WebKitGTK runtime dependencies.
- Native file-picker cancel/overwrite flows, close-with-dirty-draft behavior, packaging/installers, and manual visual QA remain unverified.
- Windows and macOS native builds and runtime tests have not run. The platform-neutral application structure and Tauri configuration are implementation intent, not proof of compatibility.

## Reproduce runtime acceptance on a provisioned Linux desktop

Install the dependencies listed in `desktop/README.md` using the operating system's supported package manager. Then:

```sh
cd desktop
npm ci
npm test -- --maxWorkers=1
npm run build
cd ..
cargo build --locked --manifest-path crates/scorestream-desktop/Cargo.toml --features smoke-test
SCORESTREAM_SMOKE_REPORT="$PWD/native-smoke.json" \
  crates/scorestream-desktop/target/debug/scorestream-desktop
```

Require `native-smoke.json` to contain `"passed": true`; retain its individual checks and a screenshot of the real native window. Repeat packaging and native runtime acceptance on Windows and macOS before claiming those platforms are tested.
