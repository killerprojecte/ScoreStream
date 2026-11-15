# VST3 metadata catalogue: isolated precursor

This small Rust library discovers **unverified metadata candidates**. It never
loads a plug-in, calls a factory, instantiates a processor, opens an editor,
produces audio or starts a helper process. The crate forbids unsafe code.
Nothing is wired into desktop playback, the main CLI, the root workspace or CI.
No user/system plug-in directory was scanned during development: tests create
synthetic temporary bundles and non-executable header fixtures only.

## What is implemented

- `suggested_roots` is a pure function returning SDK-priority candidate paths for
  Windows, macOS and Linux. The caller supplies target-machine context, including
  Windows Known Folder values. It reads no environment variables, registry,
  accounts or filesystem, and does not invent a Windows installation drive.
- `scan_roots` reads only explicitly supplied absolute roots, in caller order.
  It recursively enumerates ordinary directories, finds `.vst3` bundles and
  legacy single-file candidates, and produces deterministic structured results.
- Optional `Contents/Resources/moduleinfo.json` is preferred. The older
  `Contents/moduleinfo.json` location is accepted only when the modern file is
  absent. An invalid modern file does not silently fall back. Missing metadata
  remains explicit, with no fallback that executes code.
- Full JSON5 parsing uses pinned `json5` rather than a homemade partial grammar.
  A non-parsing lexical guard bounds nesting before recursive parsing. Known
  duplicate fields, duplicate class IDs, malformed IDs, invalid numeric flags,
  and compatibility targets not declared by the module are rejected.
- Names, vendor/version claims, categories, class flags, cardinality and
  compatibility claims are retained. Manifest URLs and snapshot paths are never
  followed. Compatibility declarations are not applied automatically.
- Bounded PE, ELF and thin/universal Mach-O header reads report actual CPU labels
  separately from directory hints. Platform/known-architecture disagreement is
  diagnosed. This does not check exported factory functions, dependencies,
  signatures, CPU subtypes, operating-system version, ABI or runtime behavior.
  Universal headers advertise slices; inner slice code is not validated.
- Duplicate processor IDs remain visible. The first metadata candidate follows
  caller root order, then deterministic path order. That precedence is a report,
  not automatic activation, approval or a claim that a plug-in is available.

## Identity and trust

`ClassId` preserves the canonical SDK manifest CID as 32 uppercase hex characters.
`vst3:<CID>` is a logical vendor identity claim suitable for future references.
It is not derived from a display name or path, and is not an authentication token.
Do not reinterpret it as host-endian GUID bytes; a future SDK bridge must use the
SDK's canonical conversion and compare the live factory's class IDs.

`manifest_sha256` identifies the exact inspected manifest bytes. It is **not** a
plug-in-code hash. Each binary's `header_prefix_sha256` covers only the inspected
prefix. A future host must separately hash/verify the complete chosen module and
dependencies, reconcile factory metadata, record an installation/revision binding
and reject unexpected changes. A forged manifest can claim another vendor's CID.

Every candidate has `host_verified = false`; every header has
`loadability_verified = false`. A claimed `Instrument` subcategory does not prove
MIDI/event input, supported channels, parameters, patches or offline rendering.
ARM64EC/ARM64X are distinct metadata labels; this code makes no automatic
cross-architecture loading or emulation decision.

Symlinks and Windows reparse points below a root are skipped with diagnostics,
even though the VST3 ecosystem permits linked installations. Root links themselves
are refused; ordinary ancestor aliases are canonicalized when resolving the
explicit root. Following links or aliases automatically needs a separate reviewed
policy. The reader checks regular files and stable size/mtime before/after reads.
This assumes stable authorized local trees; it is **not** a security sandbox
against hostile concurrent filesystem mutation, mount changes or network files.
No manifest-provided path is used as an executable/read target.

## Bounds and completeness

Defaults: 32 roots, 20,000 enumerated entries, 1,000 candidates, eight recursive
directory levels, 1 MiB per manifest, 64 KiB per header, 32 MiB total retained file
bytes. JSON5 nesting is capped at 32, classes/compatibility entries at 2,048 and
strings at 4,096 bytes. Explicit limits are validated against hard upper bounds.
Metadata parsing can transiently allocate more memory than the source bytes.
`complete` describes bounded traversal, not complete class discovery or runtime
support; individual skipped/invalid/missing resources remain in diagnostics.
Directory/budget failures set `complete = false`. Missing optional manifests can
still occur in an otherwise complete directory traversal.

File reads are prefix/size bounded. `retained_file_bytes` counts successful
bounded reads, not kernel I/O or an adversarial concurrent writer's byte traffic.
No results are persisted and no files are modified by the library/example.

## Build and test

```sh
cargo test --manifest-path crates/scorestream-vst3-catalog/Cargo.toml --locked --offline
cargo clippy --manifest-path crates/scorestream-vst3-catalog/Cargo.toml --locked --offline --all-targets -- -D warnings
cargo fmt --manifest-path crates/scorestream-vst3-catalog/Cargo.toml --check
```

The example emits JSON only for roots the caller explicitly provides:

```sh
cargo run --manifest-path crates/scorestream-vst3-catalog/Cargo.toml --locked --offline --example catalog -- /explicit/approved/root
```

Tests use synthetic Windows/macOS/Linux bundle structures on the current Linux
test host, not actual plug-ins. Native Windows/macOS filesystem behavior has not
been validated. There is no automatic installation-root scan during tests or
library initialization. No external plug-in or VST SDK installation is required.

Dependency choice: `json5 = 0.4.1` (ISC), with the four Pest packages locked to
2.8.3 (declared MSRV 1.81). This avoids silently pulling newer Pest/native-stack
dependencies or newer json5 syntax beyond the workspace's Rust 1.85 floor. Use
the committed lockfile. json5 0.4.1 has no published MSRV declaration; its edition
and source are compatible with this floor, but **the actual checks here used
Rust 1.100**, not an independently installed 1.85 toolchain. Release integration
must include the project's minimum-toolchain CI check. The old json5
unmaintained advisory was [withdrawn by RustSec](https://rustsec.org/advisories/RUSTSEC-2025-0120.html).
This is not a full dependency vulnerability audit. No third-party source is
vendored; preserve dependency notices when distributing compiled artifacts.

## Next native-host gates

1. Decide root-workspace/optional-feature integration and CI coverage. Keep
   discovery distinct from availability, selection and explicit load permission.
2. Build a native same-OS isolated scanner for missing manifests and factory
   verification, with timeouts, crash quarantine and validated IPC. Only approved
   modules may be loaded; do not execute `moduleinfotool` as a metadata shortcut.
3. Implement SDK component/controller/processor lifecycle, bus negotiation,
   state/preset loading, parameter and event mapping, sample-offset scheduling,
   latency/tails, and a separate offline instance. Metadata presence alone proves
   none of these capabilities. Bind score instrument routes to verified class IDs
   plus module revision and explicit patch/articulation state.
4. Test an authorized installed instrument and known patch, cancellation/crashes,
   state restoration, exact score timing, long streaming render and movement
   joins. This precursor is not audible VST3 support or Kontakt integration.

## Primary references and licensing

- [SDK plug-in locations](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical%2BDocumentation/Locations%2BFormat/Plugin%2BLocations.html)
- [SDK bundle structure and architectures](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical%2BDocumentation/Locations%2BFormat/Plugin%2BFormat.html)
- [SDK optional JSON5 module metadata](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical%2BDocumentation/VST%2BModule%2BArchitecture/ModuleInfo-JSON.html)
- [Microsoft PE specification](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format)
- [Linux ELF ABI declarations](https://github.com/torvalds/linux/blob/master/include/uapi/linux/elf.h)
- [Apple universal-binary layout](https://github.com/apple-oss-distributions/xnu/blob/main/EXTERNAL_HEADERS/mach-o/fat.h)
- [Apple CPU declarations](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/mach/machine.h)

The [VST3 SDK is MIT-licensed from version 3.8](https://steinbergmedia.github.io/vst3_dev_portal/pages/VST%2B3%2BLicensing/VST3%2BLicense.html).
This crate uses documented data formats, not SDK code. SDK licensing does not
license third-party plug-ins, sample libraries or trademarks, and gives no
permission to install or load any of them. No VST2 interface is implemented.
