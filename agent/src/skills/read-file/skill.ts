import { readFile, type ReadFileInput } from '../../lib/readFile.js';

defineSkill(async (input: ReadFileInput): Promise<string> => {
  return readFile(input);
});
