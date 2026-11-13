/** Defense in depth: renderer output never gains script, foreign HTML, or network authority. */
export function sanitizeSvg(svg:string):string {
 const document=new DOMParser().parseFromString(svg,'image/svg+xml');
 if(document.querySelector('parsererror')||document.documentElement.localName!=='svg')throw Error('Renderer returned malformed SVG.');
 document.querySelectorAll('script,foreignObject,iframe,object,embed,audio,video,animate,set,animateMotion,animateTransform').forEach(node=>node.remove());
 document.querySelectorAll('*').forEach(element=>{for(const attribute of [...element.attributes]){
  const name=attribute.name.toLowerCase(),value=attribute.value.trim();
  if(name.startsWith('on')||((name==='href'||name==='xlink:href')&&!value.startsWith('#'))||((name==='style'||name==='fill'||name==='filter'||name==='stroke')&&/url\(\s*['"]?(?!#)/i.test(value)))element.removeAttribute(attribute.name);
 }});
 return new XMLSerializer().serializeToString(document.documentElement);
}
