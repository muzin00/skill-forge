import {
  Anthropic,
  AnthropicAPIError,
  type ContentBlock,
  type RequestContentBlock,
  type ToolDefinition,
  type ToolChoice,
} from './anthropic-client.js';
import { callLlm as callLlmImpl } from './llm.js';

declare const globalThis: {
  execCmd: (cmd: string, args: string[]) => Promise<string>;
};

export interface SignatureEntry {
  tool: string;
  input: string;
  output: string;
}

export interface Interpreted {
  finalAnswer: string;
  signature: SignatureEntry[];
}

const MAX_ITERATIONS = 10;
const MAX_TOKENS = 4096;

const SYSTEM_PROMPT = `You are an interpretation-loop agent for skill-forge.

You receive a natural-language task from the user. To accomplish it, you call tools in a loop. The host records every tool call as a "signature" — the ordered sequence of (tool, input, output) tuples that achieves the task. After this PoC, signatures will be crystalized into static code, so structure your tool calls as if they were ordinary function calls.

# Tools

- \`callLlm(prompt: string, input?: object): string\` — Delegate a non-deterministic transformation (classification, summarization, translation, free-form generation, judgement) to an LLM. The host invokes Claude with \`prompt\` as the system instruction and \`JSON.stringify(input ?? {})\` as the user message, and returns the concatenated text response. Use this whenever the work cannot be reduced to a deterministic procedure.
- \`execCmd(cmd: string, args: string[]): string\` — Run an external command on the host and return its stdout as a string. Use this for deterministic external lookups (e.g. \`gh issue view\`, \`git log\`, file reads via \`cat\`). Choose this over \`callLlm\` when the work is a concrete, repeatable command.
- \`finalAnswer(result: string)\` — Submit the final user-facing answer and end the loop. Call exactly once, after all sub-tasks are complete.

# Rules

- Every turn must call a tool. Free-form text responses are forbidden.
- Decompose the task into a sequence of tool calls. Each \`callLlm\` call should encapsulate one self-contained sub-task; do not bundle independent sub-tasks into a single prompt.
- This loop observes only tool calls. Deterministic logic (conditionals, string manipulation, arithmetic) cannot be expressed in the loop directly — wrap such steps inside a \`callLlm\` call as well, OR delegate to \`execCmd\` when the deterministic step is naturally a shell command.
- Prefer \`execCmd\` over \`callLlm\` when the sub-task is a concrete external invocation (CLI tool, system command). Reserve \`callLlm\` for true natural-language judgement.
- When all sub-tasks are complete, call \`finalAnswer\` with the final result string.
`;

const TOOLS: ToolDefinition[] = [
  {
    name: 'callLlm',
    description:
      '自然言語入力を LLM に投げて自然言語の応答を得る。判断・分類・要約・翻訳など、決定論的に書けない処理に使う。',
    input_schema: {
      type: 'object',
      properties: {
        prompt: { type: 'string' },
        input: { type: 'object', additionalProperties: true },
      },
      required: ['prompt'],
      additionalProperties: false,
    },
  },
  {
    name: 'execCmd',
    description:
      '外部コマンドを実行し、stdout を文字列として返す。GitHub CLI 呼び出し・ファイル取得など、決定論的に書ける外部操作に使う。',
    input_schema: {
      type: 'object',
      properties: {
        cmd: { type: 'string' },
        args: { type: 'array', items: { type: 'string' } },
      },
      required: ['cmd', 'args'],
      additionalProperties: false,
    },
  },
  {
    name: 'finalAnswer',
    description:
      'ユーザーへの最終回答を提出してループを終了する。1 タスクにつき必ず 1 度だけ呼ぶ。',
    input_schema: {
      type: 'object',
      properties: {
        result: {
          type: 'string',
          description: 'ユーザーへの最終出力。',
        },
      },
      required: ['result'],
      additionalProperties: false,
    },
  },
];

const TOOL_CHOICE: ToolChoice = { type: 'any' };

type ToolUseBlock = Extract<ContentBlock, { type: 'tool_use' }>;

export async function interpret(
  prompt: string,
  model: string,
  apiKey: string,
): Promise<Interpreted> {
  if (!apiKey) {
    throw 'spec-violation: api-key argument is empty';
  }

  const client = new Anthropic({ apiKey });
  const messages: Array<{
    role: 'user' | 'assistant';
    content: string | RequestContentBlock[];
  }> = [{ role: 'user', content: prompt }];
  const signature: SignatureEntry[] = [];

  for (let i = 0; i < MAX_ITERATIONS; i++) {
    let response;
    try {
      response = await client.messages.create({
        model,
        max_tokens: MAX_TOKENS,
        system: SYSTEM_PROMPT,
        tools: TOOLS,
        tool_choice: TOOL_CHOICE,
        messages,
      });
    } catch (e) {
      if (e instanceof AnthropicAPIError) {
        throw `api-error: HTTP ${e.status}: ${e.body}`;
      }
      throw `api-error: ${e instanceof Error ? e.message : String(e)}`;
    }

    if (response.stop_reason !== 'tool_use') {
      throw `spec-violation: expected stop_reason "tool_use" but got "${response.stop_reason}"`;
    }

    const toolUses = response.content.filter(
      (b): b is ToolUseBlock => b.type === 'tool_use',
    );
    if (toolUses.length === 0) {
      throw 'spec-violation: no tool_use blocks in response';
    }

    const toolResults: Array<{ tool_use_id: string; content: string }> = [];

    let finalResult: string | null = null;
    for (const toolUse of toolUses) {
      const inputJson = JSON.stringify(toolUse.input);

      if (toolUse.name === 'finalAnswer') {
        const result = extractFinalAnswerResult(toolUse.input);
        signature.push({ tool: 'finalAnswer', input: inputJson, output: '' });
        finalResult = result;
        break;
      }

      if (toolUse.name === 'callLlm') {
        const { callPrompt, callInput } = extractCallLlmArgs(toolUse.input);
        const output = await callLlmImpl(
          callPrompt,
          JSON.stringify(callInput ?? {}),
          model,
          apiKey,
        );
        signature.push({
          tool: 'callLlm',
          input: inputJson,
          output: JSON.stringify(output),
        });
        toolResults.push({ tool_use_id: toolUse.id, content: output });
        continue;
      }

      if (toolUse.name === 'execCmd') {
        const { cmd, cmdArgs } = extractExecCmdArgs(toolUse.input);
        let output: string;
        try {
          output = await globalThis.execCmd(cmd, cmdArgs);
        } catch (e) {
          throw `exec-error: ${e instanceof Error ? e.message : String(e)}`;
        }
        signature.push({
          tool: 'execCmd',
          input: inputJson,
          output: JSON.stringify(output),
        });
        toolResults.push({ tool_use_id: toolUse.id, content: output });
        continue;
      }

      throw `spec-violation: unknown tool "${toolUse.name}"`;
    }

    if (finalResult !== null) {
      return { finalAnswer: finalResult, signature };
    }

    messages.push({ role: 'assistant', content: response.content });
    messages.push({
      role: 'user',
      content: toolResults.map((tr) => ({
        type: 'tool_result' as const,
        tool_use_id: tr.tool_use_id,
        content: tr.content,
      })),
    });
  }

  throw 'spec-violation: max iterations exceeded';
}

function extractFinalAnswerResult(input: unknown): string {
  if (typeof input !== 'object' || input === null || Array.isArray(input)) {
    throw 'parse-error: finalAnswer tool_use input is not an object';
  }
  const result = (input as Record<string, unknown>).result;
  if (typeof result !== 'string') {
    throw 'spec-violation: finalAnswer tool_use input is missing string field "result"';
  }
  return result;
}

function extractExecCmdArgs(input: unknown): {
  cmd: string;
  cmdArgs: string[];
} {
  if (typeof input !== 'object' || input === null || Array.isArray(input)) {
    throw 'parse-error: execCmd tool_use input is not an object';
  }
  const obj = input as Record<string, unknown>;
  const cmd = obj.cmd;
  if (typeof cmd !== 'string') {
    throw 'spec-violation: execCmd tool_use input is missing string field "cmd"';
  }
  const rawArgs = obj.args;
  if (!Array.isArray(rawArgs)) {
    throw 'spec-violation: execCmd tool_use input is missing array field "args"';
  }
  const cmdArgs: string[] = [];
  for (const a of rawArgs) {
    if (typeof a !== 'string') {
      throw 'parse-error: execCmd tool_use input.args entry is not a string';
    }
    cmdArgs.push(a);
  }
  return { cmd, cmdArgs };
}

function extractCallLlmArgs(input: unknown): {
  callPrompt: string;
  callInput: Record<string, unknown> | undefined;
} {
  if (typeof input !== 'object' || input === null || Array.isArray(input)) {
    throw 'parse-error: callLlm tool_use input is not an object';
  }
  const obj = input as Record<string, unknown>;
  const callPrompt = obj.prompt;
  if (typeof callPrompt !== 'string') {
    throw 'spec-violation: callLlm tool_use input is missing string field "prompt"';
  }
  const rawInput = obj.input;
  if (rawInput === undefined) {
    return { callPrompt, callInput: undefined };
  }
  if (typeof rawInput !== 'object' || rawInput === null || Array.isArray(rawInput)) {
    throw 'parse-error: callLlm tool_use input.input is not an object';
  }
  return { callPrompt, callInput: rawInput as Record<string, unknown> };
}
