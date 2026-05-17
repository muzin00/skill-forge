import { grepFile, type GrepFileInput } from '../../lib/grepFile.js';

defineTool(async (input: GrepFileInput): Promise<string> => {
  return grepFile(input);
});
