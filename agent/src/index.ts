import { Anthropic, AnthropicAPIError } from './client.js';
import {
  generateCode as generateCodeImpl,
  generateCodeFromSignature as generateCodeFromSignatureImpl,
  type SignatureEntry,
} from './codegen.js';
import { interpret as interpretImpl, type Interpreted } from './interpret.js';
import { callLlm as callLlmImpl } from './llm.js';

export async function llm(
  prompt: string,
  model: string,
  apiKey: string,
): Promise<string> {
  if (!apiKey) {
    throw 'api-key argument is empty';
  }

  const client = new Anthropic({ apiKey });
  try {
    const response = await client.messages.create({
      model,
      max_tokens: 1024,
      messages: [{ role: 'user', content: prompt }],
    });
    const textBlock = response.content.find((b) => b.type === 'text');
    if (!textBlock) {
      throw 'no text content block in response';
    }
    return textBlock.text;
  } catch (e) {
    if (e instanceof AnthropicAPIError) {
      throw `HTTP ${e.status}: ${e.body}`;
    }
    if (typeof e === 'string') throw e;
    throw e instanceof Error ? e.message : String(e);
  }
}

export async function callLlm(
  prompt: string,
  inputJson: string,
  model: string,
  apiKey: string,
): Promise<string> {
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
