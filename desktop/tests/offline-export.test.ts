import {describe,it,expect,vi} from 'vitest';
import {OfflineExport} from '../src/app/offline-export';
const deferred=<T>()=>{let resolve!:(value:T)=>void;let reject!:(error:unknown)=>void;const promise=new Promise<T>((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};};
describe('native offline export lifecycle',()=>{
 it('requires explicit confirmation and carries only revision/token, never paths',async()=>{
  const api={reserve:vi.fn(async()=> 'host-1'),run:vi.fn(async()=>({completed_frames:64})),cancel:vi.fn(async()=>{})};
  const m=new OfflineExport(api);await m.start('r1');expect(api.reserve).not.toHaveBeenCalled();
  m.open();await m.start('r1');expect(api.reserve).toHaveBeenCalledWith('r1');expect(api.run).toHaveBeenCalledWith('host-1','export');expect(m.getSnapshot().phase).toBe('succeeded');
 });
 it('edit/cancel before reserve returns retires late token without starting native run',async()=>{
  const reservation=deferred<string>();const api={reserve:()=>reservation.promise,run:vi.fn(),cancel:vi.fn(async()=>{})};
  const m=new OfflineExport(api);m.open();const done=m.start('r1');m.cancel();reservation.resolve('old');await done;
  expect(api.run).not.toHaveBeenCalled();expect(api.cancel).toHaveBeenCalledWith('old');expect(m.getSnapshot().phase).toBe('cancelled');
 });
 it('a native successful commit wins a concurrent cancel',async()=>{
  const rendered=deferred<unknown>();const api={reserve:async()=> 'one',run:()=>rendered.promise,cancel:vi.fn(async()=>{})};
  const m=new OfflineExport(api);m.open();const done=m.start('r1');await Promise.resolve();m.cancel();rendered.resolve({completed_frames:64});await done;
  expect(m.getSnapshot().phase).toBe('succeeded');expect(api.cancel).toHaveBeenCalledWith('one');
 });
 it('does not allow overlapping work and distinguishes native failure from chooser cancel',async()=>{
  const api={reserve:vi.fn(async()=> 'one'),run:vi.fn(async()=>null),cancel:vi.fn(async()=>{})};
  const m=new OfflineExport(api);m.open();const done=m.start('r1');await m.start('r1');await done;
  expect(api.reserve).toHaveBeenCalledTimes(1);expect(m.getSnapshot().phase).toBe('cancelled');
  api.run.mockRejectedValueOnce('OFFLINE_POLICY_INVALID');m.open();await m.start('r1');expect(m.getSnapshot().phase).toBe('failed');
 });
});

describe('offline IPC uncertainty',()=>{
 it('does not call an unrelated transport failure a confirmed cancellation',async()=>{
  const rendered=deferred<unknown>();const api={reserve:async()=> 'one',run:()=>rendered.promise,cancel:vi.fn(async()=>{})};
  const m=new OfflineExport(api);m.open();const done=m.start('r1');await Promise.resolve();m.cancel();rendered.reject('connection lost');await done;
  expect(m.getSnapshot().phase).toBe('failed');expect(api.cancel).toHaveBeenCalledWith('one');
 });
});

it('policy draft and validation actions share native reservation without rendering by default',async()=>{
 const api={reserve:vi.fn(async()=> 'draft-1'),run:vi.fn(async()=>({policy_compiled:false,unchosen_fields:['config.bank_sha256']})),cancel:vi.fn(async()=>{})};
 const m=new OfflineExport(api);m.open();await m.start('r1','draft');
 expect(api.run).toHaveBeenLastCalledWith('draft-1','draft');
 m.open();await m.start('r1','validate');expect(api.run).toHaveBeenLastCalledWith('draft-1','validate');
 expect(m.getSnapshot().action).toBe('validate');expect(m.getSnapshot().result).toEqual({policy_compiled:false,unchosen_fields:['config.bank_sha256']});
});
