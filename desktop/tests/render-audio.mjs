// Uses only already-pinned build tooling and an installed Chromium executable.
// No audio device, network server, sandbox exceptions, or application QA hooks.
import {build} from 'esbuild';
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';

const root=fileURLToPath(new URL('..',import.meta.url)),temporary=await mkdtemp(join(tmpdir(),'scorestream-audio-render-'));
try{
 const bundle=await build({absWorkingDir:root,entryPoints:['tests/playback-render.ts'],bundle:true,write:false,format:'iife',target:'es2022'});
 const page=join(temporary,'render.html');
 await writeFile(page,`<!doctype html><meta charset="utf-8"><body data-status="PENDING"><script>${bundle.outputFiles[0].text.replaceAll('</script','<\\/script')}</script>`);
 const {stdout}=await promisify(execFile)(process.env.CHROMIUM_PATH||'chromium',['--headless',`--user-data-dir=${join(temporary,'profile')}`,'--dump-dom','--virtual-time-budget=15000',pathToFileURL(page).href],{timeout:30000,maxBuffer:1024*1024});
 const result=stdout.match(/<body data-status="(PASS|FAIL)">([\s\S]*?)<\/body>/);
 if(!result)throw new Error(`Browser produced no completed offline-render report: ${stdout.slice(0,1000)}`);
 console.log(result[2]);
 if(result[1]!=='PASS')process.exitCode=1;
}catch(error){
 console.error('Offline audio render did not pass. An installed Chromium with its normal sandbox must be runnable.');
 console.error(error);process.exitCode=1;
}finally{await rm(temporary,{recursive:true,force:true});}
