import { loopLlm, type LoopLlmOpts } from '../../lib/loopLlm.js';

interface LoopLlmToolInput extends LoopLlmOpts {
  prompt: string;
}

defineTool(async (input: LoopLlmToolInput): Promise<unknown> => {
  const { prompt, ...opts } = input;
  return loopLlm(prompt, opts);
});
