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

await mkdir('dist/skills', { recursive: true });

for (const name of SKILLS) {
  await build({
    input: `src/skills/${name}.ts`,
    output: {
      file: `dist/skills/${name}.js`,
      format: 'esm',
    },
  });
}
