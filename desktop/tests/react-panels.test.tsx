// @vitest-environment jsdom
import {act, useState} from 'react';
import {createRoot, type Root} from 'react-dom/client';
import {afterEach, beforeEach, describe, expect, it, vi} from 'vitest';
import {Inspector, type InspectorProps} from '../src/components/Inspector';
import {PreferencesDialog, type PreferenceTab} from '../src/components/PreferencesDialog';
import {McpSettings} from '../src/components/McpSettings';
import {McpController, type McpApi, type McpStatus} from '../src/mcp/model';
import {defaults} from '../src/preferences-model';
import {setLanguage} from '../src/i18n';
import type {ScoreEvent, View} from '../src/types';

const note: ScoreEvent = {id: 'n1', voice: 'rh', pitch: 'E4', time: '1', duration: '1', staff: 1, chord: null, written_duration: null, dots: 0, source_line: 11};
const score = (): View => ({valid: true, source: '', diagnostics: [], events: [note], parts: [{id: 'p', instrument: 'piano', staves: 2}], voices: [{id: 'rh', part: 'p', staff: 1}], states: [{time: '0', scope: 'score', values: {tempo: 'q:104'}}, {time: '0', scope: 'p.s1', values: {meter: '4/4'}}], measures: [{id: 'm1', part: 'p', time: '0', length: '4', label: '1'}], document: {frames: []}, revision_id: 'r1', dirty: false, draft_seq: 0, can_undo: false, can_redo: false});
const initialStatus: McpStatus = {settings: {bind: '127.0.0.1', port: 3210}, running: false, started: false, sse_url: null, auth_required: false, error: null};
let host: HTMLDivElement;
let root: Root;
const get = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
async function click(element: HTMLElement) {await act(async () => {element.dispatchEvent(new MouseEvent('click', {bubbles: true}));});}
async function key(element: HTMLElement, value: string) {await act(async () => {element.dispatchEvent(new KeyboardEvent('keydown', {key: value, bubbles: true, cancelable: true}));});}
async function input(id: string, value: string) {
  const element = get<HTMLInputElement>(id);
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(element, value);
    element.dispatchEvent(new Event('input', {bubbles: true}));
  });
}
function mcpApi(): McpApi {return {status: vi.fn(async () => initialStatus), save: vi.fn(async () => initialStatus), start: vi.fn(async () => ({...initialStatus, running: true, started: true})), stop: vi.fn(async () => initialStatus)};}

beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  setLanguage('en');
  host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
});
afterEach(async () => {await act(async () => {root.unmount();}); document.body.replaceChildren(); vi.unstubAllGlobals();});

async function inspect(patch: Partial<InspectorProps> = {}) {
  await act(async () => {root.render(<Inspector view={score()} selected={note} fileName="First light.sfs" canEdit dirty={false} onCommit={async () => {}} onReveal={() => {}} {...patch} />);});
}

describe('React Inspector', () => {
  it('renders document overview without fake selection and uses labelled controls for selected notes', async () => {
    await inspect({selected: null});
    expect(get('inspector-title').textContent).toBe('Document');
    expect(get('document-measures').textContent).toBe('1');
    expect(get('document-events').textContent).toBe('1');
    expect(get('pitch-input')).toBeNull();
    await inspect();
    expect(get('inspector-title').textContent).toBe('Note');
    expect(document.querySelector('label[for="pitch-input"]')?.textContent).toBe('Pitch');
    expect(get('duration-input').getAttribute('aria-describedby')).toContain('duration-unit');
    expect(get('set-pitch').getAttribute('aria-label')).toBe('Apply pitch');
  });

  it('shows only explicit score metadata, never a file-derived or generic title', async () => {
    const view = score();
    view.document.metadata = {title: '  Evening study  ', description: 'A study for piano.', authors: ['Composer', 'Lyricist'], header: 'Print header', footer: 'Print footer'};
    await inspect({view, selected: null, fileName: 'not-the-score-title.sfs'});
    expect(get('document-title').textContent).toBe('Evening study');
    expect(get('document-description').textContent).toBe('A study for piano.');
    expect(get('document-authors').textContent).toBe('Composer, Lyricist');
    expect(host.textContent).not.toContain('not-the-score-title');
    expect(host.textContent).not.toContain('ScoreStream document');
    expect(host.textContent).not.toContain('Print header');
    expect(host.textContent).not.toContain('Print footer');
    view.document.metadata = {title: '   ', description: '', authors: []};
    await inspect({view, selected: null, fileName: 'not-the-score-title.sfs'});
    expect(get('document-title')).toBeNull();
    expect(document.querySelector('.document-overview')).toBeNull();
    expect(host.textContent).not.toContain('not-the-score-title');
    expect(get('document-events').textContent).toBe('1');
  });

  it('keeps description and author-only metadata without inventing a missing title', async () => {
    const view = score();
    view.document.metadata = {description: '<b>Plain catalog text</b>', authors: ['Composer']};
    await inspect({view, selected: null});
    expect(get('document-title')).toBeNull();
    expect(get('document-description').textContent).toBe('<b>Plain catalog text</b>');
    expect(get('document-description').querySelector('b')).toBeNull();
    expect(get('document-authors').textContent).toBe('Composer');
  });

  it('preserves independently staged duration during pitch commit and temporary edit locks', async () => {
    let resolve!: () => void;
    const commit = vi.fn(() => new Promise<void>(done => {resolve = done;}));
    await inspect({onCommit: commit});
    await input('pitch-input', 'F4');
    await input('duration-input', '3/2');
    await click(get('set-pitch'));
    expect(commit).toHaveBeenCalledWith('pitch', 'F4');
    expect(get<HTMLInputElement>('duration-input').disabled).toBe(true);
    await inspect({canEdit: false, onCommit: commit});
    await inspect({selected: {...note, pitch: 'F4'}, onCommit: commit});
    await act(async () => {resolve();});
    expect(get<HTMLInputElement>('pitch-input').value).toBe('F4');
    expect(get<HTMLInputElement>('duration-input').value).toBe('3/2');
    expect(get<HTMLButtonElement>('set-pitch').disabled).toBe(true);
    expect(get<HTMLButtonElement>('set-duration').disabled).toBe(false);
  });

  it('preserves drafts on equivalent rerenders and resets them when selection identity changes', async () => {
    await inspect(); await input('pitch-input', 'F4');
    await inspect({selected: {...note}});
    expect(get<HTMLInputElement>('pitch-input').value).toBe('F4');
    await inspect({selected: {...note, id: 'n2', pitch: 'A4'}});
    expect(get<HTMLInputElement>('pitch-input').value).toBe('A4');
    expect(get<HTMLButtonElement>('set-pitch').disabled).toBe(true);
  });

  it('uses Rust commit rejection for accessible feedback and Escape resets only the current field', async () => {
    const commit = vi.fn(async () => {throw new Error('Invalid written pitch');});
    await inspect({onCommit: commit});
    await input('pitch-input', 'not a pitch'); await input('duration-input', '3/2');
    await key(get('pitch-input'), 'Enter');
    expect(get('inspector-feedback').getAttribute('role')).toBe('alert');
    expect(get('pitch-input').getAttribute('aria-invalid')).toBe('true');
    expect(get('pitch-input').getAttribute('aria-describedby')).toContain('inspector-feedback');
    await key(get('pitch-input'), 'Escape');
    expect(get<HTMLInputElement>('pitch-input').value).toBe('E4');
    expect(get<HTMLInputElement>('duration-input').value).toBe('3/2');
    expect(get('inspector-feedback')).toBeNull();
    expect(get('pitch-input').hasAttribute('aria-invalid')).toBe(false);
  });

  it('blocks unchanged, locked, and repeated commits, and reveals the actual selected note', async () => {
    let resolve!: () => void;
    const commit = vi.fn(() => new Promise<void>(done => {resolve = done;}));
    const reveal = vi.fn();
    await inspect({onCommit: commit, onReveal: reveal});
    await key(get('pitch-input'), 'Enter'); expect(commit).not.toHaveBeenCalled();
    await input('pitch-input', 'F4');
    await inspect({canEdit: false, dirty: true, onCommit: commit});
    await key(get('pitch-input'), 'Enter'); expect(commit).not.toHaveBeenCalled();
    expect(get('mutation-hint').textContent).toContain('Apply or discard');
    await inspect({onCommit: commit, onReveal: reveal});
    await key(get('pitch-input'), 'Enter'); await key(get('pitch-input'), 'Enter');
    expect(commit).toHaveBeenCalledTimes(1);
    await act(async () => {resolve();});
    await click(get('jump')); expect(reveal).toHaveBeenCalledTimes(1);
  });

  it('accepts normalized values and restores field focus only after render unlock', async () => {
    let resolve!: () => void;
    const commit = vi.fn(() => new Promise<void>(done => {resolve = done;}));
    await inspect({onCommit: commit});
    await input('duration-input', '2/2');
    await act(async () => {get('duration-input').focus();});
    await key(get('duration-input'), 'Enter');
    await inspect({canEdit: false, onCommit: commit});
    await act(async () => {document.body.tabIndex = -1; document.body.focus(); document.body.removeAttribute('tabindex'); resolve();});
    expect(get<HTMLInputElement>('duration-input').value).toBe('1');
    expect(document.activeElement).toBe(document.body);
    await inspect({canEdit: true, onCommit: commit});
    expect(document.activeElement).toBe(get('duration-input'));
  });

  it('does not steal focus after the user navigates away during a pending mutation', async () => {
    let resolve!: () => void;
    const commit = vi.fn(() => new Promise<void>(done => {resolve = done;}));
    const elsewhere = document.createElement('button'); document.body.append(elsewhere);
    await inspect({onCommit: commit}); await input('pitch-input', 'F4');
    await act(async () => {get('pitch-input').focus();});
    await key(get('pitch-input'), 'Enter');
    await inspect({canEdit: false, onCommit: commit});
    await act(async () => {elsewhere.focus(); resolve();});
    await inspect({selected: {...note, pitch: 'F4'}, onCommit: commit});
    expect(document.activeElement).toBe(elsewhere);
  });

  it('uses an accessible collapsible and clears it when moving between events', async () => {
    await inspect();
    const trigger = document.querySelector<HTMLElement>('.source-details-trigger')!;
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    await click(trigger);
    expect(trigger.getAttribute('aria-expanded')).toBe('true');
    expect(get('event-id').textContent).toBe('n1');
    await inspect({selected: {...note, id: 'n2', pitch: 'r'}});
    expect(get('inspector-title').textContent).toBe('Rest');
    expect(document.querySelector('.source-details-trigger')?.getAttribute('aria-expanded')).toBe('false');
  });
});

describe('React Preferences', () => {
  function ControlledPreferences({initialTab = 'general', controller}: {initialTab?: PreferenceTab; controller: McpController}) {
    const [open, setOpen] = useState(true);
    const [tab, setTab] = useState<PreferenceTab>(initialTab);
    const [value, setValue] = useState(defaults());
    return <PreferencesDialog open={open} onOpenChange={setOpen} tab={tab} onTabChange={setTab} value={value} saved onChange={patch => setValue(current => ({...current, ...patch}))} mcp={controller} native />;
  }

  it('has valid Dialog and Tabs labels, dismisses with Escape, restores focus, and clears a session key', async () => {
    const origin = document.createElement('button'); origin.textContent = 'Preferences'; document.body.append(origin); origin.focus();
    const controller = new McpController(mcpApi(), () => {}); controller.token = 'session-only-key';
    await act(async () => {root.render(<ControlledPreferences controller={controller} />);});
    const dialog = get('preferences-dialog');
    expect(dialog.getAttribute('role')).toBe('dialog');
    expect(get(dialog.getAttribute('aria-labelledby')!).textContent).toBe('Preferences');
    for (const tab of document.querySelectorAll<HTMLElement>('[role="tab"]')) {
      expect(get(tab.getAttribute('aria-controls')!)).not.toBeNull();
    }
    expect(document.activeElement?.id).toBe('preferences-tab-general');
    await key(document.activeElement as HTMLElement, 'Escape');
    await act(async () => {await new Promise(done => setTimeout(done, 0));});
    expect(get('preferences-dialog')).toBeNull();
    expect(document.activeElement).toBe(origin);
    expect(controller.token).toBe('');
  });

  it('uses a vertical category sidebar with keyboard navigation', async () => {
    const controller = new McpController(mcpApi(), () => {});
    await act(async () => {root.render(<ControlledPreferences controller={controller} />);});
    expect(document.querySelector('[role=tablist]')?.getAttribute('aria-orientation')).toBe('vertical');
    await key(get('preferences-tab-general'), 'ArrowDown');
    await act(async () => {await new Promise(done => setTimeout(done, 0));});
    expect(document.activeElement).toBe(get('preferences-tab-appearance'));
    expect(get('preferences-tab-appearance').getAttribute('aria-selected')).toBe('true');
  });

  it('updates font and switches, keeps shared controls labelled, and has no fake audio settings', async () => {
    const controller = new McpController(mcpApi(), () => {});
    await act(async () => {root.render(<ControlledPreferences controller={controller} initialTab="appearance" />);});
    expect(get('pref-theme').getAttribute('role')).toBe('combobox');
    expect(get('pref-wrap').getAttribute('role')).toBe('switch');
    await click(get('pref-wrap'));
    expect(get('pref-wrap').getAttribute('aria-checked')).toBe('true');
    expect(get('pref-font').getAttribute('role')).toBe('combobox');
    HTMLElement.prototype.scrollIntoView = vi.fn();
    await key(get('pref-font'), 'Enter');
    const sizes = Array.from(document.querySelectorAll<HTMLElement>('[role="option"]'));
    expect(sizes.map(option => option.textContent)).toEqual(Array.from({length: 13}, (_, index) => String(index + 10)));
    await key(sizes.find(option => option.textContent === '18')!, 'Enter');
    expect(get('pref-font').textContent).toContain('18');
    await key(get('pref-font'), 'Enter');
    await key(Array.from(document.querySelectorAll<HTMLElement>('[role="option"]')).find(option => option.textContent === '22')!, 'Enter');
    expect(get('pref-font').textContent).toContain('22');
    await act(async () => {get('preferences-tab-audio').focus();});
    expect(get('audio-settings').textContent).toContain('when the audio engine is available');
    expect(get('audio-settings').querySelector('input,button')).toBeNull();
  });

  it('stays closed if a pending service start completes after dismissal', async () => {
    let resolve!: (status: McpStatus) => void;
    const api = mcpApi(); api.start = vi.fn(() => new Promise<McpStatus>(done => {resolve = done;}));
    const controller = new McpController(api, () => {}); controller.status = initialStatus;
    await act(async () => {root.render(<ControlledPreferences controller={controller} initialTab="mcp" />);});
    await input('mcp-key', 'session-only-key'); await click(get('mcp-start'));
    await click(get('close-preferences'));
    expect(controller.token).toBe('');
    await act(async () => {resolve({...initialStatus, running: true, started: true});});
    expect(controller.status?.running).toBe(true);
    expect(get('preferences-dialog')).toBeNull();
  });

  it('stays open when interacting with native window controls, resize edges, and the modal drag plane', async () => {
    const controller = new McpController(mcpApi(), () => {});
    await act(async () => {root.render(<ControlledPreferences controller={controller} />);});
    await act(async () => {await new Promise(done => setTimeout(done, 0));});
    for (const className of ['window-controls', 'window-resize-edge', 'modal-titlebar-drag']) {
      const surface = document.createElement('div'); surface.className = className;
      const target = document.createElement('button'); surface.append(target); document.body.append(surface);
      await act(async () => {target.dispatchEvent(new MouseEvent('pointerdown', {bubbles: true, cancelable: true})); target.dispatchEvent(new MouseEvent('click', {bubbles: true, cancelable: true}));});
      expect(get('preferences-dialog')).not.toBeNull();
      surface.remove();
    }
    await act(async () => {document.body.dispatchEvent(new MouseEvent('pointerdown', {bubbles: true, cancelable: true})); document.body.dispatchEvent(new MouseEvent('click', {bubbles: true, cancelable: true}));});
    expect(get('preferences-dialog')).toBeNull();
  });

  it('renders translated labels while preserving accessible control identifiers', async () => {
    setLanguage('zh-CN');
    const controller = new McpController(mcpApi(), () => {});
    await act(async () => {root.render(<ControlledPreferences controller={controller} initialTab="appearance" />);});
    expect(get('preferences-title').textContent).toBe('首选项');
    expect(document.querySelector('label[for="pref-font"]')?.textContent).toBe('编辑器字号');
    expect(get('close-preferences').getAttribute('aria-label')).toBe('关闭');
  });
});

describe('React MCP settings', () => {
  it('never starts automatically and disables unsupported native controls', async () => {
    const api = mcpApi(); const controller = new McpController(api, () => {});
    await act(async () => {root.render(<McpSettings controller={controller} native={false} />);});
    expect(api.start).not.toHaveBeenCalled();
    expect(get<HTMLButtonElement>('mcp-start').disabled).toBe(true);
    expect(get('mcp-notice').textContent).toContain('only in the native desktop app');
  });

  it('masks session keys, copies only the clean URL, and provides keyboard fallback', async () => {
    const controller = new McpController(mcpApi(), () => {});
    const url = 'http://127.0.0.1:3210/sse';
    controller.status = {...initialStatus, running: true, started: true, auth_required: true, sse_url: url};
    controller.token = 'session-only-key';
    const writeText = vi.fn().mockResolvedValueOnce(undefined).mockRejectedValueOnce(new Error('Unavailable'));
    Object.defineProperty(navigator, 'clipboard', {configurable: true, value: {writeText}});
    await act(async () => {root.render(<McpSettings controller={controller} native />);});
    expect(get<HTMLInputElement>('mcp-key').type).toBe('password');
    expect(get('mcp-key-help').textContent).toContain('never saved to disk');
    expect(get('mcp-panel').textContent).toContain('Private drafts stay private');
    await click(get('mcp-copy')); expect(writeText).toHaveBeenCalledWith(url);
    expect(get<HTMLInputElement>('mcp-url').value).not.toContain(controller.token);
    await click(get('mcp-copy'));
    expect(document.activeElement).toBe(get('mcp-url'));
    expect(get<HTMLInputElement>('mcp-url').selectionStart).toBe(0);
    expect(get<HTMLInputElement>('mcp-url').selectionEnd).toBe(url.length);
    expect(get('mcp-notice').textContent).toContain('Ctrl/⌘ C');
  });

  it('warns on network scope, validates before starting, and blocks repeated start clicks', async () => {
    const api = mcpApi();
    let controller: McpController;
    const render = () => root.render(<McpSettings controller={controller} native />);
    controller = new McpController(api, render); controller.status = initialStatus;
    await act(async () => {render();});
    await input('mcp-bind', '0.0.0.0');
    expect(get('mcp-network-warning').textContent).toContain('other devices');
    await click(get('mcp-start'));
    expect(api.start).not.toHaveBeenCalled();
    expect(get('mcp-error').textContent).toContain('A key is required');
    await input('mcp-key', 'example-session-key');
    await click(get('mcp-start'));
    expect(api.start).toHaveBeenCalledWith({bind: '0.0.0.0', port: 3210}, 'example-session-key');
    expect(controller.token).toBe('');
    expect(get<HTMLButtonElement>('mcp-start').disabled).toBe(true);
    await click(get('mcp-start'));
    expect(api.start).toHaveBeenCalledTimes(1);
  });
});
