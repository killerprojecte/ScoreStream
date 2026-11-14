export type ConfirmationKind = 'replace' | 'close';
export type ConfirmUnsaved = (kind: ConfirmationKind) => Promise<boolean>;

/** One destructive decision at a time. A second action must be requested again,
 * rather than inheriting approval for a different action. */
export class ConfirmationController {
  private pending: {kind: ConfirmationKind; resolve: (accepted: boolean) => void} | null = null;
  private listeners = new Set<() => void>();
  private active = false;
  subscribe = (listener: () => void) => {this.listeners.add(listener); return () => {this.listeners.delete(listener);};};
  getSnapshot = () => this.pending?.kind ?? null;
  activate = () => {this.active = true;};
  request: ConfirmUnsaved = kind => {
    if (!this.active || this.pending) return Promise.resolve(false);
    return new Promise<boolean>(resolve => {this.pending = {kind, resolve}; this.emit();});
  };
  settle = (accepted: boolean) => {
    const pending = this.pending;
    this.pending = null;
    this.emit();
    pending?.resolve(accepted);
  };
  dispose = () => {this.active = false; this.settle(false);};
  private emit() {for (const listener of this.listeners) listener();}
}

export interface CloseRequest {preventDefault(): void}
/** Tauri decides whether to close as soon as its event callback returns. Never
 * await the dialog before preventing the original close. The explicit second
 * close is allowed through once, without showing another confirmation. */
export function createUnsavedCloseGuard(options: {
  hasChanges(): boolean;
  confirm: ConfirmUnsaved;
  close(): Promise<void>;
  onError(error: unknown): void;
}) {
  let active = true, pending = false, approved = false;
  return {
    onCloseRequested(event: CloseRequest) {
      if (!active) return;
      if (approved) {approved = false; return;}
      if (!options.hasChanges()) return;
      event.preventDefault();
      if (pending) return;
      pending = true;
      void (async () => {
        try {
          if (!await options.confirm('close') || !active) return;
          approved = true;
          await options.close();
        } catch (error) {approved = false; if (active) options.onError(error);}
        finally {pending = false;}
      })();
    },
    dispose() {active = false;},
  };
}
