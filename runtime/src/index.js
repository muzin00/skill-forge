import { getSource } from 'skill-forge:runtime/skill-loader-host';

let skillModule = null;

function loadSkill() {
  if (skillModule !== null) return skillModule;

  const src = getSource();
  const factory = new Function(`
    const exports = {};
    ${src}
    return exports;
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

export function run(argsJson) {
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
    result = mod.run(args);
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

export function extractSchemas() {
  let mod;
  try {
    mod = loadSkill();
  } catch (e) {
    rethrow(e, 'schema-extract-error');
  }

  if (!mod.inputs || typeof mod.inputs !== 'object') {
    throw {
      code: 'schema-extract-error',
      message: 'missing or invalid `inputs` export',
      stack: undefined,
    };
  }
  if (!mod.output || typeof mod.output !== 'object') {
    throw {
      code: 'schema-extract-error',
      message: 'missing or invalid `output` export',
      stack: undefined,
    };
  }

  let inputs, output;
  try {
    inputs = JSON.stringify(mod.inputs);
    output = JSON.stringify(mod.output);
  } catch (e) {
    throw {
      code: 'schema-extract-error',
      message: `failed to stringify schema: ${e?.message ?? String(e)}`,
      stack: e?.stack ?? undefined,
    };
  }

  return { inputs, output };
}
