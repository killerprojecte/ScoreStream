// Exact arithmetic at the projection boundary. Never interpret ScoreStream source here.
export class Q {
  n:bigint; d:bigint;
  constructor(value:string|bigint,d=1n){const p=String(value).split('/');this.n=BigInt(p[0]);this.d=p[1]?BigInt(p[1]):d;if(this.d<=0n)throw Error('Invalid exact duration');}
  add(b:Q){return new Q(this.n*b.d+b.n*this.d,this.d*b.d);}
  sub(b:Q){return new Q(this.n*b.d-b.n*this.d,this.d*b.d);}
  cmp(b:Q){const x=this.n*b.d-b.n*this.d;return x<0n?-1:x>0n?1:0;}
  eq(b:Q){return this.cmp(b)===0;}
}
export function rhythm(value:Q):{dur:number;dots:number}{
 for(const dur of [1,2,4,8,16,32,64,128])for(let dots=0;dots<=3;dots++){
   if(value.eq(new Q(4n*((1n<<BigInt(dots+1))-1n),BigInt(dur)*(1n<<BigInt(dots)))))return {dur,dots};
 }
 throw Error('This preview supports ordinary and dotted durations through 128th notes. Tuplets and custom rhythms remain in the source.');
}
