export interface Diagnostic { line:number; column:number; code:string; message:string; severity:string }
export interface ScoreEvent { id:string; voice:string; pitch:string; time:string; duration:string; staff:number; chord:string|null; written_duration:string|null; dots:number; source_line:number|null; attributes?:Record<string,unknown> }
export interface Part { id:string; instrument:string; staves:number }
export interface Voice { id:string; part:string; staff:number }
export interface Measure { id:string; part:string; time:string; length:string; label:string|null }
export interface State { time:string; scope:string; values:Record<string,unknown> }
// Mirrors the tagged records serialized by scorestream-core; never reads SFS text.
export type Scalar = string | number | boolean;
export interface MarkRecord { record:'mark'; target:string; kind:string; attributes:Record<string,Scalar> }
export type DocumentRecord = MarkRecord | { record:'event' | 'state' | 'measure' | 'relation'; [key:string]:unknown };
export interface ScoreDocument { frames?:{time:string; records:DocumentRecord[]}[]; [key:string]:unknown }
export interface View { valid:boolean; diagnostics:Diagnostic[]; source:string; events:ScoreEvent[]; parts:Part[]; voices:Voice[]; measures:Measure[]; states:State[]; document:ScoreDocument; revision_id:string; dirty:boolean; draft_seq:number; can_undo:boolean; can_redo:boolean }
export type PreviewKey = { kind:'committed'; revision:string; layout:string } | { kind:'draft'; session:string; sequence:number; hash:string; baseRevision:string; layout:string };
export interface RenderRequest { requestId:number; key:PreviewKey; mei:string; width:number; map:Record<string,string> }
export interface RenderResult { requestId:number; key:PreviewKey; pages:string[]; map:Record<string,string>; elapsed:number; error?:string }
