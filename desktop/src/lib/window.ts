import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
export type ResizeDirection = Parameters<ReturnType<typeof getCurrentWindow>['startResizeDragging']>[0];
import type { MouseEvent as ReactMouseEvent } from 'react';

export type WindowPlatform = 'macos' | 'windows' | 'linux' | 'browser';
export type WindowControl = 'minimize' | 'maximize' | 'close';
export interface WindowChrome {
  platform: WindowPlatform;
  decorationLayout: string;
  doubleClickAction: 'maximize' | 'minimize' | 'none';
  theme: string;
}
declare global { interface Window { __SCORESTREAM_WINDOW__?: WindowChrome } }
const browserChrome: WindowChrome = { platform: 'browser', decorationLayout: '', doubleClickAction: 'none', theme: '' };
export const windowChrome: WindowChrome = typeof window !== 'undefined' && isTauri()
  ? (window.__SCORESTREAM_WINDOW__ ?? browserChrome) : browserChrome;
export const nativeWindow = windowChrome.platform !== 'browser';

/** GTK's setting is ordered left:right. Ignore unknown menu/icon entries; never invent buttons the user hid. */
export function parseDecorationLayout(layout: string): { start: WindowControl[]; end: WindowControl[] } {
  const seen = new Set<string>();
  const parse = (side = '') => side.split(',').filter((item): item is WindowControl => {
    if (!['minimize', 'maximize', 'close'].includes(item) || seen.has(item)) return false;
    seen.add(item); return true;
  });
  const [left, right] = layout.split(':');
  return { start: parse(left), end: parse(right) };
}
export function controlsForPlatform(chrome: WindowChrome, placement: 'start' | 'end'): WindowControl[] {
  if (chrome.platform === 'windows') return placement === 'end' ? ['minimize', 'maximize', 'close'] : [];
  if (chrome.platform === 'linux') return parseDecorationLayout(chrome.decorationLayout)[placement];
  return [];
}
export interface NativeWindowState { maximized: boolean; fullscreen: boolean; focused: boolean; resizable: boolean; pending: boolean; error: string }
let state: NativeWindowState = { maximized: false, fullscreen: false, focused: true, resizable: true, pending: false, error: '' };
const listeners = new Set<() => void>();
let cleanups: (() => void)[] = [];
let generation = 0;
let refreshId = 0;
let settleTimers: ReturnType<typeof setTimeout>[] = [];
function settleWindowState() {
  settleTimers.forEach(clearTimeout);
  // GTK/WM state flags can settle after the resize event and the command reply.
  if (listeners.size) settleTimers = [80, 240].map(delay => setTimeout(() => { void refreshWindowState().catch(reportWindowError); }, delay));
}
function publish(patch: Partial<NativeWindowState>) { state = { ...state, ...patch }; listeners.forEach(notify => notify()); }
export const getWindowState = () => state;
export function clearWindowError() { publish({ error: '' }); }
function reportWindowError(error: unknown) { publish({ error: error instanceof Error ? error.message : String(error) }); }
export async function refreshWindowState() {
  if (!nativeWindow) return;
  const request = ++refreshId;
  const win = getCurrentWindow();
  const [maximized, fullscreen, focused, resizable] = await Promise.all([win.isMaximized(), win.isFullscreen(), win.isFocused(), win.isResizable()]);
  if (request === refreshId) publish({ maximized, fullscreen, focused, resizable });
}
export function subscribeWindowState(notify: () => void) {
  listeners.add(notify);
  if (listeners.size === 1 && nativeWindow) {
    const current = ++generation;
    const win = getCurrentWindow();
    const refresh = () => { void refreshWindowState().catch(reportWindowError); settleWindowState(); };
    refresh();
    for (const subscription of [win.onResized(refresh), win.onFocusChanged(refresh)]) {
      void subscription.then(stop => {
        if (current !== generation || !listeners.size) stop();
        else cleanups.push(stop);
      }).catch(reportWindowError);
    }
  }
  return () => {
    listeners.delete(notify);
    if (!listeners.size) { generation++; refreshId++; cleanups.forEach(stop => stop()); cleanups = []; settleTimers.forEach(clearTimeout); settleTimers = []; }
  };
}
/** All actions address only the invoking main window, never arbitrary window labels. Close remains cancellable. */
export async function performWindowAction(action: WindowControl | 'fullscreen') {
  if (!nativeWindow || state.pending) return;
  publish({ pending: true, error: '' });
  const win = getCurrentWindow();
  try {
    if (action === 'minimize') await win.minimize();
    else if (action === 'close') await win.close();
    else if (action === 'fullscreen') await win.setFullscreen(!(await win.isFullscreen()));
    else if (await win.isFullscreen()) await win.setFullscreen(false);
    else if (await win.isResizable()) await win.toggleMaximize();
    if (action !== 'close') { await refreshWindowState(); settleWindowState(); }
  } catch (error) { reportWindowError(error); }
  finally { publish({ pending: false }); }
}

const interactive = 'button,a,input,textarea,select,[contenteditable="true"],[role="button"],[role="tab"],[role="menuitem"],[role="combobox"],[data-window-interactive],[data-no-drag]';
export function isTitlebarDragTarget(target: EventTarget | null, header: EventTarget | null): boolean {
  return target instanceof Element && header instanceof Element && header.contains(target) && !target.closest(interactive);
}
let doubleClickPoint: { x: number; y: number } | null = null;
let dragPoint: { x: number; y: number } | null = null;
async function titlebarDoubleClick() {
  if (windowChrome.doubleClickAction === 'none' || state.fullscreen) return;
  await performWindowAction(windowChrome.doubleClickAction);
}
/** Do not combine with data-tauri-drag-region: that installs a second native double-click handler. */
export const titlebarDragProps = {
  onMouseDown(event: ReactMouseEvent<HTMLElement>) {
    if (!nativeWindow || event.button !== 0 || !isTitlebarDragTarget(event.target, event.currentTarget)) return;
    dragPoint = null;
    if (event.detail === 2) {
      event.preventDefault();
      if (windowChrome.platform === 'macos') doubleClickPoint = { x: event.clientX, y: event.clientY };
      else void titlebarDoubleClick();
    } else if (event.detail === 1) {
      doubleClickPoint = null;
      dragPoint = { x: event.clientX, y: event.clientY };
    }
  },
  onMouseMove(event: ReactMouseEvent<HTMLElement>) {
    if (!(event.buttons & 1)) { dragPoint = null; return; }
    if (!dragPoint || !isTitlebarDragTarget(event.target, event.currentTarget)) return;
    if (Math.hypot(event.clientX - dragPoint.x, event.clientY - dragPoint.y) < 4) return;
    dragPoint = null;
    event.preventDefault();
    void getCurrentWindow().startDragging().catch(reportWindowError);
  },
  onMouseLeave() { dragPoint = null; doubleClickPoint = null; },
  onMouseUp(event: ReactMouseEvent<HTMLElement>) {
    const point = doubleClickPoint; doubleClickPoint = null; dragPoint = null;
    if (windowChrome.platform === 'macos' && event.button === 0 && event.detail === 2 && point &&
        event.clientX === point.x && event.clientY === point.y && isTitlebarDragTarget(event.target, event.currentTarget)) {
      void titlebarDoubleClick();
    }
  },
};
export function startWindowResize(direction: ResizeDirection) {
  if (!nativeWindow || state.maximized || state.fullscreen || !state.resizable) return;
  void getCurrentWindow().startResizeDragging(direction).catch(reportWindowError);
}
