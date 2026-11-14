// @vitest-environment jsdom
import {act,useState} from 'react';
import {createRoot} from 'react-dom/client';
import {it,expect,vi} from 'vitest';
import {Tooltip,TooltipProvider,TooltipTrigger,TooltipContent} from '../src/components/ui/tooltip';
import {ToggleGroup,ToggleGroupItem} from '../src/components/ui/toggle-group';
it('keeps tooltip state on its wrapper and toggle state on the official control',async()=>{
 vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT',true);
 const host=document.createElement('div');document.body.append(host);const root=createRoot(host);
 function Harness(){const [value,setValue]=useState('split');return <TooltipProvider><ToggleGroup type="single" value={value} onValueChange={next=>{if(next)setValue(next);}}>{['split','score'].map(item=><Tooltip key={item}><TooltipTrigger asChild><span className="inline-flex"><ToggleGroupItem value={item}>{item}</ToggleGroupItem></span></TooltipTrigger><TooltipContent>{item}</TooltipContent></Tooltip>)}</ToggleGroup></TooltipProvider>;}
 try{
  await act(async()=>root.render(<Harness/>));
  const controls=host.querySelectorAll<HTMLButtonElement>('[data-slot="toggle-group-item"]');
  expect(controls[0].dataset.state).toBe('on');expect(controls[1].dataset.state).toBe('off');
  await act(async()=>controls[1].click());
  expect(controls[0].dataset.state).toBe('off');expect(controls[1].dataset.state).toBe('on');
  expect(controls[1].parentElement?.getAttribute('data-state')).toBe('closed');
 }finally{await act(async()=>root.unmount());host.remove();vi.unstubAllGlobals();}
});
