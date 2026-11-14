import {useEffect,useMemo,useRef,useState,useSyncExternalStore} from 'react';
import {Pause,Play,Square} from 'lucide-react';
import {Button} from './ui/button';
import {Input} from './ui/input';
import {Label} from './ui/label';
import {Select,SelectContent,SelectItem,SelectTrigger,SelectValue} from './ui/select';
import {Tooltip,TooltipContent,TooltipTrigger} from './ui/tooltip';
import {Alert,AlertDescription} from './ui/alert';
import {Spinner} from './ui/spinner';
import {displayMeasures,formatTime,measureAt,parseSeconds} from '../playback/timeline';
import type {PlaybackTransport} from '../playback/transport';
import {t} from '../i18n';

function SecondsField({position,duration,onSeek}:{position:number;duration:number;onSeek:(seconds:number)=>void}){
 const skipBlur=useRef(false);
 const [draft,setDraft]=useState(position.toFixed(2)),[editing,setEditing]=useState(false),[invalid,setInvalid]=useState(false);
 useEffect(()=>{if(!editing)setDraft(position.toFixed(2));},[position,editing]);
 const commit=()=>{const seconds=parseSeconds(draft,duration);setInvalid(seconds===null);if(seconds!==null){onSeek(seconds);setEditing(false);}};
 return <Input id="playback-seconds" className="w-20 tabular-nums" inputMode="decimal" value={draft} aria-invalid={invalid||undefined} aria-describedby={invalid?'playback-seconds-error':undefined} onFocus={()=>setEditing(true)} onChange={event=>{setEditing(true);setDraft(event.target.value);setInvalid(false);}} onBlur={()=>{if(skipBlur.current){skipBlur.current=false;return;}commit();}} onKeyDown={event=>{if(event.key==='Enter'){event.preventDefault();commit();}else if(event.key==='Escape'){skipBlur.current=true;setDraft(position.toFixed(2));setInvalid(false);setEditing(false);event.currentTarget.blur();}}}/>;
}
export function PlaybackControls({transport,enabled}:{transport:PlaybackTransport;enabled:boolean}){
 const state=useSyncExternalStore(transport.subscribe,transport.getSnapshot);
 const plan=state.plan,duration=plan?.duration_seconds??0,active=state.status==='playing'||state.status==='loading';
 const measures=useMemo(()=>displayMeasures(plan),[plan]),measure=measureAt(measures,state.position);
 return <div className="playback-panel border-b px-3 py-2">
  <div className="flex flex-wrap items-center gap-2" role="group" aria-label={t('Playback controls')}>
   <Tooltip><TooltipTrigger asChild><Button id="playback-toggle" size="icon-sm" variant="outline" disabled={!enabled} aria-label={t(active?'Pause':'Play')} aria-keyshortcuts="Space" onClick={()=>transport.toggle()}>{state.status==='loading'?<Spinner/>:active?<Pause/>:<Play/>}</Button></TooltipTrigger><TooltipContent>{t(active?'Pause · Space':'Play · Space')}</TooltipContent></Tooltip>
   <Tooltip><TooltipTrigger asChild><Button id="playback-stop" size="icon-sm" variant="ghost" disabled={!enabled} aria-label={t('Stop playback')} onClick={()=>transport.stop()}><Square/></Button></TooltipTrigger><TooltipContent>{t('Stop playback')}</TooltipContent></Tooltip>
   <output id="playback-time" className="text-xs tabular-nums" aria-label={t('Playback time')}>{formatTime(state.position)} / {formatTime(duration)}</output>
   {plan&&<>
    <Label htmlFor="playback-seconds" className="text-xs">{t('Seconds')}</Label><SecondsField position={state.position} duration={duration} onSeek={seconds=>transport.seek(seconds)}/>
    {!!measures.length&&<><Label htmlFor="playback-measure" className="text-xs">{t('Measure')}</Label><Select value={measure?.id??''} onValueChange={id=>{const target=measures.find(item=>item.id===id);if(target)transport.seek(target.start_seconds);}}><SelectTrigger id="playback-measure" size="sm" className="w-24"><SelectValue placeholder="—"/></SelectTrigger><SelectContent>{measures.map((item,index)=><SelectItem key={item.id} value={item.id}>{item.label??String(index+1)}</SelectItem>)}</SelectContent></Select></>}
   </>}
   <span className="ml-auto text-xs text-muted-foreground">{t('Built-in synth')}</span>
  </div>
  {plan&&<progress className="mt-2 block h-1 w-full accent-primary" max={duration||1} value={state.position} aria-label={t('Playback progress')}/>}
  <span id="playback-seconds-error" className="sr-only">{t('Enter seconds between 0 and {duration}.',{duration:duration.toFixed(2)})}</span>
  {state.error&&<Alert id="playback-error" variant="destructive" className="mt-2"><AlertDescription>{t(state.error)}</AlertDescription></Alert>}
  {!!plan?.warnings.length&&<Alert id="playback-warnings" className="mt-2"><AlertDescription>{plan.warnings.join(' ')}</AlertDescription></Alert>}
 </div>;
}
