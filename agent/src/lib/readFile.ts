declare const execCmd: (cmd: string, args: string[]) => Promise<string>;

export interface ReadFileInput {
  path: string;
  lineStart?: number;
  lineEnd?: number;
}

const MAX_BYTES = 6000;
const MAX_LINES_FULL = 200;

export async function readFile(input: ReadFileInput): Promise<string> {
  const { path, lineStart, lineEnd } = input;

  let exists = false;
  try {
    await execCmd('test', ['-e', path]);
    exists = true;
  } catch {
    exists = false;
  }
  if (!exists) {
    return `ERROR: file not found: ${path}`;
  }

  if (lineStart !== undefined && lineEnd !== undefined) {
    try {
      const out = await execCmd('sed', ['-n', `${lineStart},${lineEnd}p`, path]);
      return numberLines(out.split('\n'), lineStart);
    } catch (e) {
      return `ERROR: ${errorMessage(e)}`;
    }
  }

  try {
    const out = await execCmd('cat', [path]);
    if (out.length > MAX_BYTES) {
      const lines = out.split('\n').slice(0, MAX_LINES_FULL);
      return (
        numberLines(lines, 1) +
        `\n...(truncated, file is ${out.length} chars / ${out.split('\n').length} lines)`
      );
    }
    return numberLines(out.split('\n'), 1);
  } catch (e) {
    return `ERROR: ${errorMessage(e)}`;
  }
}

function numberLines(lines: string[], startLine: number): string {
  return lines
    .map((line, i) => `${String(startLine + i).padStart(4, ' ')}  ${line}`)
    .join('\n');
}

function errorMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  return String(e);
}
