import createVerovioModule from 'verovio/wasm';
import { VerovioToolkit } from 'verovio/esm';
import {engravingPageWidth} from '../layout';
import type {RenderRequest,RenderResult} from '../types';
const ready=createVerovioModule().then(module=>new VerovioToolkit(module));
self.onmessage=async({data}:MessageEvent<RenderRequest>)=>{
 const start=performance.now();const result:RenderResult={requestId:data.requestId,key:data.key,pages:[],map:data.map,elapsed:0};
 try{
  const toolkit=await ready;
  toolkit.setOptions({pageWidth:engravingPageWidth(data.width),pageHeight:2600,scale:40,adjustPageHeight:true,breaks:'auto',header:'none',footer:'none',font:'Bravura',spacingSystem:14,pageMarginTop:40,pageMarginBottom:40,pageMarginLeft:30,pageMarginRight:30,svgViewBox:true});
  if(!toolkit.loadData(data.mei))throw Error('Verovio rejected the supported projection.');
  const count=toolkit.getPageCount();if(count>30)throw Error('Preview is limited to 30 pages.');
  for(let i=1;i<=count;i++)result.pages.push(toolkit.renderToSVG(i));
 }catch(e){result.error=String(e);}result.elapsed=performance.now()-start;self.postMessage(result);
};
