import {
  getSource,
  getDescription as hostGetDescription,
} from 'skill-forge:runtime/skill-loader-host';
import {
  getSchemaSource,
  getInputSchemaJson as hostGetInputSchemaJson,
} from 'skill-forge:runtime/schema-loader-host';
import { callLlm as hostCallLlm } from 'skill-forge:runtime/llm-host';
import { execCmd as hostExecCmd } from 'skill-forge:runtime/exec-host';
import { messages as hostAnthropicMessages } from 'skill-forge:runtime/anthropic-host';
import { invoke as hostInvoke } from 'skill-forge:runtime/invoke-host';
import { log as hostLog } from 'skill-forge:runtime/log-host';
import { getInstruction as hostGetInstruction } from 'skill-forge:runtime/instruction-loader-host';
import type { ErrorCode } from './generated/interfaces/skill-forge-runtime-types.js';

interface SkillErrorPayload {
  code: ErrorCode;
  message: string;
  stack?: string;
}

interface SkillRuntimeError extends Error {
  payload: SkillErrorPayload;
}

const g = globalThis as unknown as Record<string, unknown>;

g.callLlm = async function callLlm(
  prompt: string,
  input?: unknown,
): Promise<string> {
  return hostCallLlm(prompt, JSON.stringify(input ?? {}));
};

g.execCmd = async function execCmd(
  cmd: string,
  args: string[],
): Promise<string> {
  return hostExecCmd(cmd, args);
};

g.log = function log(message: string): void {
  hostLog(message);
};

g.getInstruction = function getInstruction(): string {
  return hostGetInstruction();
};

g.anthropicMessages = function anthropicMessages(bodyJson: string): string {
  return hostAnthropicMessages(bodyJson);
};

g.getSkillDescription = function getSkillDescription(skillName: string): string {
  return hostGetDescription(skillName);
};

g.getSkillInputSchema = function getSkillInputSchema(
  skillName: string,
): Record<string, unknown> {
  const json = hostGetInputSchemaJson(skillName);
  return JSON.parse(json);
};

g.invokeSkill = async function invokeSkill(
  name: string,
  input?: unknown,
): Promise<unknown> {
  const argsJson = JSON.stringify(input ?? {});
  let resultJson: string;
  try {
    resultJson = hostInvoke(name, argsJson);
  } catch (e) {
    const payload = (e as { payload?: Partial<SkillErrorPayload> })?.payload ?? {};
    const err = new Error(
      payload.message ?? (e as Error)?.message ?? String(e),
    ) as Error & { code?: string };
    err.code = payload.code;
    if (payload.stack) err.stack = payload.stack;
    throw err;
  }
  return JSON.parse(resultJson);
};

let __registered__: ((input: unknown) => Promise<unknown>) | undefined;
let __registered_schema__:
  | {
      input: Record<string, unknown>;
      output: Record<string, unknown> | null;
    }
  | undefined;
let __registered_args__: Record<string, unknown> | undefined;

Object.defineProperty(globalThis, 'defineSkill', {
  value: function defineSkill(runFn: (input: unknown) => Promise<unknown>) {
    if (typeof runFn !== 'function') {
      throw skillError(
        'runtime-error',
        `defineSkill argument must be a function, got ${runFn === null ? 'null' : typeof runFn}`,
      );
    }
    if (__registered__ !== undefined) {
      throw skillError('runtime-error', 'defineSkill called more than once');
    }
    __registered__ = runFn;
  },
  writable: false,
  configurable: false,
});

Object.defineProperty(globalThis, 'getRegisteredSchema', {
  value: function getRegisteredSchema() {
    if (__registered_schema__ === undefined) {
      loadSchema();
    }
    if (__registered_schema__ === undefined) return undefined;
    return {
      input: __registered_schema__.input,
      output: __registered_schema__.output,
      args: __registered_args__ ?? null,
    };
  },
  writable: false,
  configurable: false,
});

Object.defineProperty(globalThis, 'defineArgs', {
  value: function defineArgs(args: unknown) {
    if (args === null || typeof args !== 'object' || Array.isArray(args)) {
      const got =
        args === null ? 'null' : Array.isArray(args) ? 'array' : typeof args;
      throw skillError(
        'runtime-error',
        `defineArgs argument must be an object, got ${got}`,
      );
    }
    if (__registered_args__ !== undefined) {
      throw skillError('runtime-error', 'defineArgs called more than once');
    }
    __registered_args__ = args as Record<string, unknown>;
  },
  writable: false,
  configurable: false,
});

Object.defineProperty(globalThis, 'defineSchema', {
  value: function defineSchema(
    inputSchema: unknown,
    outputSchema?: unknown,
  ) {
    if (
      inputSchema === null ||
      typeof inputSchema !== 'object' ||
      Array.isArray(inputSchema)
    ) {
      const got =
        inputSchema === null
          ? 'null'
          : Array.isArray(inputSchema)
            ? 'array'
            : typeof inputSchema;
      throw skillError(
        'runtime-error',
        `defineSchema argument must be an object, got ${got}`,
      );
    }
    if (outputSchema !== undefined && outputSchema !== null) {
      if (typeof outputSchema !== 'object' || Array.isArray(outputSchema)) {
        const got = Array.isArray(outputSchema) ? 'array' : typeof outputSchema;
        throw skillError(
          'runtime-error',
          `defineSchema output argument must be an object, got ${got}`,
        );
      }
    }
    if (__registered_schema__ !== undefined) {
      throw skillError('runtime-error', 'defineSchema called more than once');
    }
    __registered_schema__ = {
      input: inputSchema as Record<string, unknown>,
      output: (outputSchema ?? null) as Record<string, unknown> | null,
    };
  },
  writable: false,
  configurable: false,
});

const DEFAULT_TASK_MAX_ITERATIONS = 15;

Object.defineProperty(globalThis, 'defineTask', {
  value: function defineTask(opts: {
    allowSkills: string[];
    maxIterations?: number;
  }) {
    defineSkill(async (input: unknown): Promise<unknown> => {
      const schema = getRegisteredSchema();
      if (!schema || !schema.output) {
        throw skillError(
          'runtime-error',
          'defineTask requires defineSchema to be called with both inputSchema and outputSchema',
        );
      }
      const prompt = getInstruction();
      if (!prompt) {
        throw skillError(
          'runtime-error',
          'defineTask requires INSTRUCTION.md alongside skill.js (getInstruction() returned empty string)',
        );
      }
      return invokeSkill('loop-llm', {
        prompt,
        context: JSON.stringify(input),
        allowSkills: opts.allowSkills,
        outputSchema: schema.output,
        maxIterations: opts.maxIterations ?? DEFAULT_TASK_MAX_ITERATIONS,
      });
    });
  },
  writable: false,
  configurable: false,
});

let skillModule: { run: (input: unknown) => Promise<unknown> } | null = null;
let schemaModule: {
  schema: { input: Record<string, unknown>; output: Record<string, unknown> | null };
  args: Record<string, unknown> | null;
} | null = null;

function loadSkill() {
  if (skillModule !== null) return skillModule;

  __registered__ = undefined;
  const src = getSource();
  const factory = new Function(src);
  factory();

  if (typeof __registered__ !== 'function') {
    throw skillError(
      'runtime-error',
      'skill must call defineSkill(async (input) => { ... }) at top level',
    );
  }

  skillModule = { run: __registered__ };
  return skillModule;
}

function loadSchema() {
  if (schemaModule !== null) return schemaModule;

  __registered_schema__ = undefined;
  __registered_args__ = undefined;
  const src = getSchemaSource();
  const factory = new Function(src);
  factory();

  if (__registered_schema__ === undefined) {
    throw skillError(
      'runtime-error',
      'skill must call defineSchema({ ... }) at top level of schema.js',
    );
  }

  schemaModule = {
    schema: __registered_schema__,
    args: __registered_args__ ?? null,
  };
  return schemaModule;
}

function skillError(
  code: ErrorCode,
  message: string,
  stack?: string,
): SkillRuntimeError {
  const e = new Error(message) as SkillRuntimeError;
  e.payload = { code, message, stack };
  return e;
}

function rethrow(e: unknown, defaultCode: ErrorCode): never {
  const payload = (e as { payload?: SkillErrorPayload })?.payload;
  if (payload && typeof payload.code === 'string') throw payload;
  throw {
    code: defaultCode,
    message: (e as Error)?.message ?? String(e),
    stack: (e as Error)?.stack,
  };
}

export async function run(argsJson: string): Promise<string> {
  let mod;
  try {
    mod = loadSkill();
  } catch (e) {
    rethrow(e, 'runtime-error');
  }

  if (typeof mod.run !== 'function') {
    throw {
      code: 'runtime-error' as ErrorCode,
      message: 'skill did not register a run function via defineSkill',
      stack: undefined,
    };
  }

  let args;
  try {
    args = JSON.parse(argsJson);
  } catch (e) {
    throw {
      code: 'runtime-error' as ErrorCode,
      message: `failed to parse args JSON: ${(e as Error)?.message ?? String(e)}`,
      stack: (e as Error)?.stack,
    };
  }

  let result: unknown;
  try {
    result = await mod.run(args);
  } catch (e) {
    throw {
      code: 'user-error' as ErrorCode,
      message: (e as Error)?.message ?? String(e),
      stack: (e as Error)?.stack,
    };
  }

  let json;
  try {
    json = JSON.stringify(result);
  } catch (e) {
    throw {
      code: 'runtime-error' as ErrorCode,
      message: `failed to stringify result: ${(e as Error)?.message ?? String(e)}`,
      stack: (e as Error)?.stack,
    };
  }

  if (json === undefined) {
    throw {
      code: 'runtime-error' as ErrorCode,
      message: 'result is not JSON-serializable (undefined)',
      stack: undefined,
    };
  }

  return json;
}

export function getSchema(): string {
  let mod;
  try {
    mod = loadSchema();
  } catch (e) {
    rethrow(e, 'runtime-error');
  }

  const envelope = {
    input: mod.schema.input,
    output: mod.schema.output,
    args: mod.args,
  };

  let json;
  try {
    json = JSON.stringify(envelope);
  } catch (e) {
    throw {
      code: 'runtime-error' as ErrorCode,
      message: `failed to stringify schema: ${(e as Error)?.message ?? String(e)}`,
      stack: (e as Error)?.stack,
    };
  }

  if (json === undefined) {
    throw {
      code: 'runtime-error' as ErrorCode,
      message: 'schema is not JSON-serializable',
      stack: undefined,
    };
  }

  return json;
}
