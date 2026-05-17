import { build } from 'rolldown';
import { mkdir, readFile } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';

const TOOLS = [
  'call-llm',
  'generate-skill-code',
  'echo',
  'error',
  'compose',
  'verify-references',
  'read-file',
  'grep-file',
  'loop-llm',
  'view-issue',
  'validate-branch-name',
  'pr-merge',
  'read-context',
];

const SKILLS = [
  'echo-task',
  'implementation-check',
  'issue-checkout',
  'pr-create',
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

async function buildEntry(srcDir, distDir, entryName) {
  await mkdir(distDir, { recursive: true });
  for (const part of [entryName, 'schema']) {
    await build({
      input: `${srcDir}/${part}.ts`,
      output: {
        file: `${distDir}/${part}.js`,
        format: 'esm',
      },
      plugins: [rawMarkdownPlugin],
    });
  }
}

for (const name of TOOLS) {
  await buildEntry(`src/tools/${name}`, `dist/tools/${name}`, 'tool');
}

for (const name of SKILLS) {
  await buildEntry(`src/skills/${name}`, `dist/skills/${name}`, 'skill');
}
