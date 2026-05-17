import {
  prMerge,
  type PrMergeInput,
  type PrMergeOutput,
} from '../../lib/prMerge.js';

defineTool(async (input: PrMergeInput): Promise<PrMergeOutput> => {
  return prMerge(input);
});
