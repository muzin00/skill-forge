import { callLlm } from '../lib/llm.js';

const BENCH_MOCK_PROMPT = '__BENCH_MOCK__';

interface CallLlmInput {
  prompt: string;
  input?: unknown;
  model: string;
  apiKey: string;
}

export async function run(input: CallLlmInput): Promise<{ output: string }> {
  const { prompt, model, apiKey } = input;
  const inputJson = JSON.stringify(input.input ?? {});

  if (prompt === BENCH_MOCK_PROMPT) {
    return { output: inputJson };
  }
  if (!apiKey) {
    throw 'spec-violation: api-key argument is empty';
  }

  const output = await callLlm(prompt, inputJson, model, apiKey);
  return { output };
}
