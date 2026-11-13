import type {RenderRequest,RenderResult} from '../types';
// One renderer owner, one running job, one replaceable pending job.
export class LatestOnly {
 private busy=false;private pending:RenderRequest|null=null;private current=0;
 constructor(private worker:Pick<Worker,'postMessage'|'onmessage'>,private publish:(r:RenderResult)=>void){worker.onmessage=(event:MessageEvent<RenderResult>)=>{this.busy=false;if(event.data.requestId===this.current)this.publish(event.data);if(this.pending){const next=this.pending;this.pending=null;this.dispatch(next);}};}
 invalidate(sequence:number){this.current=sequence;this.pending=null;}
 submit(request:RenderRequest){if(request.requestId!==this.current)return;if(this.busy)this.pending=request;else this.dispatch(request);}
 private dispatch(request:RenderRequest){this.busy=true;this.worker.postMessage(request);}
}
