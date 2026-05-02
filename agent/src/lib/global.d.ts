declare function defineSkill<TInput, TOutput>(
  run: (input: TInput) => Promise<TOutput>,
): void;
