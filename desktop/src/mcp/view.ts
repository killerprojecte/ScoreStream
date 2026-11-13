import { McpController,isLoopbackBind,type McpApi } from './model';

export const mcpMarkup=`
<dialog id="mcp-dialog" aria-labelledby="mcp-title">
  <button id="close-mcp" class="dialog-close" aria-label="Close MCP settings">×</button>
  <span class="preview-label">LOCAL CONNECTION</span><h2 id="mcp-title">MCP service</h2>
  <p>Connect an MCP client over HTTP + SSE. It can read the committed score in this editor and the ScoreStream format reference. Private drafts stay private.</p>
  <div class="mcp-status-row"><span id="mcp-state" class="status-pill" role="status">Checking</span><span id="mcp-auth"></span></div>
  <div class="mcp-fields">
    <label>Listen IP<input id="mcp-bind" type="text" value="127.0.0.1" spellcheck="false" autocapitalize="off" autocomplete="off" aria-describedby="mcp-listen-help"></label>
    <label>Port<input id="mcp-port" type="number" value="3210" min="1" max="65535" step="1" inputmode="numeric" aria-describedby="mcp-listen-help"></label>
  </div>
  <p id="mcp-listen-help" class="mcp-help">127.0.0.1 or ::1 keeps access on this computer. Stop the service to change its settings.</p>
  <label class="mcp-key-label">Access key <span>(optional on localhost)</span><input id="mcp-key" type="password" autocomplete="new-password" spellcheck="false" autocapitalize="off" aria-describedby="mcp-key-help"></label>
  <p id="mcp-key-help" class="mcp-help">Session only. Keys are never saved to disk and must be entered again after stopping or restarting the app. Configure your client with an Authorization: Bearer header on both requests.</p>
  <p id="mcp-network-warning" class="mcp-warning" role="note" hidden>Network access: other devices may read your committed score. A key is required; use a long, random value. HTTP is not encrypted, so use only a trusted network or a secure tunnel. No firewall settings are changed.</p>
  <div id="mcp-url-row" hidden><label>SSE URL<input id="mcp-url" readonly spellcheck="false" aria-label="SSE URL, without access key"></label><button id="mcp-copy" class="quiet">Copy URL</button><p id="mcp-url-help" class="mcp-help"></p></div>
  <p id="mcp-error" class="mcp-error" role="alert" hidden></p><p id="mcp-notice" class="mcp-help" role="status"></p>
  <div class="mcp-actions"><button id="mcp-save" class="quiet">Save settings</button><span></span><button id="mcp-stop" class="quiet" disabled>Stop service</button><button id="mcp-start" class="primary">Start service</button></div>
  <p class="mcp-help">Only the IP and port are saved. The service always starts off when the app opens. Score edits and subscription notifications are not available through MCP yet.</p>
</dialog>`;

export class McpDialog {
  readonly controller:McpController;
  private dialog:HTMLDialogElement;
  private $=<T extends HTMLElement=HTMLElement>(id:string)=>document.getElementById(id) as T;
  constructor(api:McpApi,private available:boolean){
    this.dialog=this.$<HTMLDialogElement>('mcp-dialog');
    this.controller=new McpController(api,()=>this.render());
    this.$('mcp-settings').onclick=()=>{this.render();this.dialog.showModal();if(available)void this.controller.refresh();};
    this.$('close-mcp').onclick=()=>this.dialog.close();
    this.dialog.addEventListener('close',()=>{this.controller.token='';this.$<HTMLInputElement>('mcp-key').value='';});
    this.$('mcp-bind').addEventListener('input',()=>this.inputsChanged());
    this.$('mcp-port').addEventListener('input',()=>this.inputsChanged());
    this.$('mcp-key').addEventListener('input',()=>{this.controller.token=this.$<HTMLInputElement>('mcp-key').value;});
    for(const action of ['save','start','stop'] as const)this.$(`mcp-${action}`).onclick=()=>void this.controller.perform(action);
    this.$('mcp-copy').onclick=()=>void this.copyUrl();
    this.render();
    if(available){void this.controller.refresh();window.setInterval(()=>{if(this.dialog.open||this.controller.status?.started)void this.controller.refresh();},3000);}
  }
  private inputsChanged(){
    this.controller.settings={bind:this.$<HTMLInputElement>('mcp-bind').value,port:Number(this.$<HTMLInputElement>('mcp-port').value)};
    this.$('mcp-network-warning').hidden=isLoopbackBind(this.controller.settings.bind);
  }
  private render(){
    const {controller:c}=this,status=c.status,running=!!status?.running,locked=c.locked||!this.available||!status;
    this.$<HTMLInputElement>('mcp-bind').value=c.settings.bind;
    this.$<HTMLInputElement>('mcp-port').value=String(c.settings.port);
    this.$<HTMLInputElement>('mcp-key').value=c.token;
    for(const id of ['mcp-bind','mcp-port','mcp-key'])this.$<HTMLInputElement>(id).disabled=locked;
    this.$<HTMLButtonElement>('mcp-start').disabled=locked||!status;
    this.$<HTMLButtonElement>('mcp-save').disabled=locked||!status;
    this.$<HTMLButtonElement>('mcp-stop').disabled=c.busy||!status?.started||!this.available;
    this.$('mcp-state').textContent=!this.available?'Native host required':c.busy?'Working…':running?'Running':status?.started?'Stopped unexpectedly':'Stopped';
    this.$('mcp-state').className=`status-pill ${running?'valid':'pending'}`;
    this.$('mcp-auth').textContent=running?(status?.auth_required?'Key required':'Localhost · no key'):'';
    const button=this.$('mcp-settings');button.classList.toggle('mcp-running',running);button.setAttribute('aria-label',`MCP service settings, ${running?'running':'stopped'}`);
    this.$('mcp-network-warning').hidden=isLoopbackBind(c.settings.bind);
    this.$('mcp-url-row').hidden=!running||!status?.sse_url;
    this.$<HTMLInputElement>('mcp-url').value=status?.sse_url??'';
    this.$('mcp-url-help').textContent=['0.0.0.0','::'].includes(c.settings.bind)?'Wildcard listener: replace the host with this computer’s network IP in remote clients. The key is never part of this URL.':'The key is never part of this URL. Changes applied in the editor are visible on the next resource read.';
    this.$('mcp-error').hidden=!c.error;this.$('mcp-error').textContent=c.error??'';
    this.$('mcp-notice').textContent=this.available?c.notice:'MCP service controls are available only in the native desktop app.';
  }
  private async copyUrl(){
    const url=this.controller.status?.sse_url;if(!url)return;
    try{await navigator.clipboard.writeText(url);this.controller.notice='SSE URL copied. Add the key separately in your client.';}
    catch{this.$<HTMLInputElement>('mcp-url').select();this.controller.notice='Copy is unavailable here. The URL is selected; use Ctrl/⌘ C.';}
    this.$('mcp-notice').textContent=this.controller.notice;
  }
}
