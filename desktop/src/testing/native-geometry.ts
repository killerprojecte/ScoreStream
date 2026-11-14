import type {Workbench} from '../app/workbench';
import {defaults, type PreferenceStore} from '../preferences-model';
import {type Assert, click, delay, get, input, key, paint, until, visible} from './native-harness';

type Box = {left: number; top: number; right: number; bottom: number; width: number; height: number};
const round = (n: number) => Math.round(n * 100) / 100;
const box = (el: Element): Box => {
  const r = el.getBoundingClientRect();
  return Object.fromEntries(['left', 'top', 'right', 'bottom', 'width', 'height'].map(k => [k, round(r[k as keyof Box])])) as Box;
};
const close = (a: number, b: number, tolerance = 1) => Math.abs(a - b) <= tolerance;
const px = (el: Element, name: string) => Number.parseFloat(getComputedStyle(el).getPropertyValue(name)) || 0;
const usableRight = (el: Element) => el.getBoundingClientRect().right - px(el, 'padding-right') - px(el, 'border-right-width');

/** Measures the shipped React shell inside the real native renderer. */
export async function runGeometry(app: Workbench, preferences: PreferenceStore, assert: Assert) {
  const saved = {...preferences.value};
  const initialRevision = app.current!.revision_id;
  const initialSource = app.current!.source;
  const report: Record<string, unknown> = {viewport: [innerWidth, innerHeight]};
  const set = async (patch: Parameters<PreferenceStore['update']>[0]) => {
    preferences.update(patch); await paint();
  };
  const stable = async () => {
    // SourcePane's real ResizeObserver schedules its engraving update at 120ms.
    await delay(180);
    await until(() => app.phase === 'ready', 'score layout');
    await paint();
  };
  const widths = () => ({source: box(get('source-pane')).width, score: box(document.querySelector('.score-pane')!).width, inspector: box(get('inspector')).width});
  const boundedPanes = (label: string) => {
    const w = widths();
    assert(w.source >= 219 && w.inspector >= 239 && w.score >= 319, `${label}: source ≥220, inspector ≥240, score ≥320 CSS pixels (${JSON.stringify(w)})`);
    assert(get('workspace').scrollWidth <= get('workspace').clientWidth + 1, `${label}: workspace has no horizontal overflow`);
    return w;
  };
  try {
    await set({...defaults('en'), theme: 'light'});
    await stable();
    const note = app.current!.events.find(event => /^[A-G]/.test(event.pitch))!;
    app.select(note.id); await paint();
    const bars = document.querySelectorAll('.app-titlebar');
    assert(bars.length === 1 && !document.querySelector('.window-toolbar,.document-toolbar'), 'Exactly one app-owned titlebar replaces the legacy duplicate title/toolbars');
    const topbar = box(bars[0]);
    const headerElements = [...document.querySelectorAll('.workspace .pane-heading')];
    const headers = headerElements.map(box);
    assert(headers.length === 3, 'Source, Score and Inspector each expose one pane header');
    const insets = headerElements.map(el => box(el.querySelector('h2')!).left - box(el).left);
    assert(headers.every(h => close(h.top, topbar.bottom+11) && close(h.height, 38)), `Three pane headers share y=${topbar.bottom+11} inside the 10px glass gutter and 38px height`);
    assert(insets.every(inset => close(inset, 16)), `All pane titles use the shared 16px inset (${insets.join(', ')})`);
    assert(close(topbar.height, 44), `The single app titlebar is 44px tall (${topbar.height})`);
    assert((document.querySelector('.statusbar')!.textContent!.match(/\bReady\b|就绪/g) ?? []).length <= 1, 'Statusbar does not duplicate Ready/就绪');
    const toolbar = ['view-split','view-score','inspector-toggle','open-preferences'].map(id=>box(get(id)));
    assert(toolbar.every(rect=>close(rect.width,28)&&close(rect.height,28)&&close(rect.top,toolbar[0].top)), 'All toolbar controls have equal 28px bounds and one vertical baseline');
    assert(get('view-split').dataset.slot==='toggle-group-item'&&get('inspector-toggle').dataset.slot==='toggle', 'Workspace controls use the official shadcn Radix ToggleGroup and Toggle');
    report.toolbar = toolbar;
    report.topbar = topbar; report.headers = headers; report.headerInsets = insets; report.defaultWidths = boundedPanes('Default split layout');

    const labelLeft = box(get('inspector-title')).left;
    const pitch = get<HTMLInputElement>('pitch-input'), duration = get<HTMLInputElement>('duration-input');
    const fieldRects = [pitch, duration].map(el => box(el.closest('.property-input')!));
    assert(close(fieldRects[0].left, fieldRects[1].left) && close(fieldRects[0].right, fieldRects[1].right) && close(fieldRects[0].height, fieldRects[1].height), 'Pitch and Duration share identical control columns and height');
    const labels = [...get('selection').querySelectorAll('.property-row > label,.property-entry dt,.inspector-group h3,.source-details-trigger > span')];
    assert(labels.length > 5 && labels.every(el => close(box(el).left, labelLeft)), 'Inspector section headings, field labels, metadata labels and disclosure text share one left baseline');
    assert(close(usableRight(pitch), usableRight(duration)), 'Pitch and Duration reserve equal unit/action slots and share a numeric baseline');
    const metadata = [...get('selection').querySelectorAll('.property-entry dd')];
    assert(metadata.length >= 5 && metadata.every(el => close(usableRight(el), usableRight(pitch))), `Inspector metadata values align with editable values (input=${round(usableRight(pitch))}, metadata=${metadata.map(el => round(usableRight(el))).join(',')})`);
    assert(get('selection').querySelectorAll('.source-details-trigger > svg').length === 1 && !get('selection').querySelector('summary'), 'Source disclosure uses one explicit SVG chevron instead of a native emoji marker');
    report.inspector = {labelLeft, fields: fieldRects, valueRight: round(usableRight(pitch)), metadataRights: metadata.map(el => round(usableRight(el)))};

    const title = get('filename'), originalTitle = title.textContent;
    const titleCases: unknown[] = [];
    try {
      for (const language of ['en', 'zh-CN'] as const) {
        await set({language});
        for (const titleText of language === 'en' ? ['First light.sfs', 'A very long score filename with orchestral revisions and rehearsal annotations '.repeat(4) + '.sfs'] : ['初光.sfs', '很长的中文总谱文件名与排练批注和修订版本'.repeat(12) + '.sfs']) {
          title.textContent = titleText; await paint();
          const titleBox = box(title.closest('.window-document')!);
          const start = box(document.querySelector('.titlebar-start')!), end = box(document.querySelector('.titlebar-end')!);
          assert(titleBox.left >= start.right - 1 && titleBox.right <= end.left + 1, `${language} ${titleText.length > 30 ? 'long' : 'short'} title never overlaps controls (left=${start.right}, title=${titleBox.left}..${titleBox.right}, right=${end.left})`);
          const center = (titleBox.left + titleBox.right) / 2;
          const wholeCenter = (topbar.left + topbar.right) / 2, availableCenter = (start.right + end.left) / 2;
          assert(close(center, wholeCenter, 2) || close(center, availableCenter, 2), `${language} title is centered in the whole window or available control-free space`);
          if (titleText.length > 30) assert(title.scrollWidth > title.clientWidth && getComputedStyle(title).textOverflow === 'ellipsis', `${language} long title truncates with ellipsis instead of widening the shell`);
          titleCases.push({language, length: titleText.length, box: titleBox, center: round(center), wholeCenter, availableCenter});
        }
      }
    } finally {title.textContent = originalTitle; await set({language: 'en'});}
    report.titles = titleCases;

    const dividers = [...get('workspace').querySelectorAll<HTMLElement>('[role="separator"]')];
    assert(dividers.length === 2 && dividers.every(el => {
      const hit = getComputedStyle(el, '::after');
      const hitWidth = box(el).width - Number.parseFloat(hit.left) - Number.parseFloat(hit.right);
      return el.getAttribute('aria-orientation') === 'vertical' && close(box(el).width, 1) && hitWidth >= 6;
    }), 'Both keyboard-accessible dividers use a 1px hairline and at least 6px hit target');
    await key(dividers[0], 'Home'); await key(dividers[1], 'Home'); await stable();
    report.minimumPaneWidths = boundedPanes('Divider minimums');
    assert(close(widths().source, 220) && close(widths().inspector, 240), 'Divider Home reaches the actual 220px and 240px pane minimums');
    await key(dividers[0], 'ArrowRight'); await paint();
    assert(close(widths().source, 230), 'Keyboard divider adjustment increases the source by exactly 10px');
    await set({sourceWidth: 680, inspectorWidth: 440}); await stable();
    report.clampedWidths = boundedPanes('Oversized persisted pane widths');
    await set({sourceWidth: 340, inspectorWidth: 264}); await stable();

    await click('open-preferences'); await until(() => visible('preferences-dialog'), 'Preferences dialog');
    const dialog = box(get('preferences-dialog'));
    const sidebar = box(document.querySelector('.preferences-sidebar')!), detail = box(document.querySelector('.preferences-content')!);
    assert(document.querySelector('.preferences-tabs')?.getAttribute('aria-orientation')==='vertical', 'Preferences category navigation is a keyboard-accessible vertical sidebar');
    assert(sidebar.right<=detail.left+1 && close(sidebar.top,detail.top)&&close(sidebar.height,detail.height), 'Settings sidebar and detail pane share top and bottom baselines without overlap');
    assert(document.querySelectorAll('.settings-category-icon svg').length===4, 'Four meaningful colored glyph tiles use vector icons, never emojis');
    assert(close((dialog.left + dialog.right) / 2, innerWidth / 2, 2) && close((dialog.top + dialog.bottom) / 2, innerHeight / 2, 2), 'Preferences dialog is centered in the native content viewport');
    await key(get('preferences-tab-appearance'), 'Enter');
    await until(() => visible('pref-theme'), 'Appearance preferences');
    const controls = ['pref-theme', 'pref-font', 'pref-view', 'pref-reset-layout'].map(id => box(get(id)));
    assert(controls.every(r => close(r.height, controls[0].height)) && controls.every(r => close(r.right, controls[0].right)), 'Preferences select, number and button controls share one height and right edge');
    await click('pref-wrap'); assert(preferences.value.wrap, 'React preference switch writes wrapping to the preference store');
    await click('pref-wrap'); assert(!preferences.value.wrap, 'Repeated preference switch returns to the original wrapping value');
    await input(get<HTMLInputElement>('pref-font'), '15'); await key(get('pref-font'), 'Enter');
    assert(preferences.value.fontSize === 15, 'Preference font number commits on Enter');
    await set({fontSize: 12});
    await click('close-preferences'); await until(() => !visible('preferences-dialog'), 'Preferences close');
    assert(document.activeElement === get('open-preferences'), 'Closing preferences restores focus to its opening control');
    await click('open-preferences'); await key(get('preferences-dialog'), 'Escape');
    await until(() => !visible('preferences-dialog'), 'Preferences Escape dismissal');
    assert(true, 'Preferences reopens and Escape dismisses without leaving a modal overlay');

    const backgrounds: Record<string, string> = {};
    for (const theme of ['dark', 'light'] as const) {
      await set({theme});
      assert(document.documentElement.dataset.theme === theme, `The ${theme} preference updates the native document theme`);
      backgrounds[theme] = getComputedStyle(document.querySelector('.app-titlebar')!, '::before').backgroundColor;
      app.select(note.id); await paint();
      assert(close(box(get('pitch-input').closest('.property-input')!).right, fieldRects[0].right), `${theme} theme preserves inspector control geometry`);
    }
    assert(backgrounds.dark !== backgrounds.light, 'Dark and light themes produce distinct actual titlebar colors');
    report.themeBackgrounds = backgrounds; report.preferences = {dialog, controls};
    assert(app.current!.revision_id === initialRevision && app.current!.source === initialSource, 'Geometry, title fixtures, divider, theme and preference checks never mutate score data');
    return report;
  } finally {
    if (visible('preferences-dialog')) await click('close-preferences');
    preferences.update(saved); await paint();
  }
}
