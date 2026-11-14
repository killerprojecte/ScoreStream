import {mkdtempSync,mkdirSync,readFileSync,readdirSync,rmSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {describe,it,expect} from 'vitest';
const root=new URL('../src/',import.meta.url);
function appSources(directory:string|URL):string[]{
 const dir=typeof directory==='string'?directory:fileURLToPath(directory);
 return readdirSync(dir,{withFileTypes:true}).flatMap(entry=>entry.isDirectory()?entry.name==='ui'?[]:appSources(join(dir,entry.name)):/\.tsx$/.test(entry.name)?[join(dir,entry.name)]:[]);
}
describe('official UI control coverage',()=>{
 it('walks file URLs containing spaces and escaped characters without scanning registry components',()=>{
  const dir=mkdtempSync(join(tmpdir(),'scorestream shadcn # 音乐-'));
  try{
   mkdirSync(join(dir,'components','ui'),{recursive:true});
   writeFileSync(join(dir,'App.tsx'),'<main />');
   writeFileSync(join(dir,'components','Panel.tsx'),'<section />');
   writeFileSync(join(dir,'components','ui','button.tsx'),'<button />');
   writeFileSync(join(dir,'ignored.ts'),'export {};');
   expect(appSources(pathToFileURL(dir)).sort()).toEqual([join(dir,'App.tsx'),join(dir,'components','Panel.tsx')].sort());
  }finally{rmSync(dir,{recursive:true,force:true});}
 });
 it('has no app-owned raw form or button primitives outside official registry components',()=>{
  for(const path of appSources(root)) expect(readFileSync(path,'utf8'),path).not.toMatch(/<(button|input|select|textarea)\b/);
 });
 it('does not call the browser confirm for app-owned prompts',()=>{
  expect(readFileSync(new URL('App.tsx',root),'utf8')).not.toMatch(/\bconfirm\s*\(/);
  expect(readFileSync(new URL('app/workbench.ts',root),'utf8')).not.toMatch(/\bconfirm\s*\(/);
 });
 it('uses shadcn radio semantics for the exclusive View menu',()=>{
  const source=readFileSync(new URL('components/AppMenus.tsx',root),'utf8');
  expect(source).toContain('<MenubarRadioGroup value={preferences.view}');
  expect(source).toContain('<MenubarRadioItem value="split">');
  expect(source).toContain('<MenubarRadioItem value="score">');
 });
});
