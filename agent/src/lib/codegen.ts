import {
  Anthropic,
  AnthropicAPIError,
  type ContentBlock,
  type MessagesCreateResponse,
  type ToolDefinition,
  type ToolChoice,
} from './anthropic-client.js';
import SYSTEM_PROMPT from './SYSTEM_PROMPT.md';

export interface Generated {
  code: string;
  capabilities: string[];
  schema: Record<string, unknown>;
}

const ALLOWED_CAPABILITIES = ['callLlm', 'execCmd'] as const;
type AllowedCapability = (typeof ALLOWED_CAPABILITIES)[number];

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
    },
    required: ['code', 'capabilities', 'schema'],
    additionalProperties: false,
  },
};

const TOOL_CHOICE: ToolChoice = { type: 'tool', name: 'submit_generated_code' };

export async function generateSkillCode(
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

  const schemaRaw = inputObj.schema;
  if (typeof schemaRaw !== 'object' || schemaRaw === null || Array.isArray(schemaRaw)) {
    throw 'spec-violation: tool_use input is missing object field "schema"';
  }
  const schema = schemaRaw as Record<string, unknown>;

  return { code, capabilities, schema };
}

function isAllowedCapability(value: string): value is AllowedCapability {
  return (ALLOWED_CAPABILITIES as readonly string[]).includes(value);
}
