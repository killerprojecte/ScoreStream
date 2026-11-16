import {afterAll,describe,expect,it,vi} from 'vitest';
import type {RenderRequest,RenderResult} from '../src/types';

const toolkit=vi.hoisted(()=>({setOptions:vi.fn(),loadData:vi.fn(()=>true),getPageCount:vi.fn(()=>31),renderToSVG:vi.fn((page:number)=>`<svg data-page="${page}"/>`)}));
vi.mock('verovio/wasm',()=>({default:async()=>({})}));
vi.mock('verovio/esm',()=>({VerovioToolkit:class {constructor(){return toolkit;}}}));
afterAll(()=>vi.unstubAllGlobals());

describe('preview worker page scale',()=>{
 it('returns every page beyond the former 30-page ceiling through the actual worker handler',async()=>{
  const postMessage=vi.fn(),worker={onmessage:undefined as undefined|((event:{data:RenderRequest})=>Promise<void>),postMessage};
  vi.stubGlobal('self',worker);
  await import('../src/render/worker');
  const request:RenderRequest={requestId:42,key:{kind:'committed',revision:'large',layout:'wide'},mei:'<mei/>',width:1200,map:{'ss-e0':'note'}};
  await worker.onmessage!({data:request});
  expect(postMessage).toHaveBeenCalledOnce();
  const result=postMessage.mock.calls[0][0] as RenderResult;
  expect(result.error).toBeUndefined();
  expect(result.pages).toEqual(Array.from({length:31},(_,i)=>`<svg data-page="${i+1}"/>`));
  expect(toolkit.renderToSVG).toHaveBeenCalledTimes(31);
  expect(result).toMatchObject({requestId:42,key:request.key,map:request.map});
 });
});
