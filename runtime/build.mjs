import { build } from 'rolldown';
import { mkdir } from 'node:fs/promises';

await mkdir('dist', { recursive: true });

await build({
  input: 'src/index.ts',
  output: {
    file: 'dist/index.js',
    format: 'esm',
  },
  external: (id) => id.startsWith('skill-forge:'),
});
