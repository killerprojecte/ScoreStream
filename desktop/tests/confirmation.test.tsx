// @vitest-environment jsdom
import {act} from 'react';
import {createRoot} from 'react-dom/client';
import {describe, expect, it, vi} from 'vitest';
import {ConfirmationController, createUnsavedCloseGuard} from '../src/app/confirmation';
import {useUnsavedConfirmation} from '../src/components/UnsavedConfirmation';
import {Workbench} from '../src/app/workbench';
import {setLanguage} from '../src/i18n';

const flush = async () => {await Promise.resolve(); await Promise.resolve();};
describe('confirmation lifecycle', () => {
  it('rejects concurrent requests and resolves pending decisions as cancelled on unmount', async () => {
    const confirmation = new ConfirmationController(); confirmation.activate();
    const pending = confirmation.request('replace');
    expect(confirmation.getSnapshot()).toBe('replace');
    expect(await confirmation.request('close')).toBe(false);
    confirmation.dispose(); expect(await pending).toBe(false);
    expect(await confirmation.request('close')).toBe(false);
    confirmation.activate(); const retry = confirmation.request('close'); confirmation.settle(true);
    expect(await retry).toBe(true);
  });
  it('prevents native close synchronously, ignores repeated clicks and allows the explicitly confirmed close once', async () => {
    const confirmation = new ConfirmationController(); confirmation.activate();
    const close = vi.fn(async () => {}), preventDefault = vi.fn();
    const guard = createUnsavedCloseGuard({hasChanges: () => true, confirm: confirmation.request, close, onError: vi.fn()});
    guard.onCloseRequested({preventDefault}); expect(preventDefault).toHaveBeenCalledTimes(1);
    guard.onCloseRequested({preventDefault}); expect(preventDefault).toHaveBeenCalledTimes(2);
    expect(close).not.toHaveBeenCalled(); confirmation.settle(true); await flush();
    expect(close).toHaveBeenCalledTimes(1);
    // Tauri's follow-up event can arrive after close() has resolved.
    guard.onCloseRequested({preventDefault}); expect(preventDefault).toHaveBeenCalledTimes(2);
    guard.onCloseRequested({preventDefault}); expect(preventDefault).toHaveBeenCalledTimes(3);
    confirmation.dispose(); guard.dispose();
  });
  it('never closes on cancellation or a late approval after unmount', async () => {
    const confirmation = new ConfirmationController(); confirmation.activate();
    const close = vi.fn(async () => {});
    const guard = createUnsavedCloseGuard({hasChanges: () => true, confirm: confirmation.request, close, onError: vi.fn()});
    guard.onCloseRequested({preventDefault() {}}); confirmation.settle(false); await flush();
    expect(close).not.toHaveBeenCalled();
    guard.onCloseRequested({preventDefault() {}}); guard.dispose(); confirmation.settle(true); await flush();
    expect(close).not.toHaveBeenCalled();
  });
  it('passes through clean closes and reports failed explicit closes without retaining approval', async () => {
    let dirty = false; const preventDefault = vi.fn(), onError = vi.fn();
    const guard = createUnsavedCloseGuard({hasChanges: () => dirty, confirm: async () => true, close: async () => {throw Error('close failed');}, onError});
    guard.onCloseRequested({preventDefault}); expect(preventDefault).not.toHaveBeenCalled();
    dirty = true; guard.onCloseRequested({preventDefault}); await flush();
    expect(onError).toHaveBeenCalledTimes(1);
    guard.onCloseRequested({preventDefault}); expect(preventDefault).toHaveBeenCalledTimes(2); await flush(); guard.dispose();
  });
});

it('renders official AlertDialog controls, protects outside interactions, focuses Cancel, cancels Escape and resolves on unmount', async () => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true); setLanguage('en');
  const host = document.createElement('div'); document.body.append(host); const root = createRoot(host);
  const app = new Workbench();
  let request!: (kind: 'replace' | 'close') => Promise<boolean>;
  const bind = vi.spyOn(app, 'bindConfirmation').mockImplementation(confirm => {request = confirm; return () => {};});
  function Harness() {const confirmation = useUnsavedConfirmation(app); return confirmation.dialog;}
  try {
    await act(async () => root.render(<Harness/>));
    let pending!: Promise<boolean>;
    await act(async () => {pending = request('replace');});
    const dialog = document.querySelector('[role="alertdialog"]')!;
    expect(dialog.getAttribute('data-slot')).toBe('alert-dialog-content');
    expect(dialog.textContent).toContain('Replace the current unsaved changes');
    expect(document.activeElement?.textContent).toBe('Cancel');
    expect(dialog.querySelectorAll('[data-slot="alert-dialog-action"], [data-slot="alert-dialog-cancel"]')).toHaveLength(2);
    const settled = vi.fn(); void pending.then(settled);
    await act(async () => {
      const overlay = document.querySelector('[data-slot="alert-dialog-overlay"]')!;
      overlay.dispatchEvent(new MouseEvent('pointerdown', {bubbles: true, cancelable: true}));
      overlay.dispatchEvent(new MouseEvent('click', {bubbles: true}));
    });
    expect(document.querySelector('[role="alertdialog"]')).not.toBeNull();
    expect(settled).not.toHaveBeenCalled();
    expect(await request('close')).toBe(false);
    await act(async () => {document.activeElement!.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true}));});
    expect(await pending).toBe(false);
    await act(async () => {pending = request('replace');});
    const cancel = Array.from(document.querySelectorAll('button')).find(button => button.textContent === 'Cancel')!;
    await act(async () => cancel.click()); expect(await pending).toBe(false);
    await act(async () => {pending = request('close');});
    const close = Array.from(document.querySelectorAll('button')).find(button => button.textContent === 'Close')!;
    await act(async () => close.click()); expect(await pending).toBe(true);
    await act(async () => {pending = request('replace');});
    await act(async () => root.unmount()); expect(await pending).toBe(false);
  } finally {bind.mockRestore(); app.dispose(); host.remove(); vi.unstubAllGlobals();}
});

it('keeps the unsaved source when the real Workbench open flow is cancelled or repeated', async () => {
  vi.useFakeTimers(); vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true); setLanguage('en');
  const host = document.createElement('div'); document.body.append(host); const root = createRoot(host);
  const app = new Workbench();
  app.changed('private unsaved source');
  function Harness() {return useUnsavedConfirmation(app).dialog;}
  try {
    await act(async () => root.render(<Harness/>));
    let opening!: Promise<void>;
    await act(async () => {opening = app.open();});
    await app.open();
    expect(document.querySelectorAll('[role="alertdialog"]')).toHaveLength(1);
    expect(app.getSnapshot().source).toBe('private unsaved source');
    const cancel = document.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-cancel"]')!;
    await act(async () => cancel.click()); await opening;
    expect(app.getSnapshot().source).toBe('private unsaved source');
    expect(app.unsaved).toBe(true);
    await act(async () => {opening = app.open();});
    await act(async () => {document.activeElement!.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true}));});
    await opening;
    expect(app.getSnapshot().source).toBe('private unsaved source');
    expect(app.unsaved).toBe(true);
  } finally {await act(async () => root.unmount()); app.dispose(); host.remove(); vi.useRealTimers(); vi.unstubAllGlobals();}
});
