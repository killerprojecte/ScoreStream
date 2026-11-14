import {invoke} from '@tauri-apps/api/core';

/** Test-feature-only observer. It never drives the UI or changes preferences. */
export function observeControls() {
  const history: unknown[] = [];
  const round=(n:number)=>Math.round(n*1000)/1000;
  const box=(el:Element)=>{const r=el.getBoundingClientRect();return {x:round(r.x),y:round(r.y),width:round(r.width),height:round(r.height)};};
  const read=(el:Element)=>{
    const rect=el.getBoundingClientRect(), css=getComputedStyle(el), inner=el.querySelector('svg,[data-slot=switch-thumb]');
    const ir=inner?.getBoundingClientRect();
    return {id:el.id,slot:el.getAttribute('data-slot'),state:el.getAttribute('data-state'),box:box(el),
      radius:[css.borderTopLeftRadius,css.borderTopRightRadius,css.borderBottomRightRadius,css.borderBottomLeftRadius],
      border:[css.borderLeftWidth,css.borderRightWidth],background:css.backgroundColor,image:css.backgroundImage,shadow:css.boxShadow,
      inner:inner&&box(inner),gaps:ir&&{left:round(ir.left-rect.left),right:round(rect.right-ir.right),top:round(ir.top-rect.top),bottom:round(rect.bottom-ir.bottom)},
      className:el.getAttribute('class')};
  };
  let pending:ReturnType<typeof setTimeout>;
  const capture=()=>{
    const controls=Array.from(document.querySelectorAll('.workspace-controls [data-slot=toggle-group-item],.workspace-controls [data-slot=toggle],.workspace-controls [data-slot=button],.settings-row [data-slot=switch]')).map(read);
    const icons=Array.from(document.querySelectorAll('.settings-category-icon')).map(read);
    history.push({at:new Date().toISOString(),viewport:[innerWidth,innerHeight],controls,icons});
    if(history.length>60)history.shift();
    void invoke('report_smoke_test',{report:JSON.stringify({kind:'native-v0-control-geometry',history},null,2)});
  };
  const schedule=()=>{clearTimeout(pending);pending=setTimeout(capture,350);};
  document.addEventListener('click',schedule,true);document.addEventListener('keyup',schedule,true);window.addEventListener('resize',schedule);
  schedule();
}
