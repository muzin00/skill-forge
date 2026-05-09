declare function defineSkill<TInput, TOutput>(
  run: (input: TInput) => Promise<TOutput>,
): void;

declare function defineSchema(
  inputSchema: Record<string, unknown>,
  outputSchema?: Record<string, unknown>,
): void;

declare function defineArgs(args: { positional?: string }): void;

declare function invokeSkill<TOutput = unknown>(
  name: string,
  input?: unknown,
): Promise<TOutput>;

declare function execCmd(cmd: string, args: string[]): Promise<string>;

declare function log(message: string): void;

declare function getRegisteredSchema():
  | {
      input: Record<string, unknown>;
      output: Record<string, unknown> | null;
      args: Record<string, unknown> | null;
    }
  | undefined;

declare module '*.md' {
  const content: string;
  export default content;
}
