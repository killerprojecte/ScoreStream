import type {ScoreEvent,View} from '../types';
import {noteLocation,performanceTempo} from './format';
const check='<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="m3 8 3 3 7-7"/></svg>';
export const inspectorMarkup=`
<header class="pane-heading inspector-header"><h2 id="inspector-title">Document</h2><button id="jump" class="source-link" title="Reveal selected note in source" hidden>Source ↗</button></header>
<div class="inspector-content">
  <div id="selection-empty">
    <div class="document-overview"><strong id="document-title">First light</strong><span id="document-summary"></span></div>
    <section class="inspector-group"><h3>Score</h3><div id="parts"></div><dl class="property-list"><dt>Measures</dt><dd id="document-measures"></dd><dt>Events</dt><dd id="document-events"></dd></dl></section>
    <section class="inspector-group" id="performance-group"><h3>Performance tempo</h3><dl class="property-list"><dt>Rate</dt><dd><span id="performance-tempo"></span> bpm</dd><dt>Beat unit</dt><dd id="tempo-unit"></dd></dl></section>
    <p class="selection-help">Select a note in the score or source to edit it.</p>
  </div>
  <div id="selection" hidden>
    <section class="inspector-group note-properties" aria-label="Note properties">
      <div class="property-row"><label for="pitch-input">Pitch</label><div class="property-input"><input id="pitch-input" aria-label="Written pitch" maxlength="30" spellcheck="false" autocomplete="off"/><button id="set-pitch" aria-label="Apply pitch" title="Apply pitch · Enter">${check}</button></div></div>
      <div class="property-row"><label for="duration-input">Duration</label><div class="property-input"><input id="duration-input" aria-label="Duration in quarter units" aria-describedby="duration-unit" maxlength="30" spellcheck="false" autocomplete="off"/><span class="field-unit" id="duration-unit" title="Quarter-note units">q</span><button id="set-duration" aria-label="Apply duration" title="Apply duration · Enter">${check}</button></div></div>
      <p id="inspector-feedback" class="field-feedback" role="alert" hidden></p>
      <p id="mutation-hint" class="field-help" hidden></p>
    </section>
    <section class="inspector-group"><h3>Location</h3><dl class="property-list"><dt>Part</dt><dd id="event-part"></dd><dt>Measure</dt><dd id="event-measure"></dd><dt>Beat</dt><dd id="event-beat"></dd><dt>Staff</dt><dd id="event-staff"></dd><dt>Voice</dt><dd id="event-voice"></dd></dl></section>
    <details class="inspector-group source-details"><summary>Source identity</summary><dl class="property-list"><dt>Event</dt><dd id="event-id"></dd><dt>Line</dt><dd id="event-line"></dd><dt>Position</dt><dd id="event-time"></dd></dl></details>
  </div>
</div>`;

export class Inspector {
  private selected:ScoreEvent|null=null;
  private canEdit=false;
  private readonly get=<T extends HTMLElement=HTMLElement>(id:string)=>document.getElementById(id) as T;
  constructor(commit:(kind:'pitch'|'duration')=>void){
    for(const kind of ['pitch','duration'] as const){
      const input=this.get<HTMLInputElement>(`${kind}-input`);
      input.addEventListener('input',()=>{input.removeAttribute('aria-invalid');this.get('inspector-feedback').hidden=true;this.updateButtons();});
      input.addEventListener('keydown',event=>{
        if(event.key==='Enter'){event.preventDefault();if(this.canEdit&&this.changed(kind))commit(kind);}
        if(event.key==='Escape'){event.preventDefault();if(this.selected)input.value=this.selected[kind];input.removeAttribute('aria-invalid');this.get('inspector-feedback').hidden=true;this.updateButtons();}
      });
    }
  }
  private changed(kind:'pitch'|'duration'){return !!this.selected&&this.get<HTMLInputElement>(`${kind}-input`).value!==this.selected[kind];}
  private updateButtons(){for(const kind of ['pitch','duration'] as const)this.get<HTMLButtonElement>(`set-${kind}`).disabled=!this.canEdit||!this.changed(kind);}
  acceptProperty(kind:'pitch'|'duration',value:string){const input=this.get<HTMLInputElement>(`${kind}-input`);input.value=value;input.removeAttribute('aria-invalid');this.get('inspector-feedback').hidden=true;}
  showError(kind:'pitch'|'duration',message:string){this.get(`${kind}-input`).setAttribute('aria-invalid','true');const feedback=this.get('inspector-feedback');feedback.textContent=message;feedback.hidden=false;}
  updateDocument(view:View,fileName:string){
    this.get('document-title').textContent=fileName.replace(/\.sfs$/i,'').replace(/[-_]/g,' ');
    this.get('document-summary').textContent='ScoreStream document';
    this.get('document-measures').textContent=String(new Set(view.measures.map(m=>m.time)).size);
    this.get('document-events').textContent=String(view.events.length);
    this.get('parts').replaceChildren(...view.parts.map(part=>{const row=document.createElement('div');row.className='part-property';const label=document.createElement('span');label.textContent=part.instrument;const value=document.createElement('span');value.textContent=`${part.staves} ${part.staves===1?'staff':'staves'}`;row.append(label,value);return row;}));
    const tempo=performanceTempo(view);this.get('performance-group').hidden=!tempo;
    if(tempo){this.get('performance-tempo').textContent=tempo.rate;this.get('tempo-unit').textContent=tempo.unit;}
  }
  updateSelection(event:ScoreEvent|null,view:View|null,canEdit:boolean,dirty:boolean){
    const changed=this.selected?.id!==event?.id;
    const staged={pitch:this.changed('pitch'),duration:this.changed('duration')};
    this.selected=event;this.canEdit=canEdit;
    this.get('selection').hidden=!event;this.get('selection-empty').hidden=!!event;this.get('jump').hidden=!event;
    this.get('inspector-title').textContent=event?(event.pitch==='r'?'Rest':event.pitch==='_'?'Spacer':'Note'):'Document';
    if(changed)this.get('inspector-feedback').hidden=true;
    if(!event||!view)return;
    const location=noteLocation(event,view);
    for(const key of ['part','measure','beat','staff','voice','line'] as const)this.get(`event-${key}`).textContent=location[key];
    this.get('event-id').textContent=event.id;this.get('event-time').textContent=`${event.time} q`;
    for(const kind of ['pitch','duration'] as const){const input=this.get<HTMLInputElement>(`${kind}-input`);if(changed||!staged[kind])input.value=event[kind];input.disabled=!canEdit;if(changed)input.removeAttribute('aria-invalid');}
    const hint=this.get('mutation-hint');hint.hidden=canEdit;hint.textContent=dirty?'Apply or discard the source draft to edit this note.':'Waiting for the current score preview.';
    this.updateButtons();
  }
}
