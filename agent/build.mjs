import { build } from 'rolldown';
import { mkdir } from 'node:fs/promises';

const SKILLS = [
  'call-llm',
  'interpret',
  'generate-code',
  'generate-code-from-signature',
  'echo',
  'error',
  'compose',
];

for (const name of SKILLS) {
  await mkdir(`dist/skills/${name}`, { recursive: true });
  for (const part of ['skill', 'schema']) {
    await build({
      input: `src/skills/${name}/${part}.ts`,
      output: {
        file: `dist/skills/${name}/${part}.js`,
        format: 'esm',
      },
    });
  }
}
