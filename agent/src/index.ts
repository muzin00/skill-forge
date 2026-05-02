import { Anthropic, AnthropicAPIError } from './client.js';
import { generateCode as generateCodeImpl } from './codegen.js';
import { interpret as interpretImpl, type Interpreted } from './interpret.js';

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
