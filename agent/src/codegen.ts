import {
  Anthropic,
  AnthropicAPIError,
  type ContentBlock,
  type ToolDefinition,
  type ToolChoice,
} from './client.js';

export interface Generated {
  code: string;
  capabilities: string[];
}

const ALLOWED_CAPABILITIES = ['callLlm'] as const;
type AllowedCapability = (typeof ALLOWED_CAPABILITIES)[number];

const SYSTEM_PROMPT = `You are a code generation agent for skill-forge.

Given a natural language task, you produce JavaScript code that runs inside the skill-runtime sandbox, plus the set of host primitives ("capabilities") the code requires.

# Code generation policy

- Define a top-level \`async function run(input)\` as the entry point. \`input\` is an object whose shape you decide based on the task.
- Write deterministic control flow in the code itself. Conditionals, loops, string manipulation, parsing, formatting, and arithmetic must be plain JavaScript — never delegated to an LLM.
- Only delegate to the LLM (via \`callLlm\`) the parts that are inherently non-deterministic: classification, summarization, translation, free-form natural language generation, and similar judgement tasks.
- The generated code runs in a minimal JS environment. Do not use Node.js APIs, browser APIs, npm packages, or imports. Standard ECMAScript and the host primitives listed below are the only things available.

# Available host primitives

- \`callLlm(prompt: string, input?: object): Promise<string>\` — Ask an LLM to produce a string given a prompt and structured input. Use this only for non-deterministic transformations.

# Output protocol

Call the \`submit_generated_code\` tool exactly once. Do not produce any free-form text response. The tool input must contain:

- \`code\`: the full JavaScript source. Must define \`async function run(input)\` at the top level.
- \`capabilities\`: the list of host primitives the code actually invokes. If the code calls \`callLlm\`, include \`"callLlm"\`. If the code uses no host primitives, return an empty list.
`;

const SUBMIT_TOOL: ToolDefinition = {
  name: 'submit_generated_code',
  description:
    '生成したコードと、その実行に必要な host primitive を提出する。1 タスクにつき必ず 1 度だけ呼ぶ。',
  input_schema: {
    type: 'object',
    properties: {
      code: {
        type: 'string',
        description:
          'skill-runtime 上で実行される JS コード本体。トップレベルに async function run(input) を定義する。',
      },
      capabilities: {
        type: 'array',
        items: { type: 'string', enum: [...ALLOWED_CAPABILITIES] },
        description: 'code が実際に呼び出す host primitive のリスト。',
      },
    },
    required: ['code', 'capabilities'],
    additionalProperties: false,
  },
};

const TOOL_CHOICE: ToolChoice = { type: 'tool', name: 'submit_generated_code' };

export async function generateCode(
  prompt: string,
  model: string,
  apiKey: string,
): Promise<Generated> {
  if (!apiKey) {
    throw 'spec-violation: api-key argument is empty';
  }

  const client = new Anthropic({ apiKey });

  let response;
  try {
    response = await client.messages.create({
      model,
      max_tokens: 4096,
      system: SYSTEM_PROMPT,
      tools: [SUBMIT_TOOL],
      tool_choice: TOOL_CHOICE,
      messages: [{ role: 'user', content: prompt }],
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

  if (response.stop_reason !== 'tool_use') {
    throw `spec-violation: expected stop_reason "tool_use" but got "${response.stop_reason}"`;
  }

  const toolUse = response.content.find(
    (b): b is Extract<ContentBlock, { type: 'tool_use' }> =>
      b.type === 'tool_use' && b.name === 'submit_generated_code',
  );
  if (!toolUse) {
    throw 'spec-violation: no submit_generated_code tool_use block in response';
  }

  const input = toolUse.input;
  if (typeof input !== 'object' || input === null || Array.isArray(input)) {
    throw 'parse-error: tool_use input is not an object';
  }
  const inputObj = input as Record<string, unknown>;

  const code = inputObj.code;
  if (typeof code !== 'string') {
    throw 'spec-violation: tool_use input is missing string field "code"';
  }

  const capabilitiesRaw = inputObj.capabilities;
  if (!Array.isArray(capabilitiesRaw)) {
    throw 'spec-violation: tool_use input is missing array field "capabilities"';
  }
  const capabilities: AllowedCapability[] = [];
  for (const cap of capabilitiesRaw) {
    if (typeof cap !== 'string') {
      throw 'parse-error: capabilities entry is not a string';
    }
    if (!isAllowedCapability(cap)) {
      throw `spec-violation: unknown capability "${cap}" (allowed: ${ALLOWED_CAPABILITIES.join(', ')})`;
    }
    capabilities.push(cap);
  }

  return { code, capabilities };
}

function isAllowedCapability(value: string): value is AllowedCapability {
  return (ALLOWED_CAPABILITIES as readonly string[]).includes(value);
}
