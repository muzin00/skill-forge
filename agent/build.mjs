import { build } from 'rolldown';
import { mkdir, readFile } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';

const SKILLS = [
  'call-llm',
  'generate-skill-code',
  'echo',
  'error',
  'compose',
  'verify-references',
  'read-file',
  'grep-file',
  'loop-llm',
  'implementation-check',
  'view-issue',
];

const rawMarkdownPlugin = {
  name: 'raw-markdown',
  async resolveId(source, importer) {
    if (!source.endsWith('.md')) return null;
    const base = importer ? dirname(importer) : process.cwd();
    return resolve(base, source);
  },
  async load(id) {
    if (!id.endsWith('.md')) return null;
    const content = await readFile(id, 'utf8');
    return `export default ${JSON.stringify(content)};`;
  },
};

for (const name of SKILLS) {
  await mkdir(`dist/skills/${name}`, { recursive: true });
  for (const part of ['skill', 'schema']) {
    await build({
      input: `src/skills/${name}/${part}.ts`,
      output: {
        file: `dist/skills/${name}/${part}.js`,
        format: 'esm',
      },
      plugins: [rawMarkdownPlugin],
    });
  }
}
