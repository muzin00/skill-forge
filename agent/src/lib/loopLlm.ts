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
declare const execCmd: (cmd: string, args: string[]) => Promise<string>;
declare const log: (message: string) => void;

const LOG_RESULT_MAX = 300;

export interface LoopLlmOpts {
  context: string[];
  allowSkills: string[];
  allowCommands?: string[];
  outputSchema: Record<string, unknown>;
  model?: string;
  maxTokens?: number;
  maxIterations?: number;
}

const DEFAULT_MODEL = 'claude-sonnet-4-6';
const DEFAULT_MAX_TOKENS = 4096;
const DEFAULT_MAX_ITERATIONS = 15;
const OUTPUT_TOOL_NAME = 'output';
const TOOL_NAME_PATTERN = /^[a-zA-Z0-9_-]{1,64}$/;

interface CommandSpec {
  toolName: string;
  cmd: string;
  prefixRest: string[];
  entry: string;
}

type ToolKind =
  | { kind: 'skill' }
  | { kind: 'command'; cmd: string; prefixRest: string[] };

export async function loopLlm<TOutput = Record<string, unknown>>(
  prompt: string,
  opts: LoopLlmOpts,
): Promise<TOutput> {
  const model = opts.model ?? DEFAULT_MODEL;
  const maxTokens = opts.maxTokens ?? DEFAULT_MAX_TOKENS;
  const maxIterations = opts.maxIterations ?? DEFAULT_MAX_ITERATIONS;

  const commandSpecs = parseAllowCommands(opts.allowCommands ?? []);
  const registry = buildRegistry(opts.allowSkills, commandSpecs);
  const tools = buildTools(opts.allowSkills, commandSpecs, opts.outputSchema);

  if (opts.context.length === 0) {
    throw 'loop-llm: context must contain at least one entry';
  }

  const client = new Anthropic();
  const messages: Array<{
    role: 'user' | 'assistant';
    content: string | RequestContentBlock[];
  }> = [
    {
      role: 'user',
      content: opts.context.map((text) => ({ type: 'text', text })),
    },
  ];

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
      toolResults.push(await dispatchTool(block, registry));
    }

    if (toolResults.length === 0) {
      throw 'spec-violation: assistant returned tool_use stop reason without tool_use blocks';
    }

    messages.push({ role: 'user', content: toolResults });
  }

  throw `loop-exceeded: agent did not call ${OUTPUT_TOOL_NAME} tool within ${maxIterations} iterations`;
}

function parseAllowCommands(entries: string[]): CommandSpec[] {
  const specs: CommandSpec[] = [];
  const seen = new Set<string>();
  for (const raw of entries) {
    const entry = raw.trim();
    if (!entry) {
      throw 'loop-llm: allowCommands entry must be non-empty';
    }
    const parts = entry.split(/\s+/);
    const toolName = parts.join('-');
    if (!TOOL_NAME_PATTERN.test(toolName)) {
      throw `loop-llm: allowCommands entry "${entry}" produces tool name "${toolName}" which does not match ${TOOL_NAME_PATTERN}`;
    }
    if (seen.has(toolName)) {
      throw `loop-llm: allowCommands has duplicate tool name "${toolName}" (entry "${entry}")`;
    }
    seen.add(toolName);
    specs.push({
      toolName,
      cmd: parts[0]!,
      prefixRest: parts.slice(1),
      entry: parts.join(' '),
    });
  }
  return specs;
}

function buildRegistry(
  allowSkills: string[],
  commandSpecs: CommandSpec[],
): Map<string, ToolKind> {
  const registry = new Map<string, ToolKind>();
  for (const name of allowSkills) {
    if (registry.has(name)) {
      throw `loop-llm: allowSkills has duplicate name "${name}"`;
    }
    registry.set(name, { kind: 'skill' });
  }
  for (const spec of commandSpecs) {
    if (registry.has(spec.toolName)) {
      throw `loop-llm: tool name "${spec.toolName}" (from allowCommands entry "${spec.entry}") collides with an allowSkills entry`;
    }
    registry.set(spec.toolName, {
      kind: 'command',
      cmd: spec.cmd,
      prefixRest: spec.prefixRest,
    });
  }
  return registry;
}

function buildTools(
  allowSkills: string[],
  commandSpecs: CommandSpec[],
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
  for (const spec of commandSpecs) {
    tools.push({
      name: spec.toolName,
      description: `Run \`${spec.entry}\` with the given args appended. Args are passed as argv (no shell evaluation: pipes, redirects, and substitutions are not interpreted). Returns stdout.`,
      input_schema: {
        type: 'object',
        properties: {
          args: {
            type: 'array',
            items: { type: 'string' },
            description: `Args appended after \`${spec.entry}\`.`,
          },
        },
        required: ['args'],
        additionalProperties: false,
      },
    });
  }
  tools.push({
    name: OUTPUT_TOOL_NAME,
    description:
      'Submit the final structured result and terminate the loop. You MUST call this tool exactly ONCE when you have gathered enough information to answer. Do not keep exploring indefinitely — once you have a reasonable conclusion, submit it via this tool. The loop will fail with `loop-exceeded` if you do not call this tool within the iteration budget.',
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
  registry: Map<string, ToolKind>,
): Promise<RequestContentBlock> {
  const inputJson = JSON.stringify(block.input);
  const entry = registry.get(block.name);
  if (!entry) {
    const msg = `tool not allowed: ${block.name}`;
    log(`[${block.name}] ${inputJson} -> error: ${msg}`);
    return {
      type: 'tool_result',
      tool_use_id: block.id,
      content: msg,
      is_error: true,
    };
  }
  try {
    const content =
      entry.kind === 'skill'
        ? await invokeSkillTool(block.name, block.input)
        : await invokeCommandTool(entry.cmd, entry.prefixRest, block.input);
    log(`[${block.name}] ${inputJson} -> ${truncateForLog(content)}`);
    return {
      type: 'tool_result',
      tool_use_id: block.id,
      content,
    };
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    log(`[${block.name}] ${inputJson} -> error: ${truncateForLog(msg)}`);
    return {
      type: 'tool_result',
      tool_use_id: block.id,
      content: msg,
      is_error: true,
    };
  }
}

async function invokeSkillTool(
  name: string,
  input: unknown,
): Promise<string> {
  const result = await invokeSkill(name, input);
  return typeof result === 'string' ? result : JSON.stringify(result, null, 2);
}

async function invokeCommandTool(
  cmd: string,
  prefixRest: string[],
  input: unknown,
): Promise<string> {
  const args = (input as { args?: unknown }).args;
  if (
    !Array.isArray(args) ||
    !args.every((a): a is string => typeof a === 'string')
  ) {
    throw `command tool requires args: string[], got ${JSON.stringify(input)}`;
  }
  return execCmd(cmd, [...prefixRest, ...args]);
}

function truncateForLog(s: string): string {
  if (s.length <= LOG_RESULT_MAX) return s;
  const head = s.slice(0, LOG_RESULT_MAX);
  const remaining = s.length - LOG_RESULT_MAX;
  return `${head}...(${remaining} more chars)`;
}
