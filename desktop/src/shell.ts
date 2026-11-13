import {inspectorMarkup} from './inspector/view';
import {mcpMarkup} from './mcp/view';
/** Presentation only. Score content and mutations remain owned by the Rust workspace. */
const icons = {
  open: '<path d="M3 7h6l2 2h10l-3 10H3V7Z"/><path d="M3 7V4h6l2 2h8v3"/>',
  save: '<path d="M5 3h12l3 3v15H4V3h1Z"/><path d="M8 3v6h8V3M8 21v-8h8v8"/>',
  undo: '<path d="m8 4-5 5 5 5M3 9h10a7 7 0 0 1 0 14"/>',
  redo: '<path d="m16 4 5 5-5 5m5-5H11a7 7 0 0 0 0 14"/>',
  check: '<path d="m4 12 5 5L20 6"/>',
  note: '<path d="M9 17V5l11-2v12M9 8l11-2"/><ellipse cx="6" cy="18" rx="3" ry="2"/><ellipse cx="17" cy="16" rx="3" ry="2"/>',
  chevron: '<path d="m8 4 8 8-8 8"/>',
  inspector: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7v.01"/>',
};
const icon = (name: keyof typeof icons) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${icons[name]}</svg>`;

export const shell = `
<header class="document-toolbar" aria-label="Document toolbar">
  <div class="toolbar-leading"><button id="open" class="icon-button" title="Open ScoreStream file" aria-label="Open ScoreStream file">${icon('open')}</button><button id="save" class="icon-button" title="Save as · Ctrl/⌘ S" aria-label="Save as">${icon('save')}</button><span class="divider"></span><div class="project-name"><span id="filename">First light.sfs</span><span id="dirty-dot" hidden aria-label="Uncommitted changes">●</span></div></div>
  <div class="view-switch" role="group" aria-label="Workspace view"><button id="view-split" aria-pressed="true" title="Source and score side by side">Split</button><button id="view-score" aria-pressed="false" title="Focus on the score">Score</button></div>
  <div class="toolbar-trailing"><button id="undo" class="icon-button" title="Undo committed edit" aria-label="Undo committed edit">${icon('undo')}</button><button id="redo" class="icon-button" title="Redo committed edit" aria-label="Redo committed edit">${icon('redo')}</button><span class="divider"></span><button id="discard" class="quiet" disabled>Discard</button><button id="apply" class="primary" disabled>Apply</button><span class="divider"></span><button id="toggle-inspector" class="icon-button" aria-label="Show inspector" aria-pressed="true" title="Show or hide inspector">${icon('inspector')}</button><button id="mcp-settings" class="quiet mcp-toolbar" title="Configure the local MCP service" aria-label="MCP service settings, stopped">MCP<span aria-hidden="true"></span></button><button id="about" class="icon-button" title="About this build" aria-label="About this build">${icon('info')}</button></div>
</header>
<main class="workspace" id="workspace">
  <section class="source-pane" aria-label="Score source">
    <header class="pane-heading"><h2>Source</h2><span class="file-tag">SFS 1.1</span><span id="source-state">Committed</span></header>
    <div id="editor"></div>
    <div class="source-footer"><span id="line-count">0 lines</span><span>UTF-8</span></div>
    <div id="diagnostics" aria-live="polite"><span class="valid-mark">✓</span> Ready</div>
  </section>
  <section class="preview-pane" aria-label="Score preview">
    <header class="pane-heading"><h2>Score</h2><span id="preview-kind">Committed score</span><span id="preview-status" class="status-pill pending">Preparing</span>
      <div class="canvas-tools" aria-label="Score view controls">
        <button id="toggle-title" aria-pressed="true" title="Show or hide the document title on the page">Title</button>
        <span class="divider"></span>
        <button id="zoom-out" aria-label="Zoom out" title="Zoom out">−</button><span id="zoom-level">100%</span><button id="zoom-in" aria-label="Zoom in" title="Zoom in">+</button><button id="fit" title="Fit score to pane width">Fit</button>
      </div>
    </header>
    <div id="stale-banner" hidden aria-live="polite"></div>
    <div id="projection-warning" hidden></div>
    <div id="canvas-scroll"><article id="paper" aria-label="Engraved score">
      <header class="score-title" id="score-heading"><h1 id="score-title">First light</h1></header>
      
      <div id="notation"><div class="empty-state"><span class="loading-ring"></span><h3>Preparing score</h3><p>Validating and engraving the source…</p></div></div>
    </article></div>
    <footer class="preview-footer"><span id="page-count">1 page</span><span id="render-time">Engraving</span></footer>
  </section>
  <aside class="inspector" id="inspector" aria-label="Inspector">${inspectorMarkup}</aside>
</main>
<footer class="statusbar"><span class="connection-status"><span class="live-dot"></span><span id="connection">Connecting</span><span id="revision" hidden></span></span><span id="global-message" role="status">Ready</span><span id="event-count">0 events</span></footer>
<dialog id="about-dialog" aria-labelledby="about-title"><button id="close-about" class="dialog-close" aria-label="Close">×</button><span class="preview-label">SCORESTREAM 0.1</span><h2 id="about-title">A workspace for notation.</h2><p>Rust validation, private live drafts, explicit commits, note property edits, session undo/redo, native SFS file open/save, and read-only MCP access to the active committed score.</p><p>Notation currently supports aligned measured staves, ordinary pitches, rests, equal-duration chords, and simple/dotted rhythms. Unsupported layouts are reported rather than silently approximated.</p><p><strong>Still ahead:</strong> MusicXML interchange, durable workspace history, MCP score editing, richer notation, playback, and native Windows/macOS verification.</p><p class="license-note">Verovio 5.6.0 · LGPL-3.0-or-later. Bravura · SIL OFL 1.1. See desktop/THIRD_PARTY_NOTICES.md and upstream source references.</p></dialog>${mcpMarkup}`;
