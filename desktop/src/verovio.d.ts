declare module 'verovio/wasm' { export default function create(options?:Record<string,unknown>):Promise<unknown>; }
declare module 'verovio/esm' { export class VerovioToolkit { constructor(module:unknown); setOptions(options:Record<string,unknown>):void; loadData(input:string):boolean; getPageCount():number; renderToSVG(page:number):string; } }
