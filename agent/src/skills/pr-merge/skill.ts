import {
  prMerge,
  type PrMergeInput,
  type PrMergeOutput,
} from '../../lib/prMerge.js';

defineSkill(async (input: PrMergeInput): Promise<PrMergeOutput> => {
  return prMerge(input);
});
