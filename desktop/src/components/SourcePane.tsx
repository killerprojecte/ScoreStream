import {useEffect,useRef} from 'react';
import {Undo2,Redo2} from 'lucide-react';
import {Button} from './ui/button';
import {Tooltip,TooltipTrigger,TooltipContent} from './ui/tooltip';
import {SourceEditor} from '../editor/source';
import type {Workbench,WorkbenchState} from '../app/workbench';
import type {Preferences} from '../preferences-model';
import {t} from '../i18n';
export function SourcePane({controller,state,preferences,theme}:{controller:Workbench;state:WorkbenchState;preferences:Preferences;theme:'light'|'dark'}){
 const host=useRef<HTMLDivElement>(null);
 useEffect(()=>{const editor=new SourceEditor(host.current!,()=>controller.changed(),line=>controller.selectSource(line));const detach=controller.attachEditor(editor);return()=>{detach();editor.dispose();};},[controller]);
 useEffect(()=>{controller.editor?.configure(preferences.fontSize,preferences.wrap,theme);},[controller,preferences.fontSize,preferences.wrap,theme]);
 const diagnostics=state.view?.diagnostics??state.current?.diagnostics??[];
 return <section className="source-pane pane glass glass-plane" aria-label={t('Source')}>
  <header className="pane-heading"><h2>{t('Source')}</h2><span id="source-state" hidden={!controller.dirty} className={controller.dirty?'draft-label':'pane-detail'}>{t(controller.dirty?'Draft':'Committed')}</span><div className="pane-actions"><Tooltip><TooltipTrigger asChild><Button id="undo" variant="ghost" size="icon-sm" aria-label={t('Undo')} disabled={state.busy||controller.dirty||!state.current?.can_undo} onClick={()=>void controller.history('undo')}><Undo2/></Button></TooltipTrigger><TooltipContent>{t('Undo')}</TooltipContent></Tooltip><Tooltip><TooltipTrigger asChild><Button id="redo" variant="ghost" size="icon-sm" aria-label={t('Redo')} disabled={state.busy||controller.dirty||!state.current?.can_redo} onClick={()=>void controller.history('redo')}><Redo2/></Button></TooltipTrigger><TooltipContent>{t('Redo')}</TooltipContent></Tooltip></div></header>
  <div id="editor" className="editor-host" ref={host}/>
  <div id="diagnostics" className="diagnostics" role="status" hidden={!diagnostics.length}>{diagnostics.length?diagnostics.slice(0,3).map((d,i)=><Button key={i} variant="ghost" size="sm" className="justify-start max-w-full truncate text-destructive" onClick={()=>controller.editor?.selectLine(d.line)}>L{d.line} · {d.message}</Button>):null}</div>
  <footer className="source-footer"><span id="line-count">{t('{count} lines',{count:state.source.split('\n').length})}</span><div hidden={!controller.dirty}><Button id="discard" variant="ghost" size="sm" disabled={state.busy||!controller.dirty} onClick={()=>void controller.discard()}>{t('Discard')}</Button><Button id="apply" size="sm" disabled={!controller.dirty||!controller.canSave} onClick={()=>void controller.apply()}>{t('Apply draft')}</Button></div></footer>
 </section>;
}
