import type {WavExportBridge,WavExportSession} from './wav-export';

/** Small injection seam so the binary IPC contract can be tested without Tauri. */
export type WavExportInvoke=<T>(command:string,args?:Record<string,unknown>|Uint8Array,options?:{headers:Record<string,string>})=>Promise<T>;
export function createWavExportBridge(invoke:WavExportInvoke):WavExportBridge {
 return {
  begin:source=>invoke<WavExportSession|null>('wav_export_begin',{source}),
  write:(sessionId,offset,bytes)=>invoke<number>('wav_export_write',bytes,{headers:{
   'x-scorestream-wav-session':sessionId,
   'x-scorestream-wav-offset':String(offset),
  }}),
  finish:sessionId=>invoke<string>('wav_export_finish',{sessionId}),
  abort:sessionId=>invoke<void>('wav_export_abort',{sessionId}),
 };
}
