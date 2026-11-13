import {afterEach,describe,expect,it,vi} from 'vitest';
import {McpController,defaultMcpSettings,isLoopbackBind,settingsError,type McpApi,type McpStatus} from '../src/mcp/model';
import {McpDialog,mcpMarkup} from '../src/mcp/view';

const stopped=():McpStatus=>({settings:defaultMcpSettings(),running:false,started:false,sse_url:null,auth_required:false,error:null});
const running=():McpStatus=>({...stopped(),running:true,started:true,sse_url:'http://127.0.0.1:3210/sse',auth_required:true});
function fixture(){
  const api:McpApi={status:vi.fn(async()=>stopped()),save:vi.fn(async settings=>({...stopped(),settings})),start:vi.fn(async()=>running()),stop:vi.fn(async()=>stopped())};
  return {api,model:new McpController(api,vi.fn())};
}
function deferred<T>(){let resolve!:(v:T)=>void;let reject!:(v:unknown)=>void;const promise=new Promise<T>((r,j)=>{resolve=r;reject=j;});return {promise,resolve,reject};}

describe('MCP preferences and security presentation',()=>{
  it('defaults to loopback and gives conservative network warnings',()=>{
    expect(defaultMcpSettings()).toEqual({bind:'127.0.0.1',port:3210});
    for(const ip of ['127.0.0.1','127.1.2.3','::1','0:0:0:0:0:0:0:1'])expect(isLoopbackBind(ip)).toBe(true);
    for(const ip of ['0.0.0.0','192.168.1.4','::','::ffff:127.0.0.1','localhost','127.0.0.999','example.org'])expect(isLoopbackBind(ip)).toBe(false);
  });
  it('requires a key off localhost and rejects malformed ports/header injection',()=>{
    expect(settingsError({bind:'0.0.0.0',port:3210},'',true)).toContain('key is required');
    expect(settingsError({bind:'0.0.0.0',port:3210},'',false)).toBeNull();
    expect(settingsError(defaultMcpSettings(),'\r\nInjected',true)).toContain('printable ASCII');
    for(const port of [0,65536,NaN,1.5])expect(settingsError({bind:'127.0.0.1',port},'',true)).toContain('Port');
  });
  it('keeps keys masked and documents session lifetime, network exposure and no editing',()=>{
    expect(mcpMarkup).toContain('type="password"');expect(mcpMarkup).toContain('never saved to disk');
    expect(mcpMarkup).toContain('HTTP is not encrypted');expect(mcpMarkup).toContain('Private drafts stay private');
    expect(mcpMarkup).toContain('subscription notifications are not available');
  });
});

describe('MCP lifecycle controller',()=>{
  it('saves only non-secret preferences without starting a listener',async()=>{
    const {api,model}=fixture();model.token='secret';model.settings={bind:'::1',port:5555};
    await model.perform('save');expect(api.save).toHaveBeenCalledWith({bind:'::1',port:5555});
    expect(api.start).not.toHaveBeenCalled();expect(model.status?.running).toBe(false);
  });
  it('starts once despite repeated clicks, clears the UI key and locks settings',async()=>{
    const {api,model}=fixture();const response=deferred<McpStatus>();api.start=vi.fn(()=>response.promise);model.token='session-key';
    const first=model.perform('start');await model.perform('start');await model.perform('save');
    expect(api.start).toHaveBeenCalledTimes(1);expect(api.save).not.toHaveBeenCalled();expect(model.busy).toBe(true);
    response.resolve(running());await first;expect(model.token).toBe('');expect(model.locked).toBe(true);
    expect(model.status?.sse_url).not.toContain('session-key');await model.perform('start');expect(api.start).toHaveBeenCalledTimes(1);
  });
  it('surfaces bind failures, preserves input and permits retry',async()=>{
    const {api,model}=fixture();api.start=vi.fn().mockRejectedValueOnce('Address already in use').mockResolvedValueOnce(running());
    model.token='session-key';await model.perform('start');expect(model.error).toContain('Address already in use');expect(model.busy).toBe(false);
    expect(model.token).toBe('session-key');await model.perform('start');expect(model.status?.running).toBe(true);expect(model.error).toBeNull();
  });
  it('stops and allows restart, with no silently reused key',async()=>{
    const {api,model}=fixture();await model.perform('start');await model.perform('stop');
    expect(model.locked).toBe(false);expect(model.status?.sse_url).toBeNull();await model.perform('start');
    expect(api.start).toHaveBeenLastCalledWith(defaultMcpSettings(),null);
  });
  it('cannot overwrite a start with a late stopped status response',async()=>{
    const {api,model}=fixture();const old=deferred<McpStatus>();api.status=vi.fn(()=>old.promise);
    const refresh=model.refresh();await model.perform('start');old.resolve(stopped());await refresh;
    expect(model.status?.running).toBe(true);
  });
  it('keeps editable settings on periodic refresh and reports unexpected shutdown',async()=>{
    const {api,model}=fixture();await model.refresh();model.settings={bind:'::1',port:6000};await model.refresh();
    expect(model.settings.port).toBe(6000);
    api.status=vi.fn(async()=>({...stopped(),started:true,error:'Stopped unexpectedly'}));await model.refresh();
    expect(model.locked).toBe(true);expect(model.error).toContain('unexpectedly');await model.perform('stop');expect(model.locked).toBe(false);
  });
  it('does not call native start for an unkeyed LAN listener',async()=>{
    const {api,model}=fixture();model.settings.bind='0.0.0.0';await model.perform('start');
    expect(api.start).not.toHaveBeenCalled();expect(model.error).toContain('key is required');
  });
});

// Minimal UI surface: exercise real dialog wiring without claiming WebView rendering.
class ElementStub {
  value='';textContent='';hidden=false;disabled=false;open=false;className='';
  onclick:(()=>void)|null=null;events=new Map<string,()=>void>();
  classList={toggle:vi.fn()};setAttribute=vi.fn();select=vi.fn();
  addEventListener(event:string,callback:()=>void){this.events.set(event,callback);}
  showModal(){this.open=true;}
  close(){this.open=false;this.events.get('close')?.();}
}
function dialogFixture(){
  const elements=new Map<string,ElementStub>();
  for(const id of ['mcp-settings',...Array.from(mcpMarkup.matchAll(/id="([^"]+)"/g),match=>match[1])])elements.set(id,new ElementStub());
  vi.stubGlobal('document',{getElementById:(id:string)=>elements.get(id)});
  vi.stubGlobal('window',{setInterval:vi.fn()});
  const {api}=fixture();const dialog=new McpDialog(api,true);
  return {api,dialog,element:(id:string)=>elements.get(id)!};
}
afterEach(()=>vi.unstubAllGlobals());
describe('MCP dialog wiring',()=>{
  it('clears an unsubmitted key on close and does not reopen after a pending start',async()=>{
    const {api,dialog,element}=dialogFixture();await Promise.resolve();
    element('mcp-settings').onclick!();element('mcp-key').value='session-key';element('mcp-key').events.get('input')!();
    const pending=deferred<McpStatus>();api.start=vi.fn(()=>pending.promise);const action=dialog.controller.perform('start');
    element('close-mcp').onclick!();expect(dialog.controller.token).toBe('');expect(element('mcp-key').value).toBe('');
    pending.resolve(running());await action;expect(element('mcp-dialog').open).toBe(false);expect(element('mcp-start').disabled).toBe(true);expect(element('mcp-stop').disabled).toBe(false);
  });
  it('copies only the clean URL and has an explicit clipboard fallback',async()=>{
    const {dialog,element}=dialogFixture();await Promise.resolve();await dialog.controller.perform('start');
    const copy=vi.fn().mockResolvedValueOnce(undefined).mockRejectedValueOnce(new Error('unavailable'));
    vi.stubGlobal('navigator',{clipboard:{writeText:copy}});
    element('mcp-copy').onclick!();await Promise.resolve();expect(copy).toHaveBeenCalledWith('http://127.0.0.1:3210/sse');
    element('mcp-copy').onclick!();await Promise.resolve();expect(element('mcp-url').select).toHaveBeenCalled();expect(element('mcp-notice').textContent).toContain('Ctrl/⌘ C');
  });
});
