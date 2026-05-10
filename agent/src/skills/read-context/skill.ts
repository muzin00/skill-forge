import {
  readContext,
  type ReadContextInput,
  type ReadContextOutput,
} from '../../lib/readContext.js';

defineSkill(async (input: ReadContextInput): Promise<ReadContextOutput> => {
  return readContext(input);
});
