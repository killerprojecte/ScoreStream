// @vitest-environment jsdom
import {act} from 'react';
import {createRoot} from 'react-dom/client';
import {describe, expect, it, vi} from 'vitest';
import {Inspector} from '../src/components/Inspector';
import type {ScoreEvent, View} from '../src/types';

const note = {id: 'n1', voice: 'v', pitch: 'E4', time: '0', duration: '1', staff: 1, chord: null, written_duration: null, dots: 0, source_line: 1} satisfies ScoreEvent;
const view = {valid: true, source: '', diagnostics: [], events: [note], parts: [], voices: [], states: [], measures: [], document: {frames: []}, revision_id: 'r1', dirty: false, draft_seq: 0, can_undo: false, can_redo: false} satisfies View;

describe('Inspector registry composition', () => {
  it('renders authored labels as text and respects empty versus absent', async () => {
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    const host = document.createElement('div'); document.body.append(host);
    const root = createRoot(host);
    try {
      const labeled: View = {...view, parts: [
        {id: 'f1', instrument: 'flute', staves: 1, display_name: 'Flute 1 <script>literal</script>'},
        {id: 'f2', instrument: 'flute', staves: 1, display_name: ''},
        {id: 'f3', instrument: 'flute', staves: 1},
      ]};
      await act(async () => root.render(<Inspector view={labeled} selected={null} fileName="score.sfs" canEdit dirty={false} onCommit={async () => {}} onReveal={() => {}} />));
      const labels = [...host.querySelectorAll('.property-entry dt')].map(node => node.textContent);
      expect(labels).toContain('Flute 1 <script>literal</script>');
      expect(labels).toContain(''); expect(labels).toContain('flute');
      expect(host.querySelector('script')).toBeNull();
    } finally {
      await act(async () => root.unmount()); host.remove(); vi.unstubAllGlobals();
    }
  });
  it('uses official input groups, addon buttons, tooltip triggers and collapsible Button without native titles', async () => {
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    const host = document.createElement('div'); document.body.append(host);
    const root = createRoot(host);
    try {
      await act(async () => root.render(<Inspector view={view} selected={note} fileName="score.sfs" canEdit dirty={false} onCommit={async () => {}} onReveal={() => {}} />));
      const groups = host.querySelectorAll('[data-slot="input-group"]');
      expect(groups.length).toBe(2);
      for (const group of groups) {
        expect(group.querySelector('input')?.getAttribute('data-slot')).toBe('input-group-control');
        expect(group.querySelector('[data-slot="input-group-addon"] button')?.getAttribute('data-size')).toBe('icon-xs');
      }
      const source = host.querySelector('.source-details-trigger')!;
      expect(source.tagName).toBe('BUTTON');
      expect(source.getAttribute('data-variant')).toBe('ghost');
      expect(host.querySelector('[title]')).toBeNull();
      expect(host.querySelector('#duration-unit')?.getAttribute('tabindex')).toBe('0');
      expect(host.querySelector('#jump')?.getAttribute('data-slot')).toBe('tooltip-trigger');
    } finally {
      await act(async () => root.unmount()); host.remove(); vi.unstubAllGlobals();
    }
  });
});
