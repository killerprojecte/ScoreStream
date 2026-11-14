import {useEffect, useMemo, useSyncExternalStore} from 'react';
import {getCurrentWindow} from '@tauri-apps/api/window';
import {native} from '../bridge';
import {describeError} from '../errors';
import {t} from '../i18n';
import type {Workbench} from '../app/workbench';
import {ConfirmationController, createUnsavedCloseGuard} from '../app/confirmation';
import {AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle} from './ui/alert-dialog';

export function useUnsavedConfirmation(controller: Workbench) {
  const confirmation = useMemo(() => new ConfirmationController(), [controller]);
  const kind = useSyncExternalStore(confirmation.subscribe, confirmation.getSnapshot);
  useEffect(() => {
    confirmation.activate();
    const unbind = controller.bindConfirmation(confirmation.request);
    return () => {unbind(); confirmation.dispose();};
  }, [controller, confirmation]);
  useEffect(() => {
    const unload = (event: BeforeUnloadEvent) => {
      if (!native && (controller.dirty || controller.unsaved)) {event.preventDefault(); event.returnValue = '';}
    };
    window.addEventListener('beforeunload', unload);
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    const guard = createUnsavedCloseGuard({
      hasChanges: () => controller.dirty || controller.unsaved,
      confirm: confirmation.request,
      close: () => getCurrentWindow().close(),
      onError: error => controller.message(describeError(error)),
    });
    if (native) void getCurrentWindow().onCloseRequested(guard.onCloseRequested)
      .then(stop => {if (cancelled) stop(); else unlisten = stop;})
      .catch(error => {if (!cancelled) controller.message(describeError(error));});
    return () => {cancelled = true; guard.dispose(); unlisten?.(); window.removeEventListener('beforeunload', unload);};
  }, [controller, confirmation]);
  return {
    open: kind !== null,
    dialog: <AlertDialog open={kind !== null} onOpenChange={open => {if (!open) confirmation.settle(false);}}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t('Unsaved changes')}</AlertDialogTitle>
          <AlertDialogDescription>{t(kind === 'close' ? 'Close without saving the current changes?' : 'Replace the current unsaved changes or uncommitted draft?')}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel onClick={() => confirmation.settle(false)}>{t('Cancel')}</AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={() => confirmation.settle(true)}>{t(kind === 'close' ? 'Close' : 'Discard')}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>,
  };
}
