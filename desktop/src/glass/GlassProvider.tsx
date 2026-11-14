import {useEffect, useRef, type ReactNode} from 'react';
import {installGlassPolicy, type AccessibilityState, type GlassController, type GlassPreferences} from './glass-policy.mjs';


/** Optional read-only system accessibility bridge. Never used to gate the theme by OS. */
export interface AccessibilityBridge {
  current(): Promise<AccessibilityState>;
  subscribe(callback: (state: AccessibilityState) => void): Promise<() => void>;
}

/** Shared root policy; attributes also style Radix portals. */
export function GlassProvider({children, preferences, accessibility}: {
  children: ReactNode;
  preferences: GlassPreferences;
  accessibility?: AccessibilityBridge;
}) {
  const controller = useRef<GlassController | null>(null);
  const latest = useRef(preferences);
  latest.current = preferences;
  useEffect(() => {
    const policy = installGlassPolicy(document.documentElement, latest.current);
    controller.current = policy;
    let stopped = false;
    let unsubscribe: (() => void) | undefined;
    let receivedEvent = false;
    if (accessibility) {
      // Subscribe before fetching, so a stale initial response cannot undo a newer event.
      void accessibility.subscribe(state => {
        if (!stopped) {receivedEvent = true; policy.updateNativeAccessibility(state);}
      }).then(off => {if (stopped) off(); else unsubscribe = off;}).catch(() => {});
      void accessibility.current().then(state => {
        if (!stopped && !receivedEvent) policy.updateNativeAccessibility(state);
      }).catch(() => {});
      // Missing platform signals do not disable the theme or alter system preferences.
      // CSS media signals and the explicit Solid option remain available.
    }
    return () => {stopped = true; unsubscribe?.(); policy.dispose(); controller.current = null;};
  }, [accessibility]);
  useEffect(() => {controller.current?.updatePreferences(preferences);}, [preferences]);
  return children;
}
