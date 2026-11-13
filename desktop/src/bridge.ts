import { invoke, isTauri } from '@tauri-apps/api/core';
import type { View } from './types';
export const native = isTauri();
export const backend = {
  openDemo:()=>invoke<View>('open_demo'),
  current:()=>invoke<View>('current'),
  preview:(source:string)=>invoke<View>('preview_source',{source}),
  apply:(source:string,expectedRevisionId:string)=>invoke<View>('apply_source',{source,expectedRevisionId}),
  undo:()=>invoke<View>('undo'), redo:()=>invoke<View>('redo'),
  setPitch:(eventId:string,pitch:string,expectedRevisionId:string)=>invoke<View>('set_event_pitch',{eventId,pitch,expectedRevisionId}),
  setDuration:(eventId:string,duration:string,expectedRevisionId:string)=>invoke<View>('set_event_duration',{eventId,duration,expectedRevisionId}),
  openFile:()=>invoke<{path:string;source:string}|null>('open_source_file'),
  saveFile:(source:string)=>invoke<string|null>('save_source_file',{source}),
};
