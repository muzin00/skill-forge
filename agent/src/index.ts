import {
  generateCode as generateCodeImpl,
  generateCodeFromSignature as generateCodeFromSignatureImpl,
  type SignatureEntry,
} from './codegen.js';
import { interpret as interpretImpl, type Interpreted } from './interpret.js';
import { callLlm as callLlmImpl } from './llm.js';

const BENCH_MOCK_PROMPT = '__BENCH_MOCK__';

export async function callLlm(
  prompt: string,
  inputJson: string,
  model: string,
  apiKey: string,
): Promise<string> {
  if (prompt === BENCH_MOCK_PROMPT) {
    return inputJson;
  }
  if (!apiKey) {
    throw 'spec-violation: api-key argument is empty';
  }
  return callLlmImpl(prompt, inputJson, model, apiKey);
}

export async function generateCode(
  prompt: string,
  model: string,
  apiKey: string,
): Promise<{ code: string; capabilities: string[] }> {
  return generateCodeImpl(prompt, model, apiKey);
}

export async function interpret(
  prompt: string,
  model: string,
  apiKey: string,
): Promise<Interpreted> {
  return interpretImpl(prompt, model, apiKey);
}

export async function generateCodeFromSignature(
  prompt: string,
  signature: SignatureEntry[],
  model: string,
  apiKey: string,
): Promise<{ code: string; capabilities: string[] }> {
  return generateCodeFromSignatureImpl(prompt, signature, model, apiKey);
}
