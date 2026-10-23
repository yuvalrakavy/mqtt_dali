import {build} from 'vite';
import preact from '@preact/preset-vite';
import {fileURLToPath} from 'node:url';
import {dirname,join} from 'node:path';
import {mkdtemp,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import assert from 'node:assert/strict';
const root=dirname(dirname(fileURLToPath(import.meta.url)));
const check=process.argv.includes('--check');
const output=check?await mkdtemp(join(tmpdir(),'dali-emulator-assets-')):join(root,'dist');
try {
  await build({root,configFile:false,plugins:[preact()],logLevel:'warn',build:{outDir:output,emptyOutDir:true,target:'es2020',lib:{entry:join(root,'src/app.tsx'),formats:['es'],fileName:()=> 'app.js',cssFileName:'app'}}});
  if(check) for(const name of ['app.js','app.css']) assert.deepEqual(await readFile(join(output,name)),await readFile(join(root,'dist',name)),`stale ${name}`);
} finally {if(check) await rm(output,{recursive:true,force:true});}
