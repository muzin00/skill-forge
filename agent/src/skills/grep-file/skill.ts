import { grepFile, type GrepFileInput } from '../../lib/grepFile.js';

defineSkill(async (input: GrepFileInput): Promise<string> => {
  return grepFile(input);
});
