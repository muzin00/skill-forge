import { callLlm } from '../../lib/llm.js';

interface CallLlmInput {
  prompt: string;
  input?: unknown;
  model: string;
  apiKey: string;
}

defineSkill(async (input: CallLlmInput): Promise<{ output: string }> => {
  const { prompt, model, apiKey } = input;
  const inputJson = JSON.stringify(input.input ?? {});

  if (!apiKey) {
    throw 'spec-violation: api-key argument is empty';
  }

  const output = await callLlm(prompt, inputJson, model, apiKey);
  return { output };
});
