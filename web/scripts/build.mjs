import { build } from 'vite';
import preact from '@preact/preset-vite';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { mkdtemp, readFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import assert from 'node:assert/strict';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const check = process.argv.includes('--check');
const output = check ? await mkdtemp(join(tmpdir(), 'dali-web-dist-')) : join(root, 'dist');
try {
  for (const name of ['login', 'app']) {
    const outDir = join(output, name);
    await build({ root, configFile: false, plugins: [preact()], logLevel: 'warn', build: {
      outDir, emptyOutDir: true, target: 'es2020', minify: true,
      lib: { entry: join(root, 'src', `${name}.tsx`), formats: ['es'], fileName: () => `${name}.js`, cssFileName: name },
    }});
    const files = (await readdir(outDir)).sort();
    assert.deepEqual(files, [`${name}.css`, `${name}.js`], 'bundle produced unexpected chunks or assets');
    if (check) for (const file of files) {
      assert.deepEqual(await readFile(join(outDir, file)), await readFile(join(root, 'dist', name, file)), `committed ${name}/${file} differs from source build`);
    }
  }
} finally { if (check) await rm(output, { recursive: true, force: true }); }
