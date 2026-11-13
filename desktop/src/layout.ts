/** Fit is a stable 100% baseline. Zoom magnifies the same engraving rather than reflowing it. */
export const MIN_ZOOM = 0.6;
export const MAX_ZOOM = 2;
export function clampZoom(value: number): number {
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, Math.round(value * 10) / 10));
}
export function engravingWidth(canvasWidth: number, canvasInsets: number, paperInsets: number): number {
  return Math.max(280, Math.floor(canvasWidth - canvasInsets - paperInsets));
}
/** About 19% larger staff spacing than the earlier width * 2.5 layout. */
export function engravingPageWidth(width: number): number {
  return Math.max(600, Math.round(width * 2.1));
}
