export interface McpSettings { bind:string; port:number }
export interface McpStatus {
  settings:McpSettings; running:boolean; started:boolean; sse_url:string|null;
  auth_required:boolean; error:string|null;
}
export interface McpApi {
  status():Promise<McpStatus>;
  save(settings:McpSettings):Promise<McpStatus>;
  start(settings:McpSettings,token:string|null):Promise<McpStatus>;
  stop():Promise<McpStatus>;
}
export const defaultMcpSettings=():McpSettings=>({bind:'127.0.0.1',port:3210});
// Presentation warning only. Rust's IpAddr parser and transport enforce the boundary.
export function isLoopbackBind(bind:string):boolean {
  const v4=bind.split('.');
  if(v4.length===4&&v4.every(part=>/^\d{1,3}$/.test(part)&&Number(part)<=255))return Number(v4[0])===127;
  if(!bind.includes(':'))return false;
  try{return new URL(`http://[${bind}]`).hostname==='[::1]';}catch{return false;}
}
export function settingsError(settings:McpSettings,token:string,starting:boolean):string|null {
  if(!settings.bind||settings.bind!==settings.bind.trim())return 'Enter a numeric listen IP address.';
  if(!Number.isInteger(settings.port)||settings.port<1||settings.port>65535)return 'Port must be between 1 and 65535.';
  if(starting&&token&&!/^[\x21-\x7e]+$/.test(token))return 'The key must use printable ASCII characters without spaces.';
  if(starting&&!isLoopbackBind(settings.bind)&&!token)return 'A key is required when listening beyond localhost.';
  return null;
}

/** UI-independent lifecycle model. Repeated actions and stale status replies are inert. */
export class McpController {
  settings=defaultMcpSettings(); token=''; status:McpStatus|null=null;
  busy=false; error:string|null=null; notice=''; private version=0; private refreshing=false;
  constructor(private api:McpApi,private changed:()=>void){}
  get locked(){return this.busy||!!this.status?.started;}
  async refresh(){
    if(this.busy||this.refreshing)return;
    this.refreshing=true;const version=this.version;
    try{
      const status=await this.api.status();if(version!==this.version)return;
      if(!this.status)this.settings={...status.settings};
      this.status=status;if(status.error)this.error=status.error;this.changed();
    }catch(error){if(version===this.version){this.error=String(error);this.changed();}}
    finally{this.refreshing=false;}
  }
  async perform(action:'save'|'start'|'stop'){
    if(this.busy||(action!=='stop'&&this.status?.started))return;
    const settings={...this.settings},token=this.token;
    const error=action==='stop'?null:settingsError(settings,token,action==='start');
    if(error){this.error=error;this.notice='';this.changed();return;}
    this.version++;this.busy=true;this.error=null;this.notice='';this.changed();
    try{
      const status=action==='start'?await this.api.start(settings,token||null):action==='save'?await this.api.save(settings):await this.api.stop();
      this.status=status;this.settings={...status.settings};
      // Only the native running transport keeps the key. Never persist it in the WebView.
      if(action!=='save')this.token='';
      this.error=status.error;
      this.notice=action==='start'?'MCP service started.':action==='stop'?'MCP service stopped.':'Listen settings saved. The service stays off until you start it.';
    }catch(error){this.error=String(error);}
    finally{this.busy=false;this.changed();}
  }
}
