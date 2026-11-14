// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { controlsForPlatform, isTitlebarDragTarget, parseDecorationLayout, type WindowChrome } from '../src/lib/window';
const chrome = (platform: WindowChrome['platform'], decorationLayout = ''): WindowChrome => ({ platform, decorationLayout, doubleClickAction: 'maximize', theme: '' });
describe('native titlebar layout', () => {
  it('keeps macOS traffic lights native and browser preview without fake buttons', () => {
    for (const platform of ['macos', 'browser'] as const) for (const side of ['start', 'end'] as const) expect(controlsForPlatform(chrome(platform), side)).toEqual([]);
  });
  it('keeps Windows caption order at right', () => {
    expect(controlsForPlatform(chrome('windows'), 'start')).toEqual([]);
    expect(controlsForPlatform(chrome('windows'), 'end')).toEqual(['minimize', 'maximize', 'close']);
  });
  it('respects GTK left/right ordering and hidden minimize/maximize', () => {
    expect(parseDecorationLayout('close,minimize,maximize:')).toEqual({ start: ['close', 'minimize', 'maximize'], end: [] });
    expect(parseDecorationLayout('menu:close')).toEqual({ start: [], end: ['close'] });
    expect(parseDecorationLayout('menu:minimize,maximize,close')).toEqual({ start: [], end: ['minimize', 'maximize', 'close'] });
  });
  it('ignores non-caption or duplicate items', () => {
    expect(parseDecorationLayout('close,icon:close,unknown,maximize')).toEqual({ start: ['close'], end: ['maximize'] });
    expect(parseDecorationLayout(':')).toEqual({ start: [], end: [] });
  });
});
describe('titlebar drag isolation', () => {
  beforeEach(() => { document.body.innerHTML = '<header><span class="filename">score.sfs</span><button><svg><path/></svg></button><div role="menuitem"><span>File</span></div><input/><div data-window-interactive><span>View</span></div></header><main>Editor</main>'; });
  it('allows the header and filename', () => {
    const header = document.querySelector('header')!;
    expect(isTitlebarDragTarget(header, header)).toBe(true);
    expect(isTitlebarDragTarget(document.querySelector('.filename'), header)).toBe(true);
  });
  it('never swallows caption icons, menus, inputs or controls', () => {
    const header = document.querySelector('header')!;
    for (const selector of ['button', 'svg', 'path', '[role=menuitem] span', 'input', '[data-window-interactive] span', 'main']) expect(isTitlebarDragTarget(document.querySelector(selector), header)).toBe(false);
    expect(isTitlebarDragTarget(null, header)).toBe(false);
  });
});
describe('current-window actions and event cleanup', () => {
  beforeEach(() => { vi.resetModules(); vi.clearAllMocks(); });
  async function setup() {
    let maximized = false, fullscreen = false;
    const win = {
      isMaximized: vi.fn(async () => maximized), isFullscreen: vi.fn(async () => fullscreen), isFocused: vi.fn(async () => true), isResizable: vi.fn(async () => true),
      minimize: vi.fn(async () => {}), close: vi.fn(async () => {}), toggleMaximize: vi.fn(async () => { maximized = !maximized; }),
      setFullscreen: vi.fn(async (value: boolean) => { fullscreen = value; }), startDragging: vi.fn(async () => {}), startResizeDragging: vi.fn(async () => {}),
      onResized: vi.fn(async () => vi.fn()), onFocusChanged: vi.fn(async () => vi.fn()),
    };
    window.__SCORESTREAM_WINDOW__ = chrome('windows', ':minimize,maximize,close');
    vi.doMock('@tauri-apps/api/core', () => ({ isTauri: () => true }));
    vi.doMock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => win }));
    return { bridge: await import('../src/lib/window'), win };
  }
  it('starts drag only after primary-button movement, never after a plain click', async () => {
    const { bridge, win } = await setup();
    document.body.innerHTML = '<header>Title</header>';
    const header = document.querySelector('header');
    const event = (extra: Record<string, unknown> = {}) => ({ button: 0, buttons: 1, detail: 1, clientX: 10, clientY: 10, target: header, currentTarget: header, preventDefault: vi.fn(), ...extra }) as never;
    bridge.titlebarDragProps.onMouseDown(event());
    bridge.titlebarDragProps.onMouseUp(event({ buttons: 0 }));
    bridge.titlebarDragProps.onMouseMove(event({ clientX: 20, buttons: 0 }));
    expect(win.startDragging).not.toHaveBeenCalled();
    bridge.titlebarDragProps.onMouseDown(event());
    bridge.titlebarDragProps.onMouseMove(event({ clientX: 12 }));
    expect(win.startDragging).not.toHaveBeenCalled();
    bridge.titlebarDragProps.onMouseMove(event({ clientX: 17 }));
    bridge.titlebarDragProps.onMouseMove(event({ clientX: 21 }));
    expect(win.startDragging).toHaveBeenCalledOnce();
  });
  it('double-click toggles once without leaving a queued drag', async () => {
    const { bridge, win } = await setup();
    document.body.innerHTML = '<header>Title</header>';
    const header = document.querySelector('header');
    const event = (detail: number) => ({ button: 0, buttons: 1, detail, clientX: 10, clientY: 10, target: header, currentTarget: header, preventDefault: vi.fn() }) as never;
    bridge.titlebarDragProps.onMouseDown(event(1)); bridge.titlebarDragProps.onMouseUp(event(1));
    bridge.titlebarDragProps.onMouseDown(event(2)); bridge.titlebarDragProps.onMouseUp(event(2));
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(win.startDragging).not.toHaveBeenCalled(); expect(win.toggleMaximize).toHaveBeenCalledOnce();
  });
  it('toggles maximize/restore and treats fullscreen separately', async () => {
    const { bridge, win } = await setup();
    await bridge.performWindowAction('maximize'); expect(bridge.getWindowState().maximized).toBe(true);
    await bridge.performWindowAction('maximize'); expect(bridge.getWindowState().maximized).toBe(false);
    await bridge.performWindowAction('fullscreen'); expect(bridge.getWindowState().fullscreen).toBe(true);
    await bridge.performWindowAction('maximize'); expect(bridge.getWindowState().fullscreen).toBe(false);
    expect(win.toggleMaximize).toHaveBeenCalledTimes(2);
  });
  it('uses cancellable close and allows a later close', async () => {
    const { bridge, win } = await setup();
    await bridge.performWindowAction('close'); await bridge.performWindowAction('close');
    expect(win.close).toHaveBeenCalledTimes(2); expect(bridge.getWindowState().pending).toBe(false);
  });
  it('surfaces native errors and recovers', async () => {
    const { bridge, win } = await setup();
    win.minimize.mockRejectedValueOnce(new Error('native failure'));
    await bridge.performWindowAction('minimize'); expect(bridge.getWindowState().error).toBe('native failure');
    bridge.clearWindowError(); expect(bridge.getWindowState().error).toBe('');
    await bridge.performWindowAction('minimize'); expect(bridge.getWindowState().pending).toBe(false);
  });
  it('releases listeners if unmounted before async registration completes', async () => {
    const { bridge, win } = await setup();
    const resizeStop = vi.fn(), focusStop = vi.fn();
    win.onResized.mockResolvedValueOnce(resizeStop); win.onFocusChanged.mockResolvedValueOnce(focusStop);
    const stop = bridge.subscribeWindowState(vi.fn()); stop();
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(resizeStop).toHaveBeenCalledOnce(); expect(focusStop).toHaveBeenCalledOnce();
  });
});
