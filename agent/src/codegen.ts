import {
  Anthropic,
  AnthropicAPIError,
  type ContentBlock,
  type MessagesCreateResponse,
  type ToolDefinition,
  type ToolChoice,
} from './client.js';

export interface Generated {
  code: string;
  capabilities: string[];
}

export interface SignatureEntry {
  tool: string;
  input: string;
  output: string;
}

const ALLOWED_CAPABILITIES = ['callLlm', 'execCmd'] as const;
type AllowedCapability = (typeof ALLOWED_CAPABILITIES)[number];

const SHARED_POLICY = `# Code generation policy

- Define a top-level \`async function run(input)\` as the entry point. \`input\` is an object whose shape you decide based on the task.
- Write deterministic control flow in the code itself. Conditionals, loops, string manipulation, parsing, formatting, and arithmetic must be plain JavaScript — never delegated to an LLM.
- Only delegate to the LLM (via \`callLlm\`) the parts that are inherently non-deterministic: classification, summarization, translation, free-form natural language generation, and similar judgement tasks.
- Delegate external process invocations to \`execCmd\`. Output post-processing (\`JSON.parse\`, \`.split\`, regex, etc.) must be plain JavaScript outside the primitive call — never bake parsing into the command itself or ask \`callLlm\` to parse it.
- The generated code runs in a minimal JS environment. Do not use Node.js APIs, browser APIs, npm packages, or imports. Standard ECMAScript and the host primitives listed below are the only things available.

# Available host primitives

- \`callLlm(prompt: string, input?: object): Promise<string>\` — Ask an LLM to produce a string given a prompt and structured input. Use this only for non-deterministic transformations.
- \`execCmd(cmd: string, args: string[]): Promise<string>\` — Run an external command on the host and return its stdout as a string. Use this for deterministic external invocations (CLI tools, system commands).

# Output protocol

Call the \`submit_generated_code\` tool exactly once. Do not produce any free-form text response. The tool input must contain:

- \`code\`: the full JavaScript source. Must define \`async function run(input)\` at the top level.
- \`capabilities\`: the list of host primitives the code actually invokes. Include \`"callLlm"\` if the code calls \`callLlm\`, and \`"execCmd"\` if the code calls \`execCmd\`. If the code uses no host primitives, return an empty list.
`;

const SYSTEM_PROMPT = `You are a code generation agent for skill-forge.

Given a natural language task, you produce JavaScript code that runs inside the skill-runtime sandbox, plus the set of host primitives ("capabilities") the code requires.

${SHARED_POLICY}`;

const SIGNATURE_SYSTEM_PROMPT = `You are a code generation agent for skill-forge.

Given a natural-language task and an observed execution signature, you produce JavaScript code that reproduces the task's structure, plus the set of host primitives ("capabilities") the code requires.

${SHARED_POLICY}
# Signature-driven generation rules

The user message contains two sections:

- \`<task>\` — the original natural-language task.
- \`<signature>\` — the observed execution trace, a JSON array of \`{tool, input, output}\` entries. \`input\` and \`output\` are JSON-encoded strings. Each \`callLlm\` entry's \`input\` decodes to \`{prompt, input?}\`. Each \`execCmd\` entry's \`input\` decodes to \`{cmd, args}\`. The trailing \`finalAnswer\` entry's \`input\` decodes to \`{result}\`.

You MUST produce code that mirrors the signature:

1. **Control flow order**: The order of host-primitive calls (\`callLlm\` and \`execCmd\`) in the generated code must match the order of those entries in the signature.
2. **Strict prompt mapping**: For each \`callLlm\` entry, the generated code's \`callLlm\` call must use the exact same \`prompt\` string literal that appears in that entry.
3. **Strict input-key mapping**: The generated code's \`callLlm\` input object must use the same set of keys as the signature entry's input. Key names may not be renamed; do not add or remove keys.
4. **execCmd cmd / args mapping**: For each \`execCmd\` entry, the generated code must call \`execCmd\` with the same \`cmd\` literal and the same \`args\` shape. Values that came from \`run()\`'s \`input\` (e.g. URLs the user supplies) must be parameterized through \`input\`; constant args remain string literals.
5. **Output parsing in plain JS**: When the code consumes an \`execCmd\` result (JSON parsing, splitting, regex, field extraction), it must do so with plain JavaScript outside the primitive call. Do not delegate parsing to \`callLlm\`.
6. **finalAnswer → return**: The trailing \`finalAnswer.input.result\` corresponds to the value returned from \`run()\`. Construct the return value deterministically from intermediate variables (or return a string literal when the signature shows it is constant). Do not call \`callLlm\` to construct the return value.
7. **Do not hardcode observed outputs**: The \`output\` field of a \`callLlm\` or \`execCmd\` entry is one past observation, not a fixed answer. Re-invoke the primitive at runtime so future executions re-derive it.
8. **No alternate termination**: The only function exit is the \`return\` corresponding to \`finalAnswer\`. Do not introduce throws or branches that bypass the recorded sequence.
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
  const response = await callSubmitTool(client, model, SYSTEM_PROMPT, prompt);
  return extractGenerated(response);
}

export async function generateCodeFromSignature(
  prompt: string,
  signature: SignatureEntry[],
  model: string,
  apiKey: string,
): Promise<Generated> {
  if (!apiKey) {
    throw 'spec-violation: api-key argument is empty';
  }
  if (signature.length === 0) {
    throw 'spec-violation: signature is empty';
  }

  const client = new Anthropic({ apiKey });
  const userContent =
    `<task>\n${prompt}\n</task>\n\n` +
    `<signature>\n${JSON.stringify(signature, null, 2)}\n</signature>\n`;

  const response = await callSubmitTool(
    client,
    model,
    SIGNATURE_SYSTEM_PROMPT,
    userContent,
  );
  return extractGenerated(response);
}

async function callSubmitTool(
  client: Anthropic,
  model: string,
  system: string,
  userContent: string,
): Promise<MessagesCreateResponse> {
  try {
    return await client.messages.create({
      model,
      max_tokens: 4096,
      system,
      tools: [SUBMIT_TOOL],
      tool_choice: TOOL_CHOICE,
      messages: [{ role: 'user', content: userContent }],
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

function extractGenerated(response: MessagesCreateResponse): Generated {
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
