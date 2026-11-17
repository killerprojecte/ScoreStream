export type PolicyAction='export'|'draft'|'validate';
export type OfflinePhase='idle'|'confirm'|'pending'|'cancelling'|'succeeded'|'cancelled'|'failed';
export interface OfflineState {phase:OfflinePhase;error:string;action:PolicyAction;result:unknown}
export interface OfflineBackend {reserve(revision:string):Promise<string>;run(id:string,action:PolicyAction):Promise<unknown|null>;cancel(id:string):Promise<void>}
/** Native host owns source, dialogs, paths, synthesis and publication. */
export class OfflineExport {
 private state:OfflineState={phase:'idle',error:'',action:'export',result:null};private listeners=new Set<()=>void>();
 private id:string|null=null;private cancelled=false;private active=false;
 constructor(private api:OfflineBackend){}
 subscribe=(listener:()=>void)=>{this.listeners.add(listener);return()=>this.listeners.delete(listener);};
 getSnapshot=()=>this.state;
 private set(phase:OfflinePhase,error=''){this.state={...this.state,phase,error};for(const l of this.listeners)l();}
 get busy(){return this.active;}
 open(){if(!this.active)this.set('confirm');}
 dismiss(){if(!this.active)this.set('idle');}
 async start(revision:string,action:PolicyAction='export'){
  if(this.active||this.state.phase!=='confirm')return;
  this.active=true;this.cancelled=false;this.state={...this.state,action,result:null};this.set('pending');
  try{
   const id=await this.api.reserve(revision);this.id=id;
   if(this.cancelled){await this.api.cancel(id);this.set('cancelled');return;}
   const result=await this.api.run(id,action);
   this.state={...this.state,result};
   // A successful publication may win a concurrent cancel: never call it undone.
   this.set(result===null?'cancelled':'succeeded');
  }catch(e){const confirmed=this.cancelled&&['OFFLINE_CANCELLED','OFFLINE_STALE','OFFLINE_DIRTY'].includes(String(e));this.set(confirmed?'cancelled':'failed',confirmed?'':String(e));}
  // busy is observed alongside the snapshot; clearing it must publish a new identity.
  finally{const id=this.id;if(id)await this.api.cancel(id).catch(()=>{});this.id=null;this.active=false;this.state={...this.state};for(const l of this.listeners)l();}
 }
 cancel(){
  if(!this.active)return;
  this.cancelled=true;this.set('cancelling');
  if(this.id)void this.api.cancel(this.id).catch(()=>{this.set('cancelling','Cancellation could not be confirmed. Waiting for the native result.');});
 }
}
