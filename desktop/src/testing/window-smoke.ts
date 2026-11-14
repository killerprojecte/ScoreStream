import { getCurrentWindow } from '@tauri-apps/api/window';
import { controlsForPlatform, getWindowState, nativeWindow, performWindowAction, windowChrome } from '../lib/window';

export async function runWindowSmoke(assert: (ok: unknown, message: string) => void) {
  if (!nativeWindow) throw new Error('Native window smoke requires the Tauri host');
  const win = getCurrentWindow();
  const until = async (predicate: () => Promise<boolean>, message: string) => {
    const deadline = Date.now() + 5000;
    while (Date.now() < deadline) {
      if (await predicate()) return;
      await new Promise(resolve => setTimeout(resolve, 40));
    }
    throw new Error(message);
  };
  assert(document.querySelectorAll('.app-titlebar').length === 1, 'one React application titlebar');
  assert(!document.querySelector('.app-titlebar[data-tauri-drag-region]'), 'no competing automatic drag handler');
  assert(await win.isDecorated() === (windowChrome.platform === 'macos'), 'platform decoration policy is active in native window');
  const expected = [...controlsForPlatform(windowChrome, 'start'), ...controlsForPlatform(windowChrome, 'end')];
  const buttons = [...document.querySelectorAll<HTMLButtonElement>('[data-window-action]')];
  assert(buttons.map(button => button.dataset.windowAction).join(',') === expected.join(','), 'caption buttons follow native platform order');
  assert(buttons.every(button => button.type === 'button' && !!button.getAttribute('aria-label')), 'caption controls have accessible button names');
  if (windowChrome.platform === 'macos') assert(document.querySelectorAll('.native-traffic-light-space').length === 1, 'space is reserved for AppKit traffic lights, not fake HTML controls');
  if (await win.isFullscreen()) throw new Error('Start isolated titlebar smoke outside fullscreen');
  const wasMaximized = await win.isMaximized();
  try {
    const button = document.querySelector<HTMLButtonElement>('[data-window-action="maximize"]');
    if (button) button.click(); else await performWindowAction('maximize');
    await until(async () => await win.isMaximized() !== wasMaximized, 'native maximize/restore did not change window state');
    await until(async () => !getWindowState().pending && getWindowState().maximized !== wasMaximized, 'caption state did not synchronize after native resize');
    assert(true, 'caption action changes real native maximize state');
    assert(!getWindowState().error, 'no denied IPC or native action error');
    if (button) {
      assert(button.getAttribute('aria-label')?.includes(wasMaximized ? 'Maximize' : 'Restore') || button.getAttribute('aria-label')?.includes(wasMaximized ? '最大化' : '还原'), 'maximize button accessible label follows native state');
    }
    await performWindowAction('maximize');
    await until(async () => await win.isMaximized() === wasMaximized, 'restore did not return original native state');
    assert(true, 'repeated maximize action restores native window state');
  } finally {
    if (await win.isMaximized() !== wasMaximized) { await win.toggleMaximize(); await until(async () => await win.isMaximized() === wasMaximized, 'cleanup could not restore original window'); }
  }
  // Moving, edge resizing, minimizing and real close/dirty cancellation are deliberately manual CUA checks.
  // No close call is made here: another listener may present a save/discard decision for the test fixture.
}
