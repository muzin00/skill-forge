import { readFile, type ReadFileInput } from '../../lib/readFile.js';

defineTool(async (input: ReadFileInput): Promise<string> => {
  return readFile(input);
});
