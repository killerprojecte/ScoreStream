import {useEffect, useRef} from 'react';
import {Minus, Plus, Maximize, FileMusic} from 'lucide-react';
import {Button} from './ui/button';
import {Tooltip,TooltipTrigger,TooltipContent} from './ui/tooltip';
import {Spinner} from './ui/spinner';
import {Alert,AlertDescription} from './ui/alert';
import {Empty,EmptyHeader,EmptyMedia,EmptyTitle,EmptyDescription} from './ui/empty';
import {engravingWidth, MIN_ZOOM, MAX_ZOOM} from '../layout';
import {sanitizeSvg} from '../render/sanitize';
import {readMetadata} from '../app/metadata';
import type {DocumentMetadata} from '../types';
import type {Workbench, WorkbenchState} from '../app/workbench';
import {t} from '../i18n';

/** Verovio owns only this SVG mount; page structure and metadata remain React-owned. */
function EngravedPage({svg}: {svg: string}) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {host.current!.innerHTML = sanitizeSvg(svg);}, [svg]);
  return <div className="notation-page" ref={host}/>;
}

export function ScoreHeading({metadata, visible}: {metadata: DocumentMetadata; visible: boolean}) {
  const hasHeading = metadata.title || metadata.subtitle || metadata.authors?.length;
  if (!visible || !hasHeading) return null;
  return <header id="score-heading" className="score-heading">
    {metadata.title && <h1 id="score-title">{metadata.title}</h1>}
    {metadata.subtitle && <p className="score-subtitle">{metadata.subtitle}</p>}
    {!!metadata.authors?.length && <p className="score-authors">{metadata.authors.join(' · ')}</p>}
  </header>;
}

export function ScorePane({controller, state, showTitle}: {controller: Workbench; state: WorkbenchState; showTitle: boolean}) {
  const canvas = useRef<HTMLDivElement>(null), notation = useRef<HTMLDivElement>(null);
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout>;
    const observer = new ResizeObserver(() => {
      clearTimeout(timer);
      timer = setTimeout(() => {if (canvas.current) controller.setWidth(engravingWidth(canvas.current.clientWidth, 40, 40));}, 120);
    });
    observer.observe(canvas.current!);
    return () => {clearTimeout(timer); observer.disconnect();};
  }, [controller]);
  useEffect(() => {
    const host = notation.current!;
    host.querySelectorAll('.selected-event').forEach(el => el.classList.remove('selected-event'));
    for (const [svgId, eventId] of Object.entries(state.renderedMap)) {
      const node = host.querySelector('[id="' + CSS.escape(svgId) + '"]'), event = state.published?.events.find(e => e.id === eventId);
      if (!node || !event) continue;
      node.setAttribute('tabindex', '0');
      node.setAttribute('role', 'button');
      node.setAttribute('aria-label', t('{pitch}, duration {duration} quarter-note units. Select in inspector.', {pitch: t(event.pitch === 'r' ? 'Rest' : event.pitch === '_' ? 'Spacer' : event.pitch), duration: event.duration}));
      if (state.selected?.id === eventId) node.classList.add('selected-event');
    }
  }, [state.renderedMap, state.published, state.selected, state.pages, t('Note')]);
  const stale = state.phase !== 'ready';
  const status = {loading: 'Preparing', pending: 'Updating', rendering: 'Engraving', invalid: 'Out of date', unsupported: 'Limited preview', ready: controller.dirty ? 'Live draft' : 'In sync'}[state.phase];
  const metadata = readMetadata(state.published?.document.metadata);
  const select = (target: Element | null) => {
    while (target && target !== notation.current) {
      if (target.id && state.renderedMap[target.id]) {controller.select(state.renderedMap[target.id], true); return;}
      target = target.parentElement;
    }
    controller.select('');
  };
  return <section className="score-pane pane glass glass-plane" aria-label={t('Score')}>
    <header className="pane-heading">
      <h2>{t('Score')}</h2>
      <span id="preview-status" hidden={state.phase === 'ready' && !controller.dirty} className={'preview-state ' + (stale ? 'pending' : '')} title={state.pages.length ? Math.round(state.elapsed) + ' ms · Verovio' : undefined}>
        {(state.phase === 'rendering' || state.phase === 'pending') && <Spinner/>}{t(status)}
      </span>
      <div className="pane-actions zoom-controls" role="group" aria-label={t('Score view controls')}>
        <Tooltip><TooltipTrigger asChild><Button id="zoom-out" variant="ghost" size="icon-sm" aria-label={t('Zoom out')} disabled={state.zoom <= MIN_ZOOM} onClick={() => controller.setZoom(state.zoom - .1)}><Minus/></Button></TooltipTrigger><TooltipContent>{t('Zoom out')}</TooltipContent></Tooltip>
        <span id="zoom-level">{Math.round(state.zoom * 100)}%</span>
        <Tooltip><TooltipTrigger asChild><Button id="zoom-in" variant="ghost" size="icon-sm" aria-label={t('Zoom in')} disabled={state.zoom >= MAX_ZOOM} onClick={() => controller.setZoom(state.zoom + .1)}><Plus/></Button></TooltipTrigger><TooltipContent>{t('Zoom in')}</TooltipContent></Tooltip>
        <Tooltip><TooltipTrigger asChild><Button id="fit" variant="ghost" size="icon-sm" aria-label={t('Fit to width')} onClick={() => controller.setZoom(1)}><Maximize/></Button></TooltipTrigger><TooltipContent>{t('Fit to width')}</TooltipContent></Tooltip>
      </div>
    </header>
    {stale && state.published && <Alert id="stale-banner" role="status"><AlertDescription>{t(state.phase === 'invalid' ? 'Source needs attention. Showing the last valid preview.' : state.phase === 'unsupported' ? 'This score exceeds the current preview scope. Last good notation retained.' : 'Updating the preview. The score below shows the previous snapshot.')}</AlertDescription></Alert>}
    {!!state.warnings.length && <Alert id="projection-warning"><AlertDescription>{state.warnings.join(' ')}</AlertDescription></Alert>}
    <div id="canvas-scroll" className="canvas-scroll" ref={canvas}>
      <div id="paper" className={'score-pages ' + (stale && state.published ? 'stale' : '')} style={{width: 'calc(' + state.zoom * 100 + '% - ' + 40 * (state.zoom - 1) + 'px)'}} onClick={event => {if (!(event.target as Element).closest('[role="button"]')) controller.select('');}}>
        <div id="notation" ref={notation} onClick={event => select(event.target as Element)} onKeyDown={event => {
          if (event.key === 'Enter' || event.key === ' ') {event.preventDefault(); select(event.target as Element);}
          else if (event.key === 'Escape') controller.select('');
        }}>
          {state.pages.map((svg, index) => <article className="score-paper glass glass--reading" key={index} aria-label={t('Page {number}', {number: index + 1})}>
            {metadata.header && <div className="score-running-header">{metadata.header}</div>}
            {index === 0 && <ScoreHeading metadata={metadata} visible={showTitle}/>}
            <EngravedPage svg={svg}/>
            {metadata.footer && <div className="score-running-footer">{metadata.footer}</div>}
          </article>)}
          {!state.pages.length && <article className="score-paper glass glass--reading"><Empty><EmptyHeader><EmptyMedia variant="icon"><FileMusic/></EmptyMedia><EmptyTitle>{t(state.phase === 'unsupported' ? 'Preview unavailable' : 'Preparing')}</EmptyTitle>{state.phase === 'unsupported' && <EmptyDescription id="preview-error-text">{t(state.message)}</EmptyDescription>}</EmptyHeader></Empty></article>}
        </div>
      </div>
    </div>
  </section>;
}
