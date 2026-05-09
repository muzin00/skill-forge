declare function defineSkill<TInput, TOutput>(
  run: (input: TInput) => Promise<TOutput>,
): void;

declare function defineSchema(
  inputSchema: Record<string, unknown>,
  outputSchema?: Record<string, unknown> | null,
): void;

declare function defineArgs(args: { positional?: string }): void;

declare function defineTask<TInput, TOutput>(opts: {
  allowSkills: string[];
  maxIterations?: number;
}): void;

declare function getRegisteredSchema():
  | {
      input: Record<string, unknown>;
      output: Record<string, unknown> | null;
      args: Record<string, unknown> | null;
    }
  | undefined;

declare function getInstruction(): string;

declare function invokeSkill<TOutput = unknown>(
  name: string,
  input?: unknown,
): Promise<TOutput>;
