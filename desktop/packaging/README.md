# Linux bundles with WebKitGTK

For the Windows/macOS/Linux x64 and ARM64 GitHub Actions matrix, temporary
installer artifacts, and local build commands, see [Desktop CI installers](CI.md).
Those CI outputs are unsigned testing builds, not release-qualified packages.

## Maintained release route

Use `./desktop/packaging/build-appimage.sh` on an x86_64 Linux builder with the dependencies from `desktop/README.md` installed normally. It invokes the pinned Tauri CLI's AppImage bundler. Do not replace system libraries or disable the WebKit sandbox to make a build work.

Tauri's official [AppImage guide](https://v2.tauri.app/distribute/appimage/) recommends building on the oldest intended distribution baseline, such as Debian 12 or Ubuntu 22.04. Building on Debian 13 does **not** establish compatibility with those older systems. AppImage does not replace the kernel, graphics drivers, display server, or all core libc requirements. ARM needs a separate native build and test.

The [Tauri linuxdeploy GTK plugin](https://github.com/tauri-apps/linuxdeploy-plugin-gtk/blob/master/linuxdeploy-plugin-gtk.sh) relocates WebKit's compiled `/usr` paths to relative `././` paths. `AppRun` must run from the bundle's `usr` directory, and the full matching WebKit helper directory must be included. Merely shipping `libwebkit2gtk` is insufficient.

## Local validation fallback

`stage-appdir.py` stages a **Debian x86_64 validation bundle**, not a universal release pipeline, from an already verified extracted Debian sysroot. This is useful when the build workspace cannot install packages system-wide:

```sh
python desktop/packaging/stage-appdir.py \
  --sysroot /path/to/extracted-debian-root \
  --binary /path/to/scorestream-desktop \
  --packages /path/to/debian-package-metadata.json \
  --output /path/to/new/ScoreStream.AppDir
/path/to/new/ScoreStream.AppDir/AppRun
```

The sysroot must come from official Debian packages checked against repository SHA-256 metadata, including WebKitGTK 4.1, matching JavaScriptCore, GTK, WebKitWebProcess, WebKitNetworkProcess, WebKitGPUProcess, injected bundle, GIO modules, schemas, and their shared-library dependencies. This script does not download or authenticate a sysroot. It copies the runtime, preserves package copyright documents, deduplicates identical files, records file hashes and package provenance, and uses the same equal-size WebKit path relocation as Tauri's plugin. It makes no system changes and does not set sandbox-disable flags. It intentionally leaves libc and its loader to the base distribution.

Use a fresh output directory; existing output is rejected. The fallback currently copies more runtime libraries/resources than strictly necessary. Package size and a minimal dependency closure remain release work. The normal executable should be used for a user package; `smoke-test` is only for validation.

AppImage assembly can use the official [AppImage appimagetool](https://github.com/AppImage/appimagetool). Record the exact downloaded tool hash, assemble this AppDir, and rerun it on a clean target. If FUSE is unavailable, `--appimage-extract-and-run` avoids installing a FUSE service. Testing the unpacked AppDir does not by itself test the final AppImage.

## Security, licenses, and updates

- Never set `WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS`, disable OS security, or run the app as root to work around a failure. The launcher inherits WebKit's normal sandbox behavior; an isolated test environment is not a complete sandbox security audit.
- The validation bundle uses Debian 13 x86_64 WebKitGTK 2.52.6-1~deb13u1. Its package inventory and `usr/share/doc/*/copyright` are included. WebKit/JavaScriptCore include LGPL/BSD components, GTK/GLib are LGPL, and other components have their own licenses. Frontend notices and license texts are also included.
- A copyright directory alone is not a complete redistribution compliance process. Before public release, review the actual included-file inventory, supply corresponding source and required build/patch instructions for applicable LGPL/GPL components, and preserve third-party notices. The staging script contains the WebKit relocation modification. Do not claim that an unsigned local validation artifact is a release-ready distribution.
- Bundled WebKit does **not** receive OS WebKit security updates automatically. The maintainer must track upstream and Debian security updates, rebuild the whole bundle, and rerun native, file-dialog, rendering, and clean-host acceptance tests. This project has no automatic updater or signing configured.
- There is no audio/playback acceptance in this slice. Do not enable Tauri `bundleMediaFramework` or add optional codec plugins without separately checking their scope and licenses.

## Current acceptance

See `docs/DESKTOP_VALIDATION.md`. Native Linux editor and Verovio rendering have been tested using the bundled runtime on a host without system WebKitGTK. Windows, macOS, ARM, older Linux distributions, Wayland, GPU-specific behavior and end-user installers are separate acceptance targets.
