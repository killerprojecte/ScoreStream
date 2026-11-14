import {Dialog,DialogContent,DialogHeader,DialogTitle,DialogDescription,DialogFooter} from './ui/dialog';
import {Button} from './ui/button';
import {Alert,AlertDescription} from './ui/alert';
import {Spinner} from './ui/spinner';
import type {WavExportController,WavExportState} from '../app/wav-export';
import {t} from '../i18n';

const phaseLabel={
 idle:'',choosing:'Choose a new WAV filename in the native save dialog.',
 rendering:'Rendering WAV audio…',writing:'Writing WAV audio…',finishing:'Finishing WAV export…',
 cancelling:'Cancelling WAV export…',succeeded:'',failed:'WAV export failed.',
};
export function WavExportDialog({controller,state}:{controller:WavExportController;state:WavExportState}){
 const busy=controller.busy,canCancel=state.phase==='rendering'||state.phase==='writing';
 return <Dialog open={state.phase!=='idle'} onOpenChange={open=>{if(!open&&!busy)controller.dismiss();}}>
  <DialogContent id="wav-export-dialog" className="glass glass--popup glass-plane glass-dialog" showCloseButton={false} onInteractOutside={event=>event.preventDefault()} onEscapeKeyDown={event=>{if(busy){event.preventDefault();if(canCancel)controller.cancel();}}}>
   <DialogHeader><DialogTitle>{t('Export WAV')}</DialogTitle><DialogDescription>{t('Exports the source captured when you choose Export WAV, without applying or saving your score. Built-in synthesized instruments only: 48 kHz stereo PCM16, up to 120 seconds.')}</DialogDescription></DialogHeader>
   {busy&&<div className="flex items-center gap-2 text-sm" role="status"><Spinner aria-hidden="true"/>{t(phaseLabel[state.phase])}</div>}
   {state.phase==='writing'&&<p className="text-sm tabular-nums" aria-live="off">{t('{percent}% written',{percent:Math.floor(state.writtenBytes/state.totalBytes*100)})}</p>}
   {state.phase==='succeeded'&&<Alert id="wav-export-success" role="status"><AlertDescription>{t('Exported {name}.',{name:state.fileName})}</AlertDescription></Alert>}
   {state.error&&<Alert id="wav-export-error" variant="destructive"><AlertDescription>{t(state.error)}</AlertDescription></Alert>}
   <DialogFooter>{busy?<Button id="cancel-wav-export" variant="outline" disabled={!canCancel} onClick={()=>controller.cancel()}>{t('Cancel')}</Button>:<Button id="close-wav-export" onClick={()=>controller.dismiss()}>{t('Close')}</Button>}</DialogFooter>
  </DialogContent>
 </Dialog>;
}
