/** Output DTOs from scorestream_app::playback, never a second score interpreter. */
export interface EnvelopePoint {offset_seconds:number; gain:number}
export interface PlaybackNote {
 id:string; written_id?:string; part:string; instrument:string; voice:string;
 start_seconds:number; duration_seconds:number; midi_note:number; frequency_hz:number;
 velocity:number; gain_envelope:EnvelopePoint[];
}
export interface EventPosition {id:string; occurrence_id?:string; start_seconds:number; end_seconds:number}
export interface PlaybackMeasure extends EventPosition {part:string; label:string|null; occurrence_index?:number}
export interface PlaybackPlan {
 duration_seconds:number; notes:PlaybackNote[];
 routes:{part:string;instrument:string}[]; warnings:string[];
 diagnostics:unknown[]; event_positions:EventPosition[]; measures:PlaybackMeasure[];
}
