import { defineConfig } from 'rolldown';

export default defineConfig({
  input: 'src/index.ts',
  output: {
    file: 'dist/agent.js',
    format: 'esm',
  },
  external: [/^wasi:/],
});
