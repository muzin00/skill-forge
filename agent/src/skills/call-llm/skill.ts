import { callLlm } from '../../lib/llm.js';

interface CallLlmInput {
  prompt: string;
  input?: unknown;
  model: string;
}

defineSkill(async (input: CallLlmInput): Promise<{ output: string }> => {
  const { prompt, model } = input;
  const inputJson = JSON.stringify(input.input ?? {});

  const output = await callLlm(prompt, inputJson, model);
  return { output };
});
