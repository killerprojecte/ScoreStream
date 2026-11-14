import { invoke, isTauri } from '@tauri-apps/api/core';
import type { View } from './types';
import type { McpSettings, McpStatus } from './mcp/model';
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
export const mcpBackend = {
  status:()=>invoke<McpStatus>('mcp_status'),
  save:(settings:McpSettings)=>invoke<McpStatus>('mcp_save_settings',{settings}),
  start:(settings:McpSettings,token:string|null)=>invoke<McpStatus>('mcp_start',{settings,token}),
  stop:()=>invoke<McpStatus>('mcp_stop'),
};

export const playbackBackend = {
  plan:(source:string)=>invoke<import('./playback/types').PlaybackPlan>('playback_plan',{source}),
};
