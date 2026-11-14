export interface GlassPreferences { theme?: 'system' | 'light' | 'dark'; material?: 'glass' | 'solid'; quality?: 'blur' | 'layered'; }
export interface AccessibilityState { reduceTransparency?: boolean; reduceMotion?: boolean; increaseContrast?: boolean; }
export interface MediaState extends AccessibilityState { dark?: boolean; forcedColors?: boolean; }
export interface GlassState { theme: 'light' | 'dark'; material: 'glass' | 'solid'; quality: 'blur' | 'layered'; reducedMotion: boolean; contrast: 'more' | 'standard'; }
export interface GlassController { updatePreferences(next: GlassPreferences): GlassState | undefined; updateNativeAccessibility(next: AccessibilityState): GlassState | undefined; dispose(): void; }
export function resolveGlassPolicy(preferences?: GlassPreferences, media?: MediaState, native?: AccessibilityState, capabilities?: {backdropFilter?: boolean; compositorUnusable?: boolean}): GlassState;
export function installGlassPolicy(root: HTMLElement, preferences?: GlassPreferences, environment?: typeof globalThis): GlassController;
