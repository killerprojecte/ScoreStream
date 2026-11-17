import {useOfflineExport,OfflineExportDialog} from './components/OfflineExportDialog';
import {isPlaybackShortcut} from './playback/transport';
import {Tabs,TabsList,TabsTrigger,TabsContent} from '@/components/ui/tabs';
import {Toggle} from '@/components/ui/toggle';
import {Tooltip,TooltipTrigger,TooltipContent,TooltipProvider} from '@/components/ui/tooltip';
import {useEffect,useMemo,useState,useSyncExternalStore} from 'react';
import {FileMusic,Columns2,PanelRight,Settings2,PlugZap} from 'lucide-react';
import {useUnsavedConfirmation} from './components/UnsavedConfirmation';
import {native,mcpBackend} from './bridge';
import {setLanguage,t} from './i18n';
import {type PreferenceStore,type Preferences} from './preferences-model';
import {McpController} from './mcp/model';
import {type Workbench} from './app/workbench';
import {SourcePane} from './components/SourcePane';
import {ScorePane} from './components/ScorePane';
import {Inspector} from './components/Inspector';
import {PreferencesDialog} from './components/PreferencesDialog';
import {WorkspaceLayout} from './components/WorkspaceLayout';
import {WavExportDialog} from './components/WavExportDialog';
import {AppMenus} from './components/AppMenus';
import {WindowControls} from './components/WindowControls';
import {titlebarDragProps} from './lib/window';
import {Button} from './components/ui/button';
import {Dialog,DialogContent,DialogHeader,DialogTitle,DialogDescription,DialogClose} from './components/ui/dialog';
import {GlassProvider} from './glass/GlassProvider';
import RenderWorker from './render/worker?worker';
type PreferenceTab='general'|'appearance'|'audio'|'mcp';
export function App({controller,store}:{controller:Workbench;store:PreferenceStore}){
 const state=useSyncExternalStore(controller.subscribe,controller.getSnapshot);
 const wavExportState=useSyncExternalStore(controller.wavExport.subscribe,controller.wavExport.getSnapshot);
 const wavExportOpen=wavExportState.phase!=='idle';
 const confirmation=useUnsavedConfirmation(controller);
 const offline=useOfflineExport(controller);
 const offlineOpen=offline.state.phase!=='idle';
 const [preferences,setPreferences]=useState(store.value),[preferenceOpen,setPreferenceOpen]=useState(false),[preferenceTab,setPreferenceTab]=useState<PreferenceTab>('general'),[aboutOpen,setAboutOpen]=useState(false),[compactPane,setCompactPane]=useState<'source'|'score'|'inspector'>('score'),[,setMcpVersion]=useState(0),[width,setWidth]=useState(window.innerWidth),[systemDark,setSystemDark]=useState(matchMedia('(prefers-color-scheme: dark)').matches);
 const mcp=useMemo(()=>new McpController(mcpBackend,()=>setMcpVersion(n=>n+1)),[]);
 setLanguage(preferences.language);
 const theme=preferences.theme==='system'?(systemDark?'dark':'light'):preferences.theme;
 const showPreferences=(tab:PreferenceTab='general')=>{setPreferenceTab(tab);setPreferenceOpen(true);};
 useEffect(()=>{store.onChange=p=>setPreferences({...p});return()=>{store.onChange=()=>{};};},[store]);
 useEffect(()=>{controller.attachRenderer(new RenderWorker());void controller.start();return()=>controller.dispose();},[controller]);
 useEffect(()=>{document.documentElement.lang=preferences.language;},[theme,preferences.language]);
 useEffect(()=>{const media=matchMedia('(prefers-color-scheme: dark)'),changed=()=>setSystemDark(media.matches),resize=()=>setWidth(window.innerWidth);media.addEventListener('change',changed);window.addEventListener('resize',resize);return()=>{media.removeEventListener('change',changed);window.removeEventListener('resize',resize);};},[]);
 useEffect(()=>{if(!native)return;void mcp.refresh();const timer=setInterval(()=>{if(preferenceOpen||mcp.status?.started)void mcp.refresh();},3000);return()=>clearInterval(timer);},[mcp,preferenceOpen]);
 useEffect(()=>{const key=(e:KeyboardEvent)=>{if(native&&!wavExportOpen&&!offlineOpen&&!preferenceOpen&&!aboutOpen&&!confirmation.open&&isPlaybackShortcut(e)){e.preventDefault();controller.playback.toggle();return;}if(wavExportOpen||offlineOpen||preferenceOpen||aboutOpen||confirmation.open||!(e.ctrlKey||e.metaKey)||e.altKey)return;const k=e.key.toLowerCase();if(k==='o'){e.preventDefault();void controller.open();}else if(k==='s'){e.preventDefault();void controller.save();}else if(k===','){e.preventDefault();showPreferences();}else if(k==='enter'){e.preventDefault();void controller.apply();}};document.addEventListener('keydown',key);return()=>document.removeEventListener('keydown',key);},[controller,preferenceOpen,aboutOpen,confirmation.open,wavExportOpen,offlineOpen]);
 const documentView=state.view?.valid?state.view:state.current;
 const mcpState=!native?'Native host required':mcp.busy?'Working…':mcp.status?.running?'Running':mcp.status?.error?'Stopped unexpectedly':'Stopped';
 return <TooltipProvider delayDuration={450}><GlassProvider preferences={{theme:preferences.theme,material:preferences.material,quality:preferences.glassQuality==='layered'?'layered':'blur'}}><Tabs className="app-shell" value={preferences.view} onValueChange={view=>store.update({view:view as Preferences['view']})}>
  <header className="app-titlebar glass-chrome" {...titlebarDragProps}>
   <div className="titlebar-start"><WindowControls placement="start"/><AppMenus offlineSupported={offline.supported} offlineDisabled={offline.model.busy||wavExportOpen||state.busy||controller.dirty||!state.current} onOffline={()=>offline.model.open()} controller={controller} state={state} preferences={preferences} onPreferences={()=>showPreferences()} onAbout={()=>setAboutOpen(true)} onChange={patch=>store.update(patch)}/></div>
   <div className="window-document"><span id="dirty-dot" hidden={!controller.unsaved} className="dirty-dot" aria-label={t('Unsaved changes')}/><span id="filename">{state.fileName}</span></div>
   <div className="titlebar-end"><div className="workspace-controls flex items-center gap-2" data-window-interactive><TabsList aria-label={t('Workspace view')}><TabsTrigger id="view-split" value="split" aria-label={t('Split')}><Columns2/></TabsTrigger><TabsTrigger id="view-score" value="score" aria-label={t('Score')}><FileMusic/></TabsTrigger></TabsList><Tooltip><TooltipTrigger asChild><span className="inline-flex"><Toggle id="inspector-toggle" variant="outline" size="sm" aria-label={t('Show Inspector')} pressed={preferences.showInspector} onPressedChange={showInspector=>store.update({showInspector})}><PanelRight/></Toggle></span></TooltipTrigger><TooltipContent>{t('Show Inspector')}</TooltipContent></Tooltip><Tooltip><TooltipTrigger asChild><Button id="open-preferences" variant="ghost" size="icon-sm" aria-label={t('Preferences')} onClick={()=>showPreferences()}><Settings2/></Button></TooltipTrigger><TooltipContent>{t('Preferences')}</TooltipContent></Tooltip></div><WindowControls/></div>
  </header>
  <TabsContent value={preferences.view} className="workspace-tab-content">
   <WorkspaceLayout width={width} preferences={preferences} compactPane={compactPane} onCompactPaneChange={setCompactPane} onWidthsChange={patch=>store.update(patch)}
    source={<div id="source-pane" className="source-slot"><SourcePane controller={controller} state={state} preferences={preferences} theme={theme}/></div>}
    score={<ScorePane controller={controller} state={state} showTitle={preferences.showTitle}/>}
    inspector={<aside id="inspector" className="inspector-pane pane glass glass-plane"><Inspector view={state.published??documentView} selected={state.selected} fileName={state.fileName} canEdit={!state.busy&&!controller.dirty&&state.phase==='ready'} dirty={controller.dirty} onCommit={(kind,value)=>controller.mutate(kind,value)} onReveal={()=>{store.update({view:'split'});setCompactPane('source');controller.reveal();}}/></aside>}/>
  </TabsContent>
  <footer className="statusbar glass-chrome"><span id="global-message" role="status" title={t(state.message,state.messageValues)}>{t(state.message,state.messageValues)}</span><div className="statusbar-meta"><span id="event-count">{documentView?t('{count} events',{count:documentView.events.length}):''}</span><span id="revision" hidden title={state.current?.revision_id}>{state.current?.revision_id.slice(0,9)}</span><Tooltip><TooltipTrigger asChild><Button id="mcp-settings" variant="ghost" size="sm" onClick={()=>showPreferences('mcp')}><PlugZap size={13}/><span>MCP</span><span className={`service-dot ${mcp.status?.running?'running':''}`}/><span>{t(mcpState)}</span></Button></TooltipTrigger><TooltipContent>{t('MCP service')}</TooltipContent></Tooltip></div></footer>
  {(preferenceOpen||aboutOpen||wavExportOpen)&&native&&<div className="modal-titlebar-drag" aria-hidden="true" {...titlebarDragProps}/>}
  {confirmation.dialog}
  <OfflineExportDialog offline={offline} revision={state.current?.revision_id??''} enabled={!state.busy&&!controller.dirty&&!!state.current}/>
  <WavExportDialog controller={controller.wavExport} state={wavExportState}/>
  <PreferencesDialog open={preferenceOpen} onOpenChange={setPreferenceOpen} tab={preferenceTab} onTabChange={setPreferenceTab} value={preferences} saved={store.saved} onChange={patch=>store.update(patch)} mcp={mcp} native={native}/>
  <Dialog open={aboutOpen} onOpenChange={setAboutOpen}><DialogContent className="glass glass--popup glass-plane glass-dialog" id="about-dialog" showCloseButton={false} onInteractOutside={event=>{if(event.detail.originalEvent.target instanceof Element&&event.detail.originalEvent.target.closest('.window-controls,.window-resize-edge,.modal-titlebar-drag'))event.preventDefault();}}><DialogHeader><DialogTitle>{t('About ScoreStream')}</DialogTitle><DialogDescription>{t('A workspace for source and notation.')}</DialogDescription></DialogHeader><p className="about-copy">ScoreStream · React + shadcn/ui<br/>Rust · Tauri · Monaco · Verovio</p><DialogClose asChild><Button id="close-about">{t('Close')}</Button></DialogClose></DialogContent></Dialog>
 </Tabs></GlassProvider></TooltipProvider>;
}
