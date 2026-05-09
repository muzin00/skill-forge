import { loopLlm } from './loopLlm.js';

const DEFAULT_MAX_ITERATIONS = 15;

export interface DefineTaskOpts {
  allowSkills: string[];
  maxIterations?: number;
}

export function defineTask<TInput = unknown, TOutput = unknown>(
  opts: DefineTaskOpts,
): void {
  defineSkill<TInput, TOutput>(async (input: TInput): Promise<TOutput> => {
    const schema = getRegisteredSchema();
    if (!schema || !schema.output) {
      throw new Error(
        'defineTask requires defineSchema to be called with both inputSchema and outputSchema',
      );
    }
    const prompt = getInstruction();
    if (!prompt) {
      throw new Error(
        'defineTask requires INSTRUCTION.md alongside skill.js (getInstruction() returned empty string)',
      );
    }
    return loopLlm<TOutput>(prompt, {
      context: JSON.stringify(input),
      allowSkills: opts.allowSkills,
      outputSchema: schema.output,
      maxIterations: opts.maxIterations ?? DEFAULT_MAX_ITERATIONS,
    });
  });
}
