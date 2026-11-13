import { describe, expect, it } from 'vitest';
import { clampZoom, engravingWidth, engravingPageWidth } from '../src/layout';
import { shell } from '../src/shell';

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
  it('provides unique targets and accessible view controls', () => {
    const ids = [...shell.matchAll(/\bid="([^"]+)"/g)].map(match => match[1]);
    expect(new Set(ids).size).toBe(ids.length);
    for (const id of ['view-split', 'view-score', 'toggle-inspector', 'toggle-title']) {
      expect(shell).toMatch(new RegExp(`id="${id}"[^>]*aria-pressed="(?:true|false)"`));
    }
  });
  it('keeps musical context without adding demo promotional text to the paper', () => {
    const paper = shell.slice(shell.indexOf('<article id="paper"'), shell.indexOf('</article>'));
    expect(paper).toContain('id="score-title"');
    expect(paper).not.toContain('id="tempo-text"');
    expect(paper).not.toContain('id="instrument-names"');
    expect(paper).not.toMatch(/study no|shared musical|drafting room/i);
  });
});
