import {
  Anthropic,
  AnthropicAPIError,
  type ContentBlock,
} from './anthropic-client.js';

const MAX_TOKENS = 4096;

export async function callLlm(
  prompt: string,
  inputJson: string,
  model: string,
): Promise<string> {
  const client = new Anthropic();
  let response;
  try {
    response = await client.messages.create({
      model,
      max_tokens: MAX_TOKENS,
      system: prompt,
      messages: [{ role: 'user', content: inputJson }],
    });
  } catch (e) {
    if (e instanceof AnthropicAPIError) {
      throw `api-error: HTTP ${e.status}: ${e.body}`;
    }
    throw `api-error: ${e instanceof Error ? e.message : String(e)}`;
  }

  return response.content
    .filter((b): b is Extract<ContentBlock, { type: 'text' }> => b.type === 'text')
    .map((b) => b.text)
    .join('');
}
