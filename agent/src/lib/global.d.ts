declare function defineSkill<TInput, TOutput>(
  run: (input: TInput) => Promise<TOutput>,
): void;

declare function defineSchema(schema: Record<string, unknown>): void;

declare function invokeSkill<TOutput = unknown>(
  name: string,
  input?: unknown,
): Promise<TOutput>;
