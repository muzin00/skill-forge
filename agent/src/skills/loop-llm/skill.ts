import { loopLlm, type LoopLlmOpts } from '../../lib/loopLlm.js';

interface LoopLlmSkillInput extends LoopLlmOpts {
  prompt: string;
}

defineSkill(async (input: LoopLlmSkillInput): Promise<unknown> => {
  const { prompt, ...opts } = input;
  return loopLlm(prompt, opts);
});
