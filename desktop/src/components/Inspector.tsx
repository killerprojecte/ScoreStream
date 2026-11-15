import {useEffect, useRef, useState, type KeyboardEvent} from 'react';
import {Check, ChevronDown, Code2} from 'lucide-react';
import {Button} from '@/components/ui/button';
import {Spinner} from '@/components/ui/spinner';
import {InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput, InputGroupText} from '@/components/ui/input-group';
import {Alert, AlertDescription} from '@/components/ui/alert';
import {Tooltip, TooltipContent, TooltipProvider, TooltipTrigger} from '@/components/ui/tooltip';
import {Label} from '@/components/ui/label';
import {Collapsible, CollapsibleContent, CollapsibleTrigger} from '@/components/ui/collapsible';
import {t} from '../i18n';
import {readMetadata} from '../app/metadata';
import {noteLocation, performanceTempo} from '../inspector/format';
import type {ScoreEvent, View} from '../types';
import './inspector.css';

export type PropertyKind = 'pitch' | 'duration';
export interface InspectorProps {
  view: View | null;
  selected: ScoreEvent | null;
  fileName: string;
  canEdit: boolean;
  dirty: boolean;
  onCommit: (kind: PropertyKind, value: string) => Promise<void>;
  onReveal: () => void;
}

function useStagedProperty(value: string) {
  const [draft, setDraft] = useState(value);
  const previous = useRef(value);
  useEffect(() => {
    const baseline = previous.current;
    previous.current = value;
    // A new committed snapshot must not destroy a different, unapplied field.
    setDraft(current => current === baseline ? value : current);
  }, [value]);
  return [draft, setDraft] as const;
}

function NoteProperties({selected, canEdit, dirty, onCommit}: Pick<InspectorProps, 'canEdit' | 'dirty' | 'onCommit'> & {selected: ScoreEvent}) {
  const [pitch, setPitch] = useStagedProperty(selected.pitch);
  const [duration, setDuration] = useStagedProperty(selected.duration);
  const [pending, setPending] = useState<PropertyKind | null>(null);
  const [error, setError] = useState<{kind: PropertyKind; message: string} | null>(null);
  const [accepted, setAccepted] = useState<{kind: PropertyKind; submitted: string} | null>(null);
  const active = useRef(true);
  const submitting = useRef(false);
  const focusAfterCommit = useRef<PropertyKind | null>(null);
  const inputs = useRef<Partial<Record<PropertyKind, HTMLInputElement | null>>>({});
  useEffect(() => { active.current = true; return () => { active.current = false; }; }, []);
  const fields = {pitch, duration};
  const setters = {pitch: setPitch, duration: setDuration};
  const editable = canEdit && pending === null;
  useEffect(() => {
    const cancelForNewTarget = (event: Event) => {
      const kind = focusAfterCommit.current;
      const target = event.target;
      if (!kind || !(target instanceof Element) || target === document.body) return;
      if (!inputs.current[kind]?.closest('[data-slot="input-group"]')?.contains(target)) focusAfterCommit.current = null;
    };
    document.addEventListener('focusin', cancelForNewTarget);
    document.addEventListener('pointerdown', cancelForNewTarget);
    return () => {
      document.removeEventListener('focusin', cancelForNewTarget);
      document.removeEventListener('pointerdown', cancelForNewTarget);
    };
  }, []);
  useEffect(() => {
    const kind = focusAfterCommit.current;
    if (!editable || !kind) return;
    const field = inputs.current[kind];
    const activeElement = document.activeElement;
    if (activeElement === document.body || !activeElement || field?.closest('[data-slot="input-group"]')?.contains(activeElement)) field?.focus();
    focusAfterCommit.current = null;
  }, [editable]);
  useEffect(() => {
    if (!accepted) return;
    const setField = accepted.kind === 'pitch' ? setPitch : setDuration;
    // The parent publishes the authoritative snapshot before resolving commit.
    // Accept its normalized value without replacing a separately staged field.
    setField(current => current === accepted.submitted ? selected[accepted.kind] : current);
    setAccepted(null);
  }, [accepted, selected.pitch, selected.duration, setPitch, setDuration]);

  async function commit(kind: PropertyKind) {
    if (!canEdit || submitting.current || fields[kind] === selected[kind]) return;
    const value = fields[kind];
    if (inputs.current[kind]?.closest('[data-slot="input-group"]')?.contains(document.activeElement)) focusAfterCommit.current = kind;
    submitting.current = true;
    setPending(kind);
    setError(null);
    try {
      await onCommit(kind, value);
      if (active.current) setAccepted({kind, submitted: value});
    } catch (reason) {
      if (active.current) setError({kind, message: reason instanceof Error ? reason.message : String(reason)});
    } finally {
      submitting.current = false;
      if (active.current) setPending(null);
    }
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>, kind: PropertyKind) {
    if (event.key === 'Enter' && !event.nativeEvent.isComposing) {
      event.preventDefault();
      void commit(kind);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      setters[kind](selected[kind]);
      setError(null);
    }
  }

  return <section className="inspector-group note-properties" aria-label={t('Note properties')} aria-busy={pending !== null}>
    {(['pitch', 'duration'] as const).map(kind => <div className="property-row" key={kind}>
      <Label htmlFor={`${kind}-input`}>{t(kind === 'pitch' ? 'Pitch' : 'Duration')}</Label>
      <InputGroup data-disabled={!editable || undefined}>
        <InputGroupInput id={`${kind}-input`} ref={element => {inputs.current[kind] = element;}} value={fields[kind]} disabled={!editable}
          aria-label={t(kind === 'pitch' ? 'Written pitch' : 'Duration in quarter units')}
          aria-invalid={error?.kind === kind || undefined}
          aria-describedby={[kind === 'duration' ? 'duration-unit' : '', error?.kind === kind ? 'inspector-feedback' : '', !canEdit ? 'mutation-hint' : ''].filter(Boolean).join(' ') || undefined}
          maxLength={30} spellCheck={false} autoComplete="off" autoCapitalize="off"
          onChange={event => { setters[kind](event.currentTarget.value); setError(null); }}
          onKeyDown={event => onKeyDown(event, kind)} />
        <InputGroupAddon align="inline-end">
          {kind === 'duration' && <Tooltip>
            <TooltipTrigger asChild><InputGroupText id="duration-unit" tabIndex={0} aria-label={t('Quarter-note units')}>q</InputGroupText></TooltipTrigger>
            <TooltipContent>{t('Quarter-note units')}</TooltipContent>
          </Tooltip>}
          <Tooltip>
            <TooltipTrigger asChild>
              <InputGroupButton id={`set-${kind}`} size="icon-xs"
                disabled={!editable || fields[kind] === selected[kind]}
                aria-label={t(kind === 'pitch' ? 'Apply pitch' : 'Apply duration')}
                onClick={() => { void commit(kind); }}>
                {pending === kind ? <Spinner aria-hidden="true" /> : <Check aria-hidden="true" />}
              </InputGroupButton>
            </TooltipTrigger>
            <TooltipContent>{t(kind === 'pitch' ? 'Apply pitch · Enter' : 'Apply duration · Enter')}</TooltipContent>
          </Tooltip>
        </InputGroupAddon>
      </InputGroup>
    </div>)}
    {error && <Alert variant="destructive" id="inspector-feedback"><AlertDescription>{t(error.message)}</AlertDescription></Alert>}
    {!canEdit && <p id="mutation-hint" className="field-help">{t(dirty ? 'Apply or discard the source draft to edit this note.' : 'Waiting for the current score preview.')}</p>}
  </section>;
}

function SelectedInspector({view, selected, ...props}: Omit<InspectorProps, 'selected' | 'view'> & {selected: ScoreEvent; view: View}) {
  const location = noteLocation(selected, view);
  return <div id="selection">
    <NoteProperties key={`properties:${selected.id}`} selected={selected} {...props} />
    <section className="inspector-group">
      <h3>{t('Location')}</h3>
      <dl className="property-list">
        {(['part', 'measure', 'beat', 'staff', 'voice'] as const).map(key => <div className="property-entry" key={key}>
          <dt>{t(key[0].toUpperCase() + key.slice(1))}</dt><dd id={`event-${key}`}>{location[key]}</dd>
        </div>)}
      </dl>
    </section>
    <Collapsible className="inspector-group source-details" key={`identity:${selected.id}`}>
      <CollapsibleTrigger asChild>
        <Button variant="ghost" className="source-details-trigger w-full justify-between">
          <span>{t('Source identity')}</span><ChevronDown aria-hidden="true" />
        </Button>
      </CollapsibleTrigger>
      <CollapsibleContent>
        <dl className="property-list source-identity-list">
          <div className="property-entry"><dt>{t('Event')}</dt><dd id="event-id">{selected.id}</dd></div>
          <div className="property-entry"><dt>{t('Line')}</dt><dd id="event-line">{location.line}</dd></div>
          <div className="property-entry"><dt>{t('Position')}</dt><dd id="event-time">{selected.time} q</dd></div>
        </dl>
      </CollapsibleContent>
    </Collapsible>
  </div>;
}

function DocumentInspector({view}: Pick<InspectorProps, 'view'>) {
  const tempo = view ? performanceTempo(view) : null;
  const metadata = readMetadata(view?.document.metadata);
  const hasMetadata = !!(metadata.title || metadata.description || metadata.authors?.length);
  return <div id="selection-empty">
    {hasMetadata && <section className="document-overview" aria-label={t('Document')}>
      {metadata.title && <strong id="document-title">{metadata.title}</strong>}
      {metadata.description && <p id="document-description">{metadata.description}</p>}
      {!!metadata.authors?.length && <div className="document-authors"><span>{t('Authors')}</span><p id="document-authors">{metadata.authors.join(', ')}</p></div>}
    </section>}
    {view && <section className="inspector-group document-facts">
      <h3>{t('Score')}</h3>
      <dl className="property-list" id="parts">
        {view.parts.map(part => <div className="property-entry" key={part.id}><dt>{part.display_name ?? part.instrument}</dt><dd>{t(part.staves === 1 ? '{count} staff' : '{count} staves', {count: part.staves})}</dd></div>)}
        <div className="property-entry"><dt>{t('Measures')}</dt><dd id="document-measures">{new Set(view.measures.map(measure => measure.time)).size}</dd></div>
        <div className="property-entry"><dt>{t('Events')}</dt><dd id="document-events">{view.events.length}</dd></div>
        {tempo && <>
          <div className="property-entry" id="performance-group"><dt>{t('Rate')}</dt><dd><span id="performance-tempo">{tempo.rate}</span> bpm</dd></div>
          <div className="property-entry"><dt>{t('Beat unit')}</dt><dd id="tempo-unit">{t(tempo.unit)}</dd></div>
        </>}
      </dl>
    </section>}
    <p className="selection-help">{t('Select a note in the score or source to edit it.')}</p>
  </div>;
}

export function Inspector(props: InspectorProps) {
  const {selected, view, onReveal} = props;
  const hasSelection = selected !== null && view !== null;
  const title = hasSelection ? selected.pitch === 'r' ? 'Rest' : selected.pitch === '_' ? 'Spacer' : 'Note' : 'Document';
  return <TooltipProvider>
    <header className="pane-heading inspector-header">
      <h2 id="inspector-title">{t(title)}</h2>
      {hasSelection && <Tooltip>
        <TooltipTrigger asChild><Button id="jump" variant="ghost" size="sm" onClick={onReveal}>
          <Code2 aria-hidden="true" /><span>{t('Source')}</span>
        </Button></TooltipTrigger>
        <TooltipContent>{t('Reveal selected note in source')}</TooltipContent>
      </Tooltip>}
    </header>
    <div className="inspector-content">
      {hasSelection ? <SelectedInspector {...props} selected={selected} view={view} /> : <DocumentInspector view={view} />}
    </div>
  </TooltipProvider>;
}
