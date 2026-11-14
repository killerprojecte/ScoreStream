import {afterEach, describe, expect, it, vi} from 'vitest';
import {Workbench} from '../src/app/workbench';
import type {RenderRequest, RenderResult, View} from '../src/types';

// This suite checks the React external-store controller only. Native geometry
// and the actual Rust/Verovio path are exercised by src/smoke.ts, not mocked here.
vi.mock('../src/render/project', () => ({RENDERER: 'test-renderer', project: () => ({mei: '<mei/>', map: {glyph: 'n1'}, warnings: []})}));

const view = (patch: Partial<View> = {}): View => ({
  valid: true, source: 'original score', diagnostics: [],
  events: [{id: 'n1', voice: 'rh', pitch: 'E4', time: '0', duration: '1', staff: 1, chord: null, written_duration: null, dots: 0, source_line: 11}],
  parts: [{id: 'piano', instrument: 'piano', staves: 1}], voices: [{id: 'rh', part: 'piano', staff: 1}],
  states: [], measures: [], document: {}, revision_id: 'r1', dirty: false, draft_seq: 0, can_undo: false, can_redo: false, ...patch,
});
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => {resolve = yes; reject = no;});
  return {promise, resolve, reject};
}
class Renderer {
  onmessage: ((event: MessageEvent<RenderResult>) => void) | null = null;
  onerror: ((event: ErrorEvent) => void) | null = null;
  requests: RenderRequest[] = [];
  postMessage = (request: RenderRequest) => {this.requests.push(request);};
  terminate = vi.fn();
  finish(request = this.requests[this.requests.length - 1]) {
    this.onmessage?.(new MessageEvent<RenderResult>('message', {data: {...request, pages: ['<svg><g id="glyph"/></svg>'], elapsed: 1}}));
  }
}
const controllers: Workbench[] = [];
function setup() {
  const api = {
    openDemo: vi.fn(async () => view()), current: vi.fn(async () => view()),
    preview: vi.fn(async (source: string) => view({source, dirty: source !== 'original score'})),
    apply: vi.fn(async (source: string, _revision: string) => view({source, revision_id: 'r2', can_undo: true})),
    undo: vi.fn(async () => view()), redo: vi.fn(async () => view({revision_id: 'r2'})),
    setPitch: vi.fn(async (_id: string, pitch: string, _revision: string) => view({revision_id: 'r2', events: [{...view().events[0], pitch}]})),
    setDuration: vi.fn(async (_id: string, duration: string, _revision: string) => view({revision_id: 'r2', events: [{...view().events[0], duration}]})),
    openFile: vi.fn(async (): Promise<{path: string; source: string} | null> => null),
    saveFile: vi.fn(async (_source: string): Promise<string | null> => null),
  };
  const app = new Workbench(api, true), renderer = new Renderer();
  app.attachRenderer(renderer as unknown as Worker); controllers.push(app);
  const ready = async () => {await app.start(); await vi.waitFor(() => expect(renderer.requests).toHaveLength(1)); renderer.finish();};
  return {app, api, renderer, ready};
}
afterEach(() => {for (const app of controllers.splice(0)) app.dispose(); vi.restoreAllMocks();});

describe('React workbench external store', () => {
  it('publishes immutable snapshots that useSyncExternalStore can subscribe to', async () => {
    const {app, ready} = setup();
    const original = app.getSnapshot(), changed = vi.fn();
    const unsubscribe = app.subscribe(changed);
    await ready();
    expect(app.getSnapshot()).not.toBe(original);
    expect(original.current).toBeNull();
    expect(app.phase).toBe('ready'); expect(app.published?.source).toBe('original score');
    expect(changed).toHaveBeenCalled();
    unsubscribe(); changed.mockClear(); app.setZoom(1.5);
    expect(changed).not.toHaveBeenCalled();
  });

  it('never invents a browser mock backend when the native host is unavailable', async () => {
    const {api} = setup(); const app = new Workbench(api, false); controllers.push(app);
    await app.start();
    expect(api.current).not.toHaveBeenCalled();
    expect(app.phase).toBe('unsupported');
    expect(app.getSnapshot().message).toContain('native desktop app');
  });

  it('ignores an older validation response after a newer invalid edit', async () => {
    const {app, api, renderer, ready} = setup(); await ready();
    const first = deferred<View>(); api.preview.mockImplementationOnce(() => first.promise);
    app.changed('first draft'); const older = app.preview();
    app.changed('invalid second draft');
    api.preview.mockResolvedValueOnce(view({source: 'invalid second draft', valid: false, diagnostics: [{line: 1, column: 1, code: 'BAD', message: 'Invalid', severity: 'error'}]}));
    await app.preview();
    first.resolve(view({source: 'first draft', dirty: true})); await older;
    expect(app.phase).toBe('invalid');
    expect(app.view?.source).toBe('invalid second draft');
    expect(app.published?.source).toBe('original score');
    expect(renderer.requests).toHaveLength(1);
  });

  it('rejects an obsolete renderer result after validation becomes invalid', async () => {
    const {app, api, renderer, ready} = setup(); await ready();
    app.changed('first draft'); await app.preview();
    await vi.waitFor(() => expect(renderer.requests).toHaveLength(2));
    app.changed('invalid newer draft');
    api.preview.mockResolvedValueOnce(view({source: 'invalid newer draft', valid: false})); await app.preview();
    renderer.finish();
    expect(app.phase).toBe('invalid');
    expect(app.published?.source).toBe('original score');
    expect(app.current?.revision_id).toBe('r1');
  });

  it('uses the current committed revision when applying a validated draft', async () => {
    const {app, api, renderer, ready} = setup(); await ready();
    app.changed('edited score'); await app.preview(); renderer.finish();
    expect(app.dirty).toBe(true); await app.apply();
    expect(api.apply).toHaveBeenCalledWith('edited score', 'r1');
    expect(app.current?.source).toBe('edited score');
    expect(app.current?.revision_id).toBe('r2'); expect(app.dirty).toBe(false);
  });

  it('surfaces rejected property mutations while preserving source and selection', async () => {
    const {app, api, ready} = setup(); await ready(); app.select('n1');
    api.setPitch.mockRejectedValueOnce(new Error('Invalid pitch'));
    await expect(app.mutate('pitch', 'invalid')).rejects.toThrow('Invalid pitch');
    expect(api.setPitch).toHaveBeenCalledWith('n1', 'invalid', 'r1');
    expect(app.current?.source).toBe('original score'); expect(app.selected?.id).toBe('n1');
    expect(app.busy).toBe(false); expect(app.getSnapshot().message).toContain('Invalid pitch');
  });

  it('does not submit unchanged values or concurrent double-click mutations', async () => {
    const {app, api, ready} = setup(); await ready(); app.select('n1');
    await app.mutate('pitch', 'E4'); expect(api.setPitch).not.toHaveBeenCalled();
    const pending = deferred<View>(); api.setPitch.mockImplementationOnce(() => pending.promise);
    const first = app.mutate('pitch', 'F4');
    await app.mutate('pitch', 'F4'); await app.mutate('duration', '2');
    expect(api.setPitch).toHaveBeenCalledTimes(1); expect(api.setDuration).not.toHaveBeenCalled();
    pending.resolve(view({revision_id: 'r2', events: [{...view().events[0], pitch: 'F4'}]})); await first;
    expect(app.selected?.pitch).toBe('F4'); expect(app.busy).toBe(false);
  });

  it('keeps zoom and selection presentation-only', async () => {
    const {app, api, renderer, ready} = setup(); await ready();
    const sequence = app.sequence, current = app.current;
    app.setZoom(200); app.select('n1');
    expect(app.zoom).toBe(2); expect(app.selected?.id).toBe('n1');
    expect(app.current).toBe(current); expect(app.sequence).toBe(sequence);
    expect(renderer.requests).toHaveLength(1); expect(api.apply).not.toHaveBeenCalled();
  });
});

describe('asynchronous unsaved file confirmation', () => {
  it('keeps the draft and never opens the picker when cancelled or no UI is bound', async () => {
    const {app, api, ready} = setup(); await ready(); app.changed('private draft');
    await app.open(); expect(api.openFile).not.toHaveBeenCalled();
    const confirm = vi.fn(async () => false); app.bindConfirmation(confirm);
    await app.open(); expect(confirm).toHaveBeenCalledWith('replace');
    expect(api.openFile).not.toHaveBeenCalled(); expect(app.getSnapshot().source).toBe('private draft');
  });
  it('opens only once after confirmation despite repeated requests', async () => {
    const {app, api, ready} = setup(); await ready(); app.changed('private draft');
    const decision = deferred<boolean>(), confirm = vi.fn(() => decision.promise); app.bindConfirmation(confirm);
    api.openFile.mockResolvedValueOnce({path: '/tmp/replacement.sfs', source: 'replacement score'});
    const first = app.open(); await app.open();
    expect(confirm).toHaveBeenCalledTimes(1); expect(api.openFile).not.toHaveBeenCalled();
    decision.resolve(true); await first;
    expect(api.openFile).toHaveBeenCalledTimes(1); expect(app.getSnapshot().source).toBe('replacement score');
  });
  it('ignores approval after the confirmation UI detaches', async () => {
    const {app, api, ready} = setup(); await ready(); app.changed('private draft');
    const decision = deferred<boolean>(), unbind = app.bindConfirmation(() => decision.promise);
    const pending = app.open(); unbind(); decision.resolve(true); await pending;
    expect(api.openFile).not.toHaveBeenCalled();
  });
  it('ignores a picker result returned after disposal', async () => {
    const {app, api, ready} = setup(); await ready();
    const file = deferred<{path: string; source: string} | null>(); api.openFile.mockReturnValueOnce(file.promise);
    const pending = app.open(); app.dispose(); file.resolve({path: '/tmp/stale.sfs', source: 'stale'}); await pending;
    expect(app.getSnapshot().source).toBe('original score');
  });
});
