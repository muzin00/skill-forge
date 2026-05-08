declare const execCmd: (cmd: string, args: string[]) => Promise<string>;

export interface GrepFileInput {
  query: string;
  maxHits?: number;
}

const DEFAULT_MAX_HITS = 20;

export async function grepFile(input: GrepFileInput): Promise<string> {
  const { query } = input;
  const maxHits = input.maxHits ?? DEFAULT_MAX_HITS;

  let hits: string[] = [];
  try {
    const out = await execCmd('git', ['grep', '-n', '-F', query]);
    hits = out.split('\n').filter((l) => l.length > 0);
  } catch {
    hits = [];
  }

  if (hits.length === 0) {
    return `no hits for ${query}`;
  }

  const head = hits.slice(0, maxHits);
  const remainder = hits.length - head.length;
  return head.join('\n') + (remainder > 0 ? `\n...(${remainder} more hits)` : '');
}
