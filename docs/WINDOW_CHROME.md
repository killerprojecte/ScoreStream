# Platform window chrome

ScoreStream's single 48px React application header replaces the separate OS title row. The titlebar implementation is split between `desktop/src/components/WindowControls.tsx`, `desktop/src/lib/window.ts`, and the native builder plus `window_chrome.rs`.

## Platform contract

- **macOS:** Keep decorations enabled, use Tauri's Overlay titlebar and hide the OS title. AppKit renders the real traffic lights at logical `(12, 18)` inside the application header. The start slot reserves 78px. Green keeps native fullscreen, Option-zoom and supported OS hover/tiling behavior. Command-M and Control-Command-F have WebView keyboard fallbacks. Double-click is released on mouse-up only when the pointer has not moved; the saved macOS preference selects zoom, minimize or no action. Newer macOS “Fill” is currently mapped to native zoom, not independently implemented Fill. Native macOS build, traffic-light position across scaling, Full Keyboard Access, fullscreen and OS-version-specific behavior still require macOS acceptance.
- **Windows:** Disable the separate OS titlebar and use right-aligned 46px rectangular caption buttons in minimize, maximize/restore, close order. Keep the native shadow request enabled and native resize handling. Close turns red on hover. Maximize/restore state is read back from the native window and refreshed after native resize/focus changes. Windows 11's maximize-hover Snap layout menu is not implemented by HTML buttons; it requires native `WM_NCHITTEST`/`HTMAXBUTTON` support. Windows native build, scaling, snapping, border and shadow acceptance remain pending.
- **Linux:** Disable the separate titlebar. Read GTK's `gtk-decoration-layout` without changing it, respecting button order, side and intentionally hidden buttons. Read `gtk-titlebar-double-click` for maximize, minimize or no-action. Breeze themes use flat compact buttons; the generic Linux style uses round caption targets. This is a platform-convention rendering, not a pixel-identical clone of every GTK/KDE theme. Four thin edges and four corners call native resize-drag for undecorated windows. Compositor/WM shadows are not guaranteed: Tauri's shadow setting is unsupported on Linux, and X11/Wayland/window-manager behavior differs. Verify each supported Linux display stack.
- **Browser preview:** No fake minimize, maximize or close buttons. This application requires its native backend.

Preferences are read at process startup. Changing desktop titlebar preferences requires reopening ScoreStream for the custom header to pick them up. No OS/desktop preferences are changed.

## Interaction and safety

Use `titlebarDragProps` on `.app-titlebar`; do not add `data-tauri-drag-region` as well, because it installs an independent double-click handler. Noninteractive text and empty header areas drag after 4px of primary-button movement, preventing a normal click/double-click from leaving a delayed native drag. Buttons, their SVG descendants, menus, inputs, links, editable controls and explicit `data-window-interactive` regions never initiate a drag. Caption buttons are keyboard-focusable native HTML buttons with English/Chinese accessible names and visible focus states.

All operations use `getCurrentWindow()`. Caption close calls Tauri `close()`, preserving the application's dirty/save cancellation handler. The Tauri 2.9 `onCloseRequested` helper itself invokes `destroy()` only after that handler permits closure, so the main-window capability includes this required completion action. Repeated asynchronous maximize/fullscreen actions are serialized, state is reread from the window, and errors are visible rather than silently ignored. Resize/focus subscriptions are removed when the last controls unmount, including late async registrations.

Only seven window action permissions were added to the main-window capability: minimize, toggle maximize, close, start dragging, start resize dragging set fullscreen and destroy (the framework's post-guard close completion). No shell, filesystem, credential, arbitrary window-creation or system-setting write permission is introduced.

## Validation

`desktop/tests/window.test.ts` covers GTK order and hidden controls, Windows placement, native-only macOS/browser rendering, interactive drag exclusion, maximize/fullscreen separation, cancellable close retries, native error recovery and async listener cleanup.

The opt-in existing native smoke harness calls `desktop/src/testing/window-smoke.ts` to inspect actual Tauri decoration state, caption order/accessibility, native maximize/restore and button-state synchronization. The helper restores the original maximize state and deliberately never minimizes or closes a dirty test fixture. Native movement, resizing, minimize/restore, keyboard activation and actual close/save cancellation require the isolated test window and CUA acceptance.

## Primary references

- [Tauri 2 window customization](https://v2.tauri.app/learn/window-customization/)
- [Tauri 2 window APIs](https://v2.tauri.app/reference/javascript/api/namespacewindow/)
- [Tauri core permissions](https://v2.tauri.app/reference/acl/core-permissions/)
- [Apple window guidance](https://developer.apple.com/design/human-interface-guidelines/windows)
- [Apple move and arrange windows](https://support.apple.com/en-afri/guide/mac-help/-mchlp2469/mac)
- [GTK decoration layout](https://docs.gtk.org/gtk3/property.Settings.gtk-decoration-layout.html)
- [Windows Snap layout requirements](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-snap-layout-menu)
