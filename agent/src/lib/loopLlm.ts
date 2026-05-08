import {
  Anthropic,
  AnthropicAPIError,
  type ContentBlock,
  type MessagesCreateResponse,
  type RequestContentBlock,
  type ToolDefinition,
} from './anthropic-client.js';

declare const getSkillDescription: (skillName: string) => string;
declare const getSkillInputSchema: (skillName: string) => Record<string, unknown>;

export interface LoopLlmOpts {
  context: string;
  allowSkills: string[];
  outputSchema: Record<string, unknown>;
  model?: string;
  maxTokens?: number;
  maxIterations?: number;
}

const DEFAULT_MODEL = 'claude-sonnet-4-6';
const DEFAULT_MAX_TOKENS = 4096;
const DEFAULT_MAX_ITERATIONS = 15;
const OUTPUT_TOOL_NAME = 'output';

export async function loopLlm<TOutput = Record<string, unknown>>(
  prompt: string,
  opts: LoopLlmOpts,
): Promise<TOutput> {
  const model = opts.model ?? DEFAULT_MODEL;
  const maxTokens = opts.maxTokens ?? DEFAULT_MAX_TOKENS;
  const maxIterations = opts.maxIterations ?? DEFAULT_MAX_ITERATIONS;

  const allowedNames = new Set(opts.allowSkills);
  const tools = buildTools(opts.allowSkills, opts.outputSchema);

  const client = new Anthropic();
  const messages: Array<{
    role: 'user' | 'assistant';
    content: string | RequestContentBlock[];
  }> = [{ role: 'user', content: opts.context }];

  for (let iteration = 0; iteration < maxIterations; iteration++) {
    const response = await callMessages(
      client,
      model,
      maxTokens,
      prompt,
      tools,
      messages,
    );

    if (response.stop_reason !== 'tool_use') {
      throw `spec-violation: expected stop_reason "tool_use" but got "${response.stop_reason}"`;
    }

    const outputBlock = response.content.find(
      (b): b is Extract<ContentBlock, { type: 'tool_use' }> =>
        b.type === 'tool_use' && b.name === OUTPUT_TOOL_NAME,
    );
    if (outputBlock) {
      return outputBlock.input as TOutput;
    }

    messages.push({ role: 'assistant', content: response.content });

    const toolResults: RequestContentBlock[] = [];
    for (const block of response.content) {
      if (block.type !== 'tool_use') continue;
      toolResults.push(await dispatchTool(block, allowedNames));
    }

    if (toolResults.length === 0) {
      throw 'spec-violation: assistant returned tool_use stop reason without tool_use blocks';
    }

    messages.push({ role: 'user', content: toolResults });
  }

  throw `loop-exceeded: agent did not call ${OUTPUT_TOOL_NAME} tool within ${maxIterations} iterations`;
}

function buildTools(
  allowSkills: string[],
  outputSchema: Record<string, unknown>,
): ToolDefinition[] {
  const tools: ToolDefinition[] = [];
  for (const name of allowSkills) {
    tools.push({
      name,
      description: getSkillDescription(name),
      input_schema: getSkillInputSchema(name),
    });
  }
  tools.push({
    name: OUTPUT_TOOL_NAME,
    description:
      'Submit the final structured result. Call this exactly ONCE when you have completed the task. After this call, the loop terminates.',
    input_schema: outputSchema,
  });
  return tools;
}

async function callMessages(
  client: Anthropic,
  model: string,
  maxTokens: number,
  systemPrompt: string,
  tools: ToolDefinition[],
  messages: Array<{
    role: 'user' | 'assistant';
    content: string | RequestContentBlock[];
  }>,
): Promise<MessagesCreateResponse> {
  try {
    return await client.messages.create({
      model,
      max_tokens: maxTokens,
      system: systemPrompt,
      tools,
      messages,
    });
  } catch (e) {
    if (e instanceof AnthropicAPIError) {
      throw `api-error: HTTP ${e.status}: ${e.body}`;
    }
    if (e instanceof SyntaxError) {
      throw `parse-error: ${e.message}`;
    }
    throw `api-error: ${e instanceof Error ? e.message : String(e)}`;
  }
}

async function dispatchTool(
  block: Extract<ContentBlock, { type: 'tool_use' }>,
  allowedNames: Set<string>,
): Promise<RequestContentBlock> {
  if (!allowedNames.has(block.name)) {
    return {
      type: 'tool_result',
      tool_use_id: block.id,
      content: `tool not allowed: ${block.name}`,
      is_error: true,
    };
  }
  try {
    const result = await invokeSkill(block.name, block.input);
    const content =
      typeof result === 'string' ? result : JSON.stringify(result, null, 2);
    return {
      type: 'tool_result',
      tool_use_id: block.id,
      content,
    };
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return {
      type: 'tool_result',
      tool_use_id: block.id,
      content: msg,
      is_error: true,
    };
  }
}
