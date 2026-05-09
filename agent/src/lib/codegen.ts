import {
  Anthropic,
  AnthropicAPIError,
  type ContentBlock,
  type MessagesCreateResponse,
  type RequestContentBlock,
  type ToolDefinition,
} from './anthropic-client.js';
import SYSTEM_PROMPT from './SYSTEM_PROMPT.md';

declare const callLlm: (prompt: string, input?: unknown) => Promise<string>;
declare const execCmd: (cmd: string, args: string[]) => Promise<string>;

export interface Generated {
  code: string;
  capabilities: string[];
  schema: Record<string, unknown>;
  instruction: string;
}

const ALLOWED_CAPABILITIES = ['callLlm', 'execCmd'] as const;
type AllowedCapability = (typeof ALLOWED_CAPABILITIES)[number];

const MAX_ITERATIONS = 16;
const MAX_TOKENS = 4096;

const TOOLS: ToolDefinition[] = [
  {
    name: 'callLlm',
    description:
      'callLlm host primitive を実行する。非決定的な LLM 判断 (分類・要約・自然言語生成) が必要なときだけ使う。',
    input_schema: {
      type: 'object',
      properties: {
        prompt: {
          type: 'string',
          description: 'LLM に与える system prompt 相当の指示文。',
        },
        input: {
          description:
            'LLM に渡す構造化入力 (任意)。文字列・数値・配列・オブジェクトいずれも可。',
        },
      },
      required: ['prompt'],
    },
  },
  {
    name: 'execCmd',
    description:
      'execCmd host primitive を実行する。ホスト環境を調査するため、CLI の存在確認 (which) や --help 出力の取得など、副作用の小さい read-only コマンドに限定する。',
    input_schema: {
      type: 'object',
      properties: {
        cmd: { type: 'string', description: '実行するコマンド名。' },
        args: {
          type: 'array',
          items: { type: 'string' },
          description: 'コマンドへ渡す引数の配列。',
        },
      },
      required: ['cmd', 'args'],
    },
  },
  {
    name: 'submit_generated_code',
    description:
      '生成したコードと、その実行に必要な host primitive を提出する。1 タスクにつき必ず 1 度だけ呼び、これを呼んだ時点で session を終了する。',
    input_schema: {
      type: 'object',
      properties: {
        code: {
          type: 'string',
          description:
            'skill-runtime 上で実行される JS コード本体。トップレベルで defineSkill(async (input) => { ... }) を 1 度だけ呼び出す。',
        },
        capabilities: {
          type: 'array',
          items: { type: 'string', enum: [...ALLOWED_CAPABILITIES] },
          description: 'code が実際に呼び出す host primitive のリスト。',
        },
        schema: {
          type: 'object',
          description:
            '生成した skill が受け取る input オブジェクトの JSON Schema。type は "object" 固定、additionalProperties は false、properties のキー集合は code が input から取り出すキーと完全一致させる。',
        },
        instruction: {
          type: 'string',
          description:
            '生成した skill が実行時に内部 LLM へ渡す指示文。skill が内部で LLM を呼ばない場合は空文字列。生成コードからは getInstruction() で参照する。',
        },
      },
      required: ['code', 'capabilities', 'schema', 'instruction'],
      additionalProperties: false,
    },
  },
];

export async function generateSkillCode(
  prompt: string,
  model: string,
): Promise<Generated> {
  const client = new Anthropic();
  const messages: Array<{
    role: 'user' | 'assistant';
    content: string | RequestContentBlock[];
  }> = [{ role: 'user', content: prompt }];

  for (let iteration = 0; iteration < MAX_ITERATIONS; iteration++) {
    const response = await callMessages(client, model, messages);

    if (response.stop_reason !== 'tool_use') {
      throw `spec-violation: expected stop_reason "tool_use" but got "${response.stop_reason}"`;
    }

    const submitBlock = response.content.find(
      (b): b is Extract<ContentBlock, { type: 'tool_use' }> =>
        b.type === 'tool_use' && b.name === 'submit_generated_code',
    );
    if (submitBlock) {
      return extractGenerated(submitBlock);
    }

    messages.push({ role: 'assistant', content: response.content });

    const toolResults: RequestContentBlock[] = [];
    for (const block of response.content) {
      if (block.type !== 'tool_use') continue;
      toolResults.push(await dispatchTool(block));
    }

    if (toolResults.length === 0) {
      throw 'spec-violation: assistant returned tool_use stop reason without tool_use blocks';
    }

    messages.push({ role: 'user', content: toolResults });
  }

  throw `loop-exceeded: agent did not call submit_generated_code within ${MAX_ITERATIONS} iterations`;
}

async function callMessages(
  client: Anthropic,
  model: string,
  messages: Array<{
    role: 'user' | 'assistant';
    content: string | RequestContentBlock[];
  }>,
): Promise<MessagesCreateResponse> {
  try {
    return await client.messages.create({
      model,
      max_tokens: MAX_TOKENS,
      system: SYSTEM_PROMPT,
      tools: TOOLS,
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
): Promise<RequestContentBlock> {
  const input =
    typeof block.input === 'object' && block.input !== null && !Array.isArray(block.input)
      ? (block.input as Record<string, unknown>)
      : null;

  try {
    if (input === null) {
      throw `tool input is not an object`;
    }
    if (block.name === 'callLlm') {
      const promptArg = input.prompt;
      if (typeof promptArg !== 'string') {
        throw 'callLlm requires string "prompt"';
      }
      const result = await callLlm(promptArg, input.input);
      return { type: 'tool_result', tool_use_id: block.id, content: result };
    }
    if (block.name === 'execCmd') {
      const cmdArg = input.cmd;
      const argsArg = input.args;
      if (typeof cmdArg !== 'string') {
        throw 'execCmd requires string "cmd"';
      }
      if (!Array.isArray(argsArg) || !argsArg.every((a) => typeof a === 'string')) {
        throw 'execCmd requires string[] "args"';
      }
      const result = await execCmd(cmdArg, argsArg);
      return { type: 'tool_result', tool_use_id: block.id, content: result };
    }
    throw `unknown tool "${block.name}"`;
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

function extractGenerated(
  toolUse: Extract<ContentBlock, { type: 'tool_use' }>,
): Generated {
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

  const schemaRaw = inputObj.schema;
  if (typeof schemaRaw !== 'object' || schemaRaw === null || Array.isArray(schemaRaw)) {
    throw 'spec-violation: tool_use input is missing object field "schema"';
  }
  const schema = schemaRaw as Record<string, unknown>;

  const instructionRaw = inputObj.instruction;
  if (typeof instructionRaw !== 'string') {
    throw 'spec-violation: tool_use input is missing string field "instruction"';
  }
  const instruction = instructionRaw;

  return { code, capabilities, schema, instruction };
}

function isAllowedCapability(value: string): value is AllowedCapability {
  return (ALLOWED_CAPABILITIES as readonly string[]).includes(value);
}
