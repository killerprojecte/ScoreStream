import { useEffect, useSyncExternalStore } from 'react';
import type { ResizeDirection } from '../lib/window';
import { clearWindowError, controlsForPlatform, getWindowState, nativeWindow, performWindowAction, startWindowResize, subscribeWindowState, windowChrome, type WindowControl } from '../lib/window';
import { getLanguage } from '../i18n';
import './window-controls.css';
import {Button} from '@/components/ui/button';
import {Tooltip,TooltipTrigger,TooltipContent} from '@/components/ui/tooltip';
import {Minus,Square,Copy,X} from 'lucide-react';

function controlLabel(control: WindowControl, restored: boolean) {
  const zh = getLanguage() === 'zh-CN';
  if (control === 'close') return zh ? '关闭窗口' : 'Close window';
  if (control === 'minimize') return zh ? '最小化窗口' : 'Minimize window';
  return restored ? (zh ? '还原窗口' : 'Restore window') : (zh ? '最大化窗口' : 'Maximize window');
}
const edges: ResizeDirection[] = ['North', 'NorthEast', 'East', 'SouthEast', 'South', 'SouthWest', 'West', 'NorthWest'];

/** Mount start before the menus; the default/end instance lives at the far right of the 48px titlebar. */
export function WindowControls({ placement = 'end' }: { placement?: 'start' | 'end' }) {
  const state = useSyncExternalStore(subscribeWindowState, getWindowState, getWindowState);
  const controls = controlsForPlatform(windowChrome, placement);
  const restored = state.maximized || state.fullscreen;
  useEffect(() => {
    if (placement !== 'end' || !nativeWindow) return;
    document.documentElement.dataset.windowPlatform = windowChrome.platform;
    const onKey = (event: KeyboardEvent) => {
      // AppKit normally owns these shortcuts. Route them only when the webview receives them.
      if (windowChrome.platform !== 'macos' || event.defaultPrevented || event.repeat) return;
      if (event.metaKey && event.ctrlKey && !event.altKey && event.key.toLowerCase() === 'f') {
        event.preventDefault(); void performWindowAction('fullscreen');
      } else if (event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey && event.key.toLowerCase() === 'm') {
        event.preventDefault(); void performWindowAction('minimize');
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [placement]);
  if (!nativeWindow) return null;
  if (windowChrome.platform === 'macos' && placement === 'start') return <div className="native-traffic-light-space" aria-hidden="true"/>;
  return <>
    {controls.length > 0 && <div className={`window-controls window-controls-${windowChrome.platform} window-controls-${placement}`} data-window-interactive data-window-focused={state.focused} data-window-theme={/breeze/i.test(windowChrome.theme) ? "breeze" : "standard"} role="group" aria-label={getLanguage() === 'zh-CN' ? '窗口控制' : 'Window controls'}>
      {controls.map(control => <Tooltip key={control}><TooltipTrigger asChild><Button variant="ghost" size="icon-sm" type="button" className={`window-control window-control-${control}`} data-window-action={control} aria-label={controlLabel(control, restored)} disabled={state.pending || (control === 'maximize' && !state.resizable && !state.fullscreen)} onMouseDown={event => event.stopPropagation()} onDoubleClick={event => event.stopPropagation()} onClick={() => void performWindowAction(control)}>
        {control === 'close' ? <X/> : control === 'minimize' ? <Minus/> : restored ? <Copy/> : <Square/>}
      </Button></TooltipTrigger><TooltipContent>{controlLabel(control, restored)}</TooltipContent></Tooltip>)}
    </div>}
    {placement === 'end' && <>
      {state.error && <div className="window-control-error" role="alert" data-window-interactive><span>{getLanguage() === 'zh-CN' ? '窗口操作失败：' : 'Window action failed: '}{state.error}</span><Button variant="ghost" size="icon-sm" type="button" onClick={clearWindowError} aria-label={getLanguage() === 'zh-CN' ? '关闭错误提示' : 'Dismiss error'}><X/></Button></div>}
      {windowChrome.platform === 'linux' && !restored && state.resizable && edges.map(edge => <div key={edge} className={`window-resize-edge window-resize-${edge.toLowerCase()}`} data-window-interactive aria-hidden="true" onMouseDown={event => { if (event.button !== 0) return; event.preventDefault(); event.stopPropagation(); startWindowResize(edge); }}/>) }
    </>}
  </>;
}
