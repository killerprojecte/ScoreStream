import {readFileSync,readdirSync} from 'node:fs';
import {join} from 'node:path';
import {describe,it,expect} from 'vitest';
const root=new URL('../src/',import.meta.url);
function appSources(dir:string):string[]{return readdirSync(dir,{withFileTypes:true}).flatMap(entry=>entry.isDirectory()?entry.name==='ui'?[]:appSources(join(dir,entry.name)):/\.tsx$/.test(entry.name)?[join(dir,entry.name)]:[]);}
describe('official UI control coverage',()=>{
 it('has no app-owned raw form or button primitives outside official registry components',()=>{
  for(const path of appSources(root.pathname)) expect(readFileSync(path,'utf8'),path).not.toMatch(/<(button|input|select|textarea)\b/);
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
