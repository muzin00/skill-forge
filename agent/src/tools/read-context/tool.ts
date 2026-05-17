import {
  readContext,
  type ReadContextInput,
  type ReadContextOutput,
} from '../../lib/readContext.js';

defineTool(async (input: ReadContextInput): Promise<ReadContextOutput> => {
  return readContext(input);
});
