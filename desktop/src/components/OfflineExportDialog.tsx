import {useEffect,useMemo,useState,useSyncExternalStore} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {native} from '../bridge';
import {OfflineExport} from '../app/offline-export';
import type {Workbench} from '../app/workbench';
import {Dialog,DialogContent,DialogHeader,DialogTitle,DialogDescription,DialogFooter} from './ui/dialog';
import {Button} from './ui/button';
import {Textarea} from './ui/textarea';
import {t} from '../i18n';

export function useOfflineExport(workbench:Workbench){
 const model=useMemo(()=>new OfflineExport({
  reserve:expectedRevisionId=>invoke<string>('offline_export_reserve',{expectedRevisionId}),
  run:(id,action)=>action==='export'?invoke('offline_export_run',{id,mode:'disclosed_rehearsal_v1'}):invoke(action==='draft'?'offline_policy_draft':'offline_policy_validate',{id}),
  cancel:id=>invoke<void>('offline_export_cancel',{id}),
 }),[]);
 const [supported,setSupported]=useState(false);
 const state=useSyncExternalStore(model.subscribe,model.getSnapshot);
 useEffect(()=>{let live=true;if(native)void invoke<boolean>('offline_export_supported').then(v=>{if(live)setSupported(v);}).catch(()=>{});return()=>{live=false;model.cancel();};},[model]);
 useEffect(()=>{let previous=workbench.getSnapshot();const unsubscribe=workbench.subscribe(()=>{
  const next=workbench.getSnapshot();
  if(next.source!==previous.source||next.current?.revision_id!==previous.current?.revision_id)model.cancel();
  previous=next;
 });return()=>{unsubscribe();};},[workbench,model]);
 return {model,state,supported};
}
export function OfflineExportDialog({offline,revision,enabled}:{offline:ReturnType<typeof useOfflineExport>;revision:string;enabled:boolean}){
 const {model,state}=offline;
 return <Dialog open={state.phase!=='idle'} onOpenChange={open=>{if(!open)model.dismiss();}}>
  <DialogContent id="offline-export-dialog" className="glass glass--popup glass-plane glass-dialog" showCloseButton={false} onInteractOutside={e=>e.preventDefault()} onEscapeKeyDown={e=>{if(model.busy){e.preventDefault();model.cancel();}}}>
   <DialogHeader><DialogTitle>{t('Offline rehearsal WAV (Linux)')}</DialogTitle><DialogDescription>{t('Uses the current applied score and your explicitly selected local policy and TimGM6mb bank. Existing files are never replaced. Normal playback and Export WAV are unchanged.')}</DialogDescription></DialogHeader>
   <p className="text-sm">{t('This rehearsal does not realize wedge, dashes, ornaments or slur sound. It keeps per-note gain curves, uses declared local GM mixing and shared pizzicato samples, treats tied continuation dynamics as inert, and releases unmarked notes before selected holds. Native note timing is quantized to 64 frames. Hold and grace choices come from your source-bound policy. It is not a fully faithful performance.')}</p>
   {state.phase==='pending'&&<p role="status">{t(state.action==='export'?'Choose policy JSON, bank and a new WAV destination in the native dialogs. Then native verification and rendering run. Editing cancels this export.':state.action==='draft'?'Choose a new JSON filename for the incomplete source-bound draft. No sound or timing choices are filled in.':'Choose your edited policy JSON. Validation uses this score; it does not open a bank or create audio.')}</p>}
   {state.phase==='cancelling'&&<p role="status">{t('Cancelling offline export. Close any native chooser; waiting for the native result…')}</p>}
   {state.phase==='succeeded'&&<><p role="status">{t(state.action==='export'?'Verified WAV saved to your selected destination.':state.action==='draft'?'Incomplete policy draft saved. Fill its null fields explicitly, then validate it here.':'Policy validation finished. Check missing choices and errors below; a compiled policy is not native bank verification.')}</p>{state.action==='validate'&&<Textarea aria-label={t('Policy validation report')} className="w-full min-h-48 font-mono text-xs" rows={12} readOnly value={JSON.stringify(state.result,null,2)}/>}</>}
   {state.phase==='cancelled'&&<p role="status">{t('Offline export cancelled before publication.')}</p>}
   {state.phase==='failed'&&<p role="alert">{t('Offline export failed. Check the source-bound policy, trusted local bank and new destination. No successful publication was reported.')}</p>}
   {state.error&&<p className="text-sm" role="alert">{state.error}</p>}
   <DialogFooter className="flex-wrap">{state.phase==='confirm'?<><Button variant="outline" onClick={()=>model.dismiss()}>{t('Cancel')}</Button><Button variant="outline" disabled={!enabled} onClick={()=>void model.start(revision,'draft')}>{t('Generate policy draft…')}</Button><Button variant="outline" disabled={!enabled} onClick={()=>void model.start(revision,'validate')}>{t('Validate edited policy…')}</Button><Button id="start-offline-export" disabled={!enabled} onClick={()=>void model.start(revision)}>{t('Use this rehearsal policy and choose files')}</Button></>:model.busy?<Button id="cancel-offline-export" disabled={state.phase==='cancelling'} onClick={()=>model.cancel()}>{t('Cancel')}</Button>:<Button onClick={()=>model.dismiss()}>{t('Close')}</Button>}</DialogFooter>
  </DialogContent>
 </Dialog>;
}
