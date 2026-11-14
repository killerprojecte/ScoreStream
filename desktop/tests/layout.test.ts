import { describe, expect, it } from 'vitest';
import { clampZoom, engravingWidth, engravingPageWidth } from '../src/layout';
import { fitPanes } from '../src/preferences-model';

describe('score workspace layout', () => {
  it('measures the unzoomed canvas instead of the already magnified score', () => {
    expect(engravingWidth(630, 44, 58)).toBe(528);
    expect(engravingWidth(100, 44, 58)).toBe(280);
  });
  it('clamps and rounds repeated zoom steps', () => {
    expect(clampZoom(1.2000000000000002)).toBe(1.2);
    expect(clampZoom(-1)).toBe(.6);
    expect(clampZoom(5)).toBe(2);
    expect(clampZoom(1)).toBe(1);
  });
  it('keeps physical staff sizing stable as the pane changes width', () => {
    expect(engravingPageWidth(500)).toBe(1050);
    expect(engravingPageWidth(900)).toBe(1890);
    expect(engravingPageWidth(200)).toBe(600);
  });
  it('fits sidebars while preserving a useful score width', () => {
    const fit = fitPanes(1120,340,264);
    expect(fit).toEqual({source:340,inspector:264});
    expect(fitPanes(1120,680,440)).toEqual({source:348,inspector:440});
    expect(fitPanes(1120,340,264,false,false)).toEqual({source:0,inspector:0});
  });
});
