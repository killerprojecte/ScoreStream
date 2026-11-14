import {useReducer, useRef} from 'react';
import {Copy, Play, Square} from 'lucide-react';
import {Button} from '@/components/ui/button';
import {Input} from '@/components/ui/input';
import {Label} from '@/components/ui/label';
import {Spinner} from '@/components/ui/spinner';
import {Alert,AlertDescription} from '@/components/ui/alert';
import {t} from '../i18n';
import {isLoopbackBind, type McpController} from '../mcp/model';

export interface McpSettingsProps {controller: McpController; native: boolean}
export function McpSettings({controller: c, native}: McpSettingsProps) {
  const [, render] = useReducer(count => count + 1, 0);
  const urlInput = useRef<HTMLInputElement>(null);
  const status = c.status;
  const running = !!status?.running;
  const locked = c.locked || !native || !status;
  const state = !native ? 'Native host required' : c.busy ? 'Working…' : running ? 'Running' : status?.started ? 'Stopped unexpectedly' : status ? 'Stopped' : 'Checking';
  const networkAccess = !isLoopbackBind(c.settings.bind);
  const stateError = c.error ? t(c.error) : null;

  async function copyUrl() {
    const url = c.status?.sse_url;
    if (!url) return;
    try {
      await navigator.clipboard.writeText(url);
      c.notice = 'SSE URL copied. Add the key separately in your client.';
    } catch {
      urlInput.current?.focus();
      urlInput.current?.select();
      c.notice = 'Copy is unavailable here. The URL is selected; use Ctrl/⌘ C.';
    }
    render();
  }

  return <section id="mcp-panel" className="mcp-panel" aria-labelledby="mcp-title" aria-busy={c.busy}>
    <div className="preference-section-heading"><h3 id="mcp-title">{t('MCP service')}</h3>
      <p>{t('Connect an MCP client over HTTP + SSE. It can read the committed score in this editor and the ScoreStream format reference. Private drafts stay private.')}</p>
    </div>
    <div className="mcp-status-row">
      <span id="mcp-state" className="mcp-status" data-running={running || undefined} role="status"><span aria-hidden="true" />{t(state)}</span>
      <span id="mcp-auth">{running ? t(status?.auth_required ? 'Key required' : 'Localhost · no key') : ''}</span>
    </div>
    <div className="settings-group mcp-settings-group">
      <div className="mcp-fields">
        <div className="mcp-field"><Label htmlFor="mcp-bind">{t('Listen IP')}</Label>
          <Input id="mcp-bind" type="text" value={c.settings.bind} disabled={locked} spellCheck={false} autoCapitalize="off" autoComplete="off"
            aria-describedby="mcp-listen-help" onChange={event => {c.settings = {...c.settings, bind: event.currentTarget.value}; render();}} />
        </div>
        <div className="mcp-field"><Label htmlFor="mcp-port">{t('Port')}</Label>
          <Input id="mcp-port" type="text" value={c.settings.port || ''} disabled={locked} inputMode="numeric" pattern="[0-9]*"
            aria-describedby="mcp-listen-help" aria-invalid={c.error === 'Port must be between 1 and 65535.' || undefined}
            onChange={event => {c.settings = {...c.settings, port: Number(event.currentTarget.value)}; render();}} />
        </div>
      </div>
      <p id="mcp-listen-help" className="setting-help">{t('127.0.0.1 or ::1 keeps access on this computer. Stop the service to change its settings.')}</p>
      <div className="mcp-field mcp-key-field">
        <Label htmlFor="mcp-key">{t('Access key')} <span className="mcp-optional">{t('(optional on localhost)')}</span></Label>
        <Input id="mcp-key" type="password" value={c.token} disabled={locked} autoComplete="new-password" spellCheck={false} autoCapitalize="off"
          aria-describedby="mcp-key-help" onChange={event => {c.token = event.currentTarget.value; render();}} />
      </div>
      <p id="mcp-key-help" className="setting-help">{t('Session only. Keys are never saved to disk and must be entered again after stopping or restarting the app. Configure your client with an Authorization: Bearer header on both requests.')}</p>
    </div>
    {networkAccess && <Alert id="mcp-network-warning" className="my-4" role="note"><AlertDescription>{t('Network access: other devices may read your committed score. A key is required; use a long, random value. HTTP is not encrypted, so use only a trusted network or a secure tunnel. No firewall settings are changed.')}</AlertDescription></Alert>}
    {running && status?.sse_url && <div id="mcp-url-row" className="settings-group mcp-url-group">
      <Label htmlFor="mcp-url">{t('SSE URL')}</Label>
      <div className="mcp-url-control"><Input ref={urlInput} id="mcp-url" value={status.sse_url} readOnly spellCheck={false} aria-label={t('SSE URL, without access key')} />
        <Button id="mcp-copy" type="button" size="sm" variant="outline" onClick={() => {void copyUrl();}}><Copy aria-hidden="true" />{t('Copy URL')}</Button>
      </div>
      <p id="mcp-url-help" className="setting-help">{t(['0.0.0.0', '::'].includes(c.settings.bind)
        ? 'Wildcard listener: replace the host with this computer’s network IP in remote clients. The key is never part of this URL.'
        : 'The key is never part of this URL. Changes applied in the editor are visible on the next resource read.')}</p>
    </div>}
    {stateError && <Alert id="mcp-error" variant="destructive" className="my-4"><AlertDescription>{stateError}</AlertDescription></Alert>}
    <p id="mcp-notice" className="setting-help mcp-notice" role="status">{t(native ? c.notice : 'MCP service controls are available only in the native desktop app.')}</p>
    <div className="mcp-actions">
      <Button id="mcp-save" type="button" size="sm" variant="outline" disabled={locked} onClick={() => {void c.perform('save');}}>{t('Save settings')}</Button>
      <div className="mcp-service-actions">
        <Button id="mcp-stop" type="button" size="sm" variant="outline" disabled={c.busy || !status?.started || !native} onClick={() => {void c.perform('stop');}}><Square aria-hidden="true" />{t('Stop service')}</Button>
        <Button id="mcp-start" type="button" size="sm" disabled={locked} onClick={() => {void c.perform('start');}}>{c.busy ? <Spinner aria-hidden="true" /> : <Play aria-hidden="true" />}{t('Start service')}</Button>
      </div>
    </div>
    <p className="setting-help mcp-disclosure">{t('Only the IP and port are saved. The service always starts off when the app opens. Score edits and subscription notifications are not available through MCP yet.')}</p>
  </section>;
}
