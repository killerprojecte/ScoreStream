import {useEffect, useRef} from 'react';
import {AudioLines, Monitor, Plug, Settings2, X} from 'lucide-react';
import {Button} from '@/components/ui/button';
import {Label} from '@/components/ui/label';
import {Select, SelectContent, SelectItem, SelectTrigger, SelectValue} from '@/components/ui/select';
import {Switch} from '@/components/ui/switch';
import {Empty,EmptyHeader,EmptyMedia,EmptyDescription} from '@/components/ui/empty';
import {Tabs, TabsContent, TabsList, TabsTrigger} from '@/components/ui/tabs';
import {Dialog, DialogClose, DialogContent, DialogDescription, DialogTitle} from '@/components/ui/dialog';
import {t} from '../i18n';
import {defaults, type Preferences} from '../preferences-model';
import type {McpController} from '../mcp/model';
import {McpSettings} from './McpSettings';
import './preferences.css';

export type PreferenceTab = 'general' | 'appearance' | 'audio' | 'mcp';
export interface PreferencesDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  tab: PreferenceTab;
  onTabChange: (tab: PreferenceTab) => void;
  value: Preferences;
  saved: boolean;
  onChange: (patch: Partial<Omit<Preferences, 'version'>>) => void;
  mcp: McpController;
  native: boolean;
}

const preferenceTabs = [
  {value: 'general', title: 'General', icon: Settings2, color: 'slate'},
  {value: 'appearance', title: 'Appearance', icon: Monitor, color: 'indigo'},
  {value: 'audio', title: 'Audio', icon: AudioLines, color: 'rose'},
  {value: 'mcp', title: 'MCP', icon: Plug, color: 'green'},
] as const;

/** Every supported integer remains available; the official Select owns behavior. */
function FontSizeField({value, onChange}: {value: number; onChange: (value: number) => void}) {
  const options = Array.from(new Set([...Array.from({length: 13}, (_, index) => index + 10), value])).sort((a,b) => a-b);
  return <Select value={String(value)} onValueChange={next => onChange(Number(next))}>
    <SelectTrigger size="sm" id="pref-font"><SelectValue /></SelectTrigger>
    <SelectContent>{options.map(size => <SelectItem key={size} value={String(size)}>{size}</SelectItem>)}</SelectContent>
  </Select>;
}

export function PreferencesDialog({open, onOpenChange, tab, onTabChange, value, saved, onChange, mcp, native}: PreferencesDialogProps) {
  const dialog = useRef<HTMLDivElement>(null);
  const returnFocus = useRef<HTMLElement | null>(null);
  useEffect(() => {if (!open) mcp.token = '';}, [open, mcp]);
  useEffect(() => () => {mcp.token = '';}, [mcp]);
  function setOpen(next: boolean) {
    if (!next) mcp.token = '';
    onOpenChange(next);
  }
  const saveNotice = t(saved ? 'Preferences are saved on this computer.' : 'Preferences could not be saved on this computer.');

  return <Dialog open={open} onOpenChange={setOpen}>
    <DialogContent id="preferences-dialog" ref={dialog} className="preferences-dialog glass glass--popup glass-plane glass-dialog" showCloseButton={false} aria-labelledby="preferences-title" aria-describedby="preferences-description"
      onInteractOutside={event => {
        const target = event.detail.originalEvent.target;
        if (target instanceof Element && target.closest('.window-controls,.window-resize-edge,.modal-titlebar-drag')) event.preventDefault();
      }}
      onOpenAutoFocus={event => {
        returnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        const activeTab = dialog.current?.querySelector<HTMLElement>('[role="tab"][data-state="active"]');
        if (activeTab) {event.preventDefault(); activeTab.focus();}
      }}
      onCloseAutoFocus={event => {event.preventDefault(); if (returnFocus.current?.isConnected) returnFocus.current.focus();}}>
      <header className="preferences-header">
        <DialogTitle id="preferences-title">{t('Preferences')}</DialogTitle>
        <DialogDescription id="preferences-description" className="sr-only">{t('Preferences are saved on this computer.')}</DialogDescription>
        <DialogClose asChild><Button id="close-preferences" variant="ghost" size="icon-sm" aria-label={t('Close')}><X aria-hidden="true" /></Button></DialogClose>
      </header>
      <Tabs value={tab} onValueChange={next => onTabChange(next as PreferenceTab)} orientation="vertical" className="preferences-body">
        <aside className="preferences-sidebar">
          <div className="preferences-sidebar-label">ScoreStream</div>
          <TabsList className="preferences-tabs" aria-label={t('Preferences')}>
            {preferenceTabs.map(({value: key, title, icon: Icon, color}) => <TabsTrigger id={`preferences-tab-${key}`} key={key} value={key} aria-controls={`preferences-${key}`}>
              <span className={`settings-category-icon settings-category-${color}`}><Icon aria-hidden="true" /></span><span>{t(title)}</span>
            </TabsTrigger>)}
          </TabsList>
        </aside>
        <div className="preferences-content">
          <TabsContent id="preferences-general" aria-labelledby="preferences-tab-general" value="general" className="preference-section">
            <div className="preference-section-heading"><h3>{t('General')}</h3></div>
            <div className="settings-group">
              <div className="settings-row"><Label htmlFor="pref-language">{t('Language')}</Label>
                <Select value={value.language} onValueChange={language => onChange({language: language as Preferences['language']})}>
                  <SelectTrigger size="sm" id="pref-language"><SelectValue /></SelectTrigger><SelectContent><SelectItem value="en">English</SelectItem><SelectItem value="zh-CN">简体中文</SelectItem></SelectContent>
                </Select>
              </div>
              <p className="setting-help">{t('Choose the language used by the interface.')}</p>
            </div>
          </TabsContent>
          <TabsContent id="preferences-appearance" aria-labelledby="preferences-tab-appearance" value="appearance" className="preference-section">
            <div className="preference-section-heading"><h3>{t('Appearance')}</h3></div>
            <h4 className="settings-group-title">{t('Interface')}</h4>
            <div className="settings-group">
              <div className="settings-row"><Label htmlFor="pref-theme">{t('Theme')}</Label>
                <Select value={value.theme} onValueChange={theme => onChange({theme: theme as Preferences['theme']})}>
                  <SelectTrigger size="sm" id="pref-theme"><SelectValue /></SelectTrigger><SelectContent>{(['system', 'light', 'dark'] as const).map(theme => <SelectItem key={theme} value={theme}>{t({system: 'System', light: 'Light', dark: 'Dark'}[theme])}</SelectItem>)}</SelectContent>
                </Select>
              </div>
              <div className="settings-row"><Label htmlFor="pref-material">{t('Material')}</Label>
                <Select value={value.material} onValueChange={material => onChange({material: material as Preferences['material']})}>
                  <SelectTrigger size="sm" id="pref-material" aria-describedby="pref-material-help"><SelectValue /></SelectTrigger><SelectContent><SelectItem value="glass">{t('Glass')}</SelectItem><SelectItem value="solid">{t('Solid')}</SelectItem></SelectContent>
                </Select>
              </div>
              <div className="settings-row"><Label htmlFor="pref-glass-quality">{t('Glass effects')}</Label>
                <Select value={value.glassQuality} onValueChange={glassQuality => onChange({glassQuality: glassQuality as Preferences['glassQuality']})}>
                  <SelectTrigger size="sm" id="pref-glass-quality" disabled={value.material==='solid'}><SelectValue /></SelectTrigger><SelectContent><SelectItem value="auto">{t('Automatic blur')}</SelectItem><SelectItem value="layered">{t('Layered, without blur')}</SelectItem></SelectContent>
                </Select>
              </div>
              <p className="setting-help" id="pref-material-help">{t('Reduced transparency and high contrast settings take priority. Solid removes decorative transparency.')}</p>
            </div>
            <h4 className="settings-group-title">{t('Source editor')}</h4>
            <div className="settings-group">
              <div className="settings-row"><Label htmlFor="pref-font">{t('Editor font size')}</Label><FontSizeField value={value.fontSize} onChange={fontSize => onChange({fontSize})} /></div>
              <div className="settings-row"><Label htmlFor="pref-wrap">{t('Wrap source lines')}</Label><Switch id="pref-wrap" checked={value.wrap} onCheckedChange={wrap => onChange({wrap})} /></div>
            </div>
            <h4 className="settings-group-title">{t('Workspace')}</h4>
            <div className="settings-group">
              <div className="settings-row"><Label htmlFor="pref-title">{t('Show page title')}</Label><Switch id="pref-title" checked={value.showTitle} onCheckedChange={showTitle => onChange({showTitle})} /></div>
              <div className="settings-row"><Label htmlFor="pref-inspector">{t('Show Inspector')}</Label><Switch id="pref-inspector" checked={value.showInspector} onCheckedChange={showInspector => onChange({showInspector})} /></div>
              <div className="settings-row"><Label htmlFor="pref-view">{t('Default workspace view')}</Label>
                <Select value={value.view} onValueChange={view => onChange({view: view as Preferences['view']})}>
                  <SelectTrigger size="sm" id="pref-view"><SelectValue /></SelectTrigger><SelectContent><SelectItem value="split">{t('Split')}</SelectItem><SelectItem value="score">{t('Score')}</SelectItem></SelectContent>
                </Select>
              </div>
              <div className="settings-row"><span>{t('Reset pane sizes')}</span><Button id="pref-reset-layout" type="button" variant="outline" size="sm" onClick={() => {const p = defaults(); onChange({sourceWidth: p.sourceWidth, inspectorWidth: p.inspectorWidth});}}>{t('Reset pane sizes')}</Button></div>
              <p className="setting-help">{t('Drag the dividers to resize. Use arrow keys when a divider is focused.')}</p>
            </div>
          </TabsContent>
          <TabsContent id="preferences-audio" aria-labelledby="preferences-tab-audio" value="audio" className="preference-section">
            <div className="preference-section-heading"><h3>{t('Audio')}</h3></div>
            <Empty id="audio-settings"><EmptyHeader><EmptyMedia variant="icon"><AudioLines aria-hidden="true" /></EmptyMedia><EmptyDescription>{t('Playback settings appear here when the audio engine is available.')}</EmptyDescription></EmptyHeader></Empty>
          </TabsContent>
          <TabsContent id="preferences-mcp" aria-labelledby="preferences-tab-mcp" value="mcp" className="preference-section">
            <div className="preference-section-heading"><h3>{t('MCP')}</h3></div><McpSettings controller={mcp} native={native} /></TabsContent>
        </div>
      </Tabs>
      <footer className="preferences-footer"><span id="preferences-storage" className={saved ? undefined : 'save-error'} role="status">{saveNotice}</span></footer>
    </DialogContent>
  </Dialog>;
}
