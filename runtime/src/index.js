import { getSource } from 'skill-forge:runtime/skill-loader-host';
import { callLlm as hostCallLlm } from 'skill-forge:runtime/llm-host';
import { execCmd as hostExecCmd } from 'skill-forge:runtime/exec-host';

globalThis.callLlm = async function callLlm(prompt, input) {
  return hostCallLlm(prompt, JSON.stringify(input ?? {}));
};

globalThis.execCmd = async function execCmd(cmd, args) {
  return hostExecCmd(cmd, args);
};

let skillModule = null;

function loadSkill() {
  if (skillModule !== null) return skillModule;

  const src = getSource();
  const factory = new Function(`
    ${src}
    return { run: typeof run === 'function' ? run : undefined };
  `);
  const mod = factory();

  if (typeof mod !== 'object' || mod === null) {
    throw skillError('runtime-error', 'skill module did not produce exports');
  }

  skillModule = mod;
  return skillModule;
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
      message: 'skill must export a `run` function',
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
