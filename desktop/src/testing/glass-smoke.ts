import type {Workbench} from '../app/workbench';
import type {PreferenceStore} from '../preferences-model';
import {click,get,key,paint,until,visible,type Assert} from './native-harness';

/** Actual WebKitGTK composition/portal state, not a simulated browser backend. */
export async function runGlassSmoke(app:Workbench,preferences:PreferenceStore,assert:Assert){
 const saved={...preferences.value},revision=app.current!.revision_id,source=app.current!.source;
 const modes:unknown[]=[];
 try{
  for(const theme of ['light','dark'] as const)for(const mode of ['blur','layered','solid'] as const){
   preferences.update({theme,material:mode==='solid'?'solid':'glass',glassQuality:mode==='layered'?'layered':'auto'});await paint();
   const root=document.documentElement,panes=[...document.querySelectorAll('.pane.glass')];
   assert(panes.length===3,`${theme}/${mode}: every workspace pane uses shared glass material`);
   const effect=getComputedStyle(panes[0]);const backdrop=effect.getPropertyValue('backdrop-filter')||effect.getPropertyValue('-webkit-backdrop-filter');
   const expectedBlur=mode==='blur'&&root.dataset.glassQuality==='blur'&&root.dataset.material!=='solid';
   assert(expectedBlur?backdrop.includes('blur'):backdrop==='none',`${theme}/${mode}: compositor style matches resolved material policy`);
   const paper=getComputedStyle(document.querySelector('.score-paper')!);
   assert(paper.color==='rgb(20, 24, 29)',`${theme}/${mode}: music paper keeps opaque neutral dark ink`);
   assert(!getComputedStyle(document.querySelector('.app-titlebar')!).getPropertyValue('backdrop-filter').includes('blur'),`${theme}/${mode}: titlebar does not trap caption controls in a backdrop stacking context`);
   await click('open-preferences');await until(()=>visible('preferences-dialog'),'preferences');await key(get('preferences-tab-appearance'),'Enter');await paint();
   const dialog=get('preferences-dialog'),rect=dialog.getBoundingClientRect();
   assert(getComputedStyle(dialog).position==='fixed'&&Math.abs(rect.x+rect.width/2-innerWidth/2)<2,`${theme}/${mode}: glass dialog remains fixed and centered`);
   assert(!!get('pref-material')&&!!get('pref-glass-quality'),`${theme}/${mode}: visible labelled material and fallback choices`);
   assert(get('pref-wrap').children.length===1,`${theme}/${mode}: Radix switch keeps exactly one thumb`);
   const caption=document.querySelector<HTMLElement>('[data-window-action="maximize"]');if(caption){const b=caption.getBoundingClientRect();assert(document.elementFromPoint(b.x+b.width/2,b.y+b.height/2)?.closest('[data-window-action]')===caption,`${theme}/${mode}: native caption target remains above the modal`);}
   if(mode==='solid'){
    assert(!getComputedStyle(dialog).backgroundColor.startsWith('rgba'),`${theme}/solid: popup fill is fully opaque`);
    assert(!getComputedStyle(get('pref-font')).backgroundColor.startsWith('rgba'),`${theme}/solid: input fill is fully opaque`);
   }
   modes.push({theme,requested:mode,resolved:{material:root.dataset.material,quality:root.dataset.glassQuality},backdrop,paper:paper.backgroundColor,dialog:getComputedStyle(dialog).backgroundColor});
   await click('close-preferences');await until(()=>!visible('preferences-dialog'),'preferences close');
   assert(document.activeElement===get('open-preferences'),`${theme}/${mode}: dialog close restores opener focus`);
  }
  assert(app.current!.revision_id===revision&&app.current!.source===source,'Glass appearance changes never mutate score content or revision');
  return modes;
 }finally{if(visible('preferences-dialog'))await click('close-preferences');preferences.update(saved);await paint();}
}
