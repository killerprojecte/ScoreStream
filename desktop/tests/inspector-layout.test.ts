import {readFileSync} from 'node:fs';
import {describe,it,expect} from 'vitest';
import postcss from 'postcss';
const read=(name:string)=>readFileSync(new URL('../src/'+name,import.meta.url),'utf8');
describe('read-only Inspector alignment composition',()=>{
 it('fills two flexible columns and has no reserved trailing action column',()=>{
  const css=postcss.parse(read('components/inspector.css'));
  let columns='',padding='';
  css.walkRules(rule=>{if(rule.selector==='.property-entry')rule.walkDecls('grid-template-columns',decl=>{columns=decl.value;});if(rule.selector==='.property-entry dd')rule.walkDecls('padding',decl=>{padding=decl.value;});});
  expect(columns).toBe('minmax(0, 1fr) minmax(0, 1fr)');expect(padding).toBe('0');
  for(const source of [read('components/inspector.css'),read('reference-v0.css')])expect(source).not.toMatch(/\.property-entry dd\s*\{[^}]*padding-right/);
  expect(read('components/inspector.css')).toMatch(/\.property-entry dt\s*\{[^}]*min-width: 0;[^}]*overflow-wrap: anywhere/);
  expect(read('components/inspector.css')).toMatch(/\.property-entry dd\s*\{[^}]*min-width: 0;[^}]*text-align: right;[^}]*overflow-wrap: anywhere/);
 });
 it('retains balanced group insets and has no outer Resizable gutter in layout composition',()=>{
  expect(read('reference-v0.css')).toContain('.inspector-content .inspector-group {padding:20px;}');
  const layout=read('components/workspace-layout.css');
  expect(layout).toMatch(/\.app-workspace,\.desktop-workspace-panels\s*\{height:100%;width:100%;/);
  expect(layout).not.toMatch(/(?:padding|margin)-(?:left|right|inline)/);
  expect(read('components/WorkspaceLayout.tsx')).toContain('<ResizablePanelGroup');
 });
});
