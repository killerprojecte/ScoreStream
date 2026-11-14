# Desktop CI installers

`.github/workflows/desktop-bundles.yml` builds the real Tauri desktop application,
including its frontend, on separate native GitHub-hosted runners. It runs for
relevant changes pushed to `master`, pull requests targeting `master`, and manual
`workflow_dispatch` runs. No GitHub Release, tag, deployment, updater, signing
certificate, or notarization service is configured. A green build is not a
clean-host installation/runtime acceptance report.

## Build matrix and outputs

| Platform | Runner | Rust target | Required uploaded installers |
| --- | --- | --- | --- |
| Linux x64 | `ubuntu-22.04` | `x86_64-unknown-linux-gnu` | `.deb`, `.AppImage` |
| Linux ARM64 | `ubuntu-22.04-arm` | `aarch64-unknown-linux-gnu` | `.deb`, `.AppImage` |
| macOS Intel | `macos-15-intel` | `x86_64-apple-darwin` | `.dmg` containing a `.app` |
| macOS Apple Silicon | `macos-15` | `aarch64-apple-darwin` | `.dmg` containing a `.app` |
| Windows x64 | `windows-2022` | `x86_64-pc-windows-msvc` | NSIS `-setup.exe` |
| Windows ARM64 | `windows-11-arm` | `aarch64-pc-windows-msvc` | NSIS `-setup.exe` |

The [GitHub runner list](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
provides these host architectures. Each macOS job builds one architecture; neither
is labeled universal. Linux uses Ubuntu 22.04 to avoid silently raising the glibc
baseline; compatibility with other distributions still needs runtime testing.
ARM AppImages are built on ARM, not cross-compiled, as required by
[Tauri's AppImage tooling](https://v2.tauri.app/distribute/appimage/).
[Windows ARM64](https://v2.tauri.app/distribute/windows-installer/#building-for-32-bit-or-arm)
has a native ARM64 app inside an x86 NSIS installer that runs under emulation.
The Windows runner supplies the MSVC tools. The default installer may download
WebView2 if the destination machine does not already have it.

The frontend and the Rust desktop workspace remain separate. The command uses
`desktop/package.json`'s `desktop:build` script, which changes into
`crates/scorestream-desktop` before invoking the locked Tauri CLI. Its
`beforeBuildCommand` runs `npm run build` in `desktop` to typecheck and bundle the UI.
No raw executable or `desktop/dist` directory is uploaded instead of an installer.

Native output paths, relative to the repository root:

```text
crates/scorestream-desktop/target/<rust-target>/release/bundle/deb/*.deb
crates/scorestream-desktop/target/<rust-target>/release/bundle/appimage/*.AppImage
crates/scorestream-desktop/target/<rust-target>/release/bundle/dmg/*.dmg
crates/scorestream-desktop/target/<rust-target>/release/bundle/macos/ScoreStream.app
crates/scorestream-desktop/target/<rust-target>/release/bundle/nsis/*-setup.exe
```

The collector requires exactly one nonempty installer for **each** format in that
target's row and, on macOS, the underlying `.app` executable. Thus a missing
AppImage cannot be hidden by a successful `.deb` build. It copies the installers
into a fresh runner-temporary directory and adds `SHA256SUMS`, `build-info.json`,
frontend license notices, and usage notes. The manifest records the checked-out
commit (the merge commit for a normal PR run), target, actual toolchain/runner
versions, lockfile hashes, installer sizes, and SHA-256 hashes.

## Downloading artifacts

Open the repository's **Actions → Desktop bundles → run → Artifacts** and choose
`scorestream-<platform>-<arch>-<commit>-<attempt>`. Each job has a distinct name,
including rerun attempts. GitHub login/access is required to download an artifact.
Artifacts expire after **7 days**, not the account's potentially longer default.
Missing artifact files fail the upload rather than merely warning.

The [official upload-artifact action](https://github.com/actions/upload-artifact)
archives the selected files. GitHub's artifact ZIP does not retain executable
permissions, so after extracting a Linux artifact, use `chmod +x *.AppImage`.
macOS `.app` permissions and symlinks are preserved inside its DMG. Checksums
detect corrupted downloads; they are not publisher signatures or attestations.

These are unsigned testing builds. OS trust warnings and installation limitations
are expected. Do not disable Gatekeeper, SmartScreen, WebKit's sandbox, or other
OS security controls to bypass a failure. Native UI, file dialogs, rendering,
audio if later added, and clean-host install/uninstall acceptance remain separate
tasks. The existing [packaging security and license notes](README.md#security-licenses-and-updates)
still apply, including bundled-library source/license obligations and rebuilds
for WebKit security updates. Included frontend notices are not a full native SBOM.

## Reproducibility and trust boundaries

- Node.js `22.23.4`, Rust `1.100.0`, the repository's Tauri CLI version, and the
  GitHub actions' full commit SHAs are pinned. Update them deliberately alongside
  validation. `npm ci` uses the committed frontend lockfile; `-- --locked` passes
  Cargo's lockfile requirement through Tauri. Both Cargo lockfiles are committed
  and checked for unintended changes after the build.
- The only dependency cache is setup-node's npm download cache, keyed by OS,
  architecture and `desktop/package-lock.json`. `node_modules`, Rust build outputs
  and installer directories are not cached or restored. Every job starts on a
  fresh hosted runner, with no shared artifact destination between architectures.
- Runner images and OS package repositories continue to receive updates. This is
  a locked application-dependency build, not a claim of bit-identical builds or
  frozen system-library versions. The output manifest records the runner image.
- Permissions are `contents: read`; checkout does not retain Git credentials.
  PR jobs use `pull_request`, never `pull_request_target`/privileged follow-up
  workflows. No signing secrets or write token are passed to project commands.
  Uploading a workflow artifact does not need repository-content write access.
- Frontend tests, headless Rust tests, and collector tests must pass before
  packaging and upload. There is a 60-minute per-job timeout, independent matrix
  failures, and cancellation of superseded runs for the same event/ref.

## Reproduce locally

Install the normal OS prerequisites from [desktop/README.md](../README.md).
On Ubuntu 22.04 the CI job additionally installs `libssl-dev`, `patchelf`,
`libfuse2` and `file` for bundling. Use the corresponding native OS and CPU from
the matrix, including the ARM64 MSVC tools on Windows ARM64. No Linux-to-Windows
or Intel-to-ARM AppImage cross-compilation is implied.

For example, on Linux x64, from the repository root:

```sh
rustup toolchain install 1.100.0 --profile minimal --no-self-update
rustup target add --toolchain 1.100.0 x86_64-unknown-linux-gnu
export RUSTUP_TOOLCHAIN=1.100.0
node --test desktop/packaging/test-collect-bundles.mjs
cargo test --workspace --locked
cd desktop
npm ci --no-audit --no-fund
npm test -- --maxWorkers=1
npm run desktop:build -- --ci --no-sign --target x86_64-unknown-linux-gnu --bundles deb,appimage -- --locked
cd ..
node desktop/packaging/collect-bundles.mjs x86_64-unknown-linux-gnu /absolute/new/output-directory
```

Use Node.js `22.23.4` to match CI. For macOS substitute the appropriate Apple
target and `--bundles app,dmg`; for Windows use the MSVC target and
`--bundles nsis`. In PowerShell, set `$env:RUSTUP_TOOLCHAIN = '1.100.0'` instead
of `export`. Each collection needs a new output directory. Do not mix old bundle
files with new ones. On Linux builders without FUSE, set
`APPIMAGE_EXTRACT_AND_RUN=1` for the build to allow the packaging tool to extract
itself; this does not disable any app sandbox.

Validate YAML with a GitHub Actions-aware checker before changing the workflow.
The collector's Node tests use fixture bytes, not real installers: they test
selection/failure/checksum behavior only. Native packaging and upload require
the real platform runners; no workflow has been dispatched merely by adding
these files to a local checkout.
