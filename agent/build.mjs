import { build } from 'rolldown';
import { readFile, writeFile, mkdir } from 'node:fs/promises';

const SKILLS = [
  'call-llm',
  'interpret',
  'generate-code',
  'generate-code-from-signature',
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

  const path = `dist/skills/${name}.js`;
  let src = await readFile(path, 'utf8');
  src = stripExports(src);
  await writeFile(path, src);
}

function stripExports(src) {
  let out = src;
  out = out.replace(/^export\s*\{[\s\S]*?\};?\s*$/gm, '');
  out = out.replace(/^export\s+default\s+/gm, '');
  out = out.replace(/^export\s+(async\s+function|function|class)\b/gm, '$1');
  out = out.replace(/^export\s+(const|let|var)\b/gm, '$1');
  return out;
}
