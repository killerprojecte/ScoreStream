import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {describe,expect,it} from 'vitest';
import {loadConfigFromFile,transformWithEsbuild} from 'vite';

const root=fileURLToPath(new URL('../',import.meta.url));
const source=readFileSync(new URL('../src/main.tsx',import.meta.url),'utf8');

describe('native QA build boundary',()=>{
 it('removes smoke runners and observers from the ordinary frontend',async()=>{
  const config=await loadConfigFromFile({command:'build',mode:'production'},`${root}vite.config.ts`,root,'silent');
  expect(config?.config.define?.__SCORESTREAM_QA_BUILD__).toBe('false');
  const output=await transformWithEsbuild(source,'main.tsx',{loader:'tsx',define:{__SCORESTREAM_QA_BUILD__:'false'},minifySyntax:true});
  expect(output.code).not.toContain("import(\"./smoke\")");
  expect(output.code).not.toContain('control-geometry-v0');
  expect(output.code).not.toContain('__SCORESTREAM_SMOKE__');
  expect(output.code).not.toContain('__SCORESTREAM_V0_CONTROL_QA__');
 });
 it('retains the existing runners only in explicitly selected smoke mode',async()=>{
  const config=await loadConfigFromFile({command:'build',mode:'smoke'},`${root}vite.config.ts`,root,'silent');
  expect(config?.config.define?.__SCORESTREAM_QA_BUILD__).toBe('true');
  const output=await transformWithEsbuild(source,'main.tsx',{loader:'tsx',define:{__SCORESTREAM_QA_BUILD__:'true'},minifySyntax:true});
  expect(output.code).toContain('./smoke');
  expect(output.code).toContain('control-geometry-v0');
 });
 it('keeps production identity and requires an isolated profile for native QA',()=>{
  const native=new URL('../../crates/scorestream-desktop/',import.meta.url);
  const production=JSON.parse(readFileSync(new URL('tauri.conf.json',native),'utf8'));
  const qa=JSON.parse(readFileSync(new URL('tauri.smoke.conf.json',native),'utf8'));
  expect(production.productName).toBe('ScoreStream');
  expect(production.identifier).toBe('io.scorestream.desktop');
  expect(qa.identifier).toBe('io.scorestream.desktop.qa');
  expect(qa.bundle.active).toBe(false);
  expect(qa.build.beforeBuildCommand.script).toBe('npm run build:smoke');
  expect(qa.build.beforeDevCommand.script).toBe('npm run dev:smoke');
  const main=readFileSync(new URL('src/main.rs',native),'utf8');
  expect(main.indexOf('ensure_isolated_identifier')).toBeLessThan(main.indexOf('McpService::load'));
  const cargo=readFileSync(new URL('Cargo.toml',native),'utf8');
  expect(cargo).toMatch(/default\s*=\s*\["custom-protocol"\]/);
  expect(cargo).not.toMatch(/default\s*=\s*\[[^\]]*"smoke-test"/);
 });
});
