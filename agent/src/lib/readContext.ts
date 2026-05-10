declare const execCmd: (cmd: string, args: string[]) => Promise<string>;

export interface ReadContextInput {
  value: string;
}

export interface ReadContextOutput {
  content: string;
  source: 'literal' | 'file';
  sourcePath?: string;
}

export async function readContext(
  input: ReadContextInput,
): Promise<ReadContextOutput> {
  const { value } = input;
  if (!value.startsWith('@')) {
    return { content: value, source: 'literal' };
  }
  const path = value.slice(1);
  if (path === '') {
    throw new Error('read-context: empty path after "@"');
  }
  const content = await execCmd('cat', [path]);
  return { content, source: 'file', sourcePath: path };
}
