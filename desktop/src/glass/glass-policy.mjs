/** Standalone policy prototype. No OS name or user-agent branch is allowed. */
export function resolveGlassPolicy(preferences = {}, media = {}, native = {}, capabilities = {}) {
  const reduceTransparency = Boolean(media.reduceTransparency || native.reduceTransparency);
  const reduceMotion = Boolean(media.reduceMotion || native.reduceMotion);
  const increaseContrast = Boolean(media.increaseContrast || native.increaseContrast);
  const forcedColors = Boolean(media.forcedColors);
  const material = preferences.material === 'solid' || reduceTransparency || forcedColors || capabilities.compositorUnusable === true ? 'solid' : 'glass';
  return {
    theme: preferences.theme === 'dark' || (preferences.theme !== 'light' && Boolean(media.dark)) ? 'dark' : 'light',
    material,
    quality: preferences.quality === 'layered' || capabilities.backdropFilter === false ? 'layered' : 'blur',
    reducedMotion: reduceMotion,
    contrast: increaseContrast || forcedColors ? 'more' : 'standard',
  };
}

/** Future React useEffect adapter. Native state is accessibility only, not an OS gate.
 * Caller owns authenticated bridge/subscription and passes read-only booleans here.
 * No native commands, storage, network, system settings, or permissions are changed.
 */
export function installGlassPolicy(root, initialPreferences = {}, environment = globalThis) {
  const definitions = {
    dark: '(prefers-color-scheme: dark)',
    reduceTransparency: '(prefers-reduced-transparency: reduce)',
    reduceMotion: '(prefers-reduced-motion: reduce)',
    increaseContrast: '(prefers-contrast: more)',
    forcedColors: '(forced-colors: active)',
  };
  const queries = Object.fromEntries(Object.entries(definitions).map(([key, value]) => [key, environment.matchMedia?.(value)]));
  const supported = Boolean(environment.CSS?.supports?.('backdrop-filter', 'blur(1px)') || environment.CSS?.supports?.('-webkit-backdrop-filter', 'blur(1px)'));
  let preferences = {...initialPreferences}, native = {}, disposed = false;
  function apply() {
    if (disposed) return;
    const media = Object.fromEntries(Object.entries(queries).map(([key, query]) => [key, Boolean(query?.matches)]));
    const state = resolveGlassPolicy(preferences, media, native, {backdropFilter: supported});
    root.dataset.theme = state.theme;
    root.dataset.material = state.material;
    root.dataset.glassQuality = state.quality;
    root.dataset.reducedMotion = String(state.reducedMotion);
    root.dataset.contrast = state.contrast;
    return state;
  }
  for (const query of Object.values(queries)) query?.addEventListener?.('change', apply);
  apply();
  return {
    updatePreferences(next) {preferences = {...preferences, ...next}; return apply();},
    updateNativeAccessibility(next) {
      native = Object.fromEntries(['reduceTransparency', 'reduceMotion', 'increaseContrast'].map(key => [key, next[key] === true]));
      return apply();
    },
    dispose() {disposed = true; for (const query of Object.values(queries)) query?.removeEventListener?.('change', apply);},
  };
}
