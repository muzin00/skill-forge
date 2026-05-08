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

globalThis.callLlm = async function callLlm(prompt, input) {
  return hostCallLlm(prompt, JSON.stringify(input ?? {}));
};

globalThis.execCmd = async function execCmd(cmd, args) {
  return hostExecCmd(cmd, args);
};

globalThis.anthropicMessages = function anthropicMessages(bodyJson) {
  return hostAnthropicMessages(bodyJson);
};

globalThis.getSkillDescription = function getSkillDescription(skillName) {
  return hostGetDescription(skillName);
};

globalThis.getSkillInputSchema = function getSkillInputSchema(skillName) {
  const json = hostGetInputSchemaJson(skillName);
  return JSON.parse(json);
};

globalThis.invokeSkill = async function invokeSkill(name, input) {
  const argsJson = JSON.stringify(input ?? {});
  let resultJson;
  try {
    resultJson = hostInvoke(name, argsJson);
  } catch (e) {
    const payload = e?.payload ?? {};
    const err = new Error(payload.message ?? e?.message ?? String(e));
    err.code = payload.code;
    if (payload.stack) err.stack = payload.stack;
    throw err;
  }
  return JSON.parse(resultJson);
};

let __registered__;
let __registered_schema__;

Object.defineProperty(globalThis, 'defineSkill', {
  value: function defineSkill(runFn) {
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

Object.defineProperty(globalThis, 'defineSchema', {
  value: function defineSchema(inputSchema, outputSchema) {
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
      input: inputSchema,
      output: outputSchema ?? null,
    };
  },
  writable: false,
  configurable: false,
});

let skillModule = null;
let schemaModule = null;

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
  const src = getSchemaSource();
  const factory = new Function(src);
  factory();

  if (__registered_schema__ === undefined) {
    throw skillError(
      'runtime-error',
      'skill must call defineSchema({ ... }) at top level of schema.js',
    );
  }

  schemaModule = { schema: __registered_schema__ };
  return schemaModule;
}

function skillError(code, message, stack) {
  const e = new Error(message);
  e.payload = { code, message, stack: stack ?? undefined };
  return e;
}

function rethrow(e, defaultCode) {
  if (e && e.payload && typeof e.payload.code === 'string') throw e.payload;
  throw {
    code: defaultCode,
    message: e?.message ?? String(e),
    stack: e?.stack ?? undefined,
  };
}

export async function run(argsJson) {
  let mod;
  try {
    mod = loadSkill();
  } catch (e) {
    rethrow(e, 'runtime-error');
  }

  if (typeof mod.run !== 'function') {
    throw {
      code: 'runtime-error',
      message: 'skill did not register a run function via defineSkill',
      stack: undefined,
    };
  }

  let args;
  try {
    args = JSON.parse(argsJson);
  } catch (e) {
    throw {
      code: 'runtime-error',
      message: `failed to parse args JSON: ${e?.message ?? String(e)}`,
      stack: e?.stack ?? undefined,
    };
  }

  let result;
  try {
    result = await mod.run(args);
  } catch (e) {
    throw {
      code: 'user-error',
      message: e?.message ?? String(e),
      stack: e?.stack ?? undefined,
    };
  }

  let json;
  try {
    json = JSON.stringify(result);
  } catch (e) {
    throw {
      code: 'runtime-error',
      message: `failed to stringify result: ${e?.message ?? String(e)}`,
      stack: e?.stack ?? undefined,
    };
  }

  if (json === undefined) {
    throw {
      code: 'runtime-error',
      message: 'result is not JSON-serializable (undefined)',
      stack: undefined,
    };
  }

  return json;
}

export function getSchema() {
  let mod;
  try {
    mod = loadSchema();
  } catch (e) {
    rethrow(e, 'runtime-error');
  }

  let json;
  try {
    json = JSON.stringify(mod.schema);
  } catch (e) {
    throw {
      code: 'runtime-error',
      message: `failed to stringify schema: ${e?.message ?? String(e)}`,
      stack: e?.stack ?? undefined,
    };
  }

  if (json === undefined) {
    throw {
      code: 'runtime-error',
      message: 'schema is not JSON-serializable',
      stack: undefined,
    };
  }

  return json;
}
