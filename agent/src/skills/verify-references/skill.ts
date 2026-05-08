import {
  Anthropic,
  type RequestContentBlock,
  type ToolDefinition,
} from '../../lib/anthropic-client.js';

declare const execCmd: (cmd: string, args: string[]) => Promise<string>;

const MODEL = 'claude-sonnet-4-6';
const MAX_TOKENS = 4096;
const FINDINGS_MAX_ITERATIONS = 15;

type RefKind = 'file' | 'symbol' | 'line-range';

interface VerifyReferencesInput {
  text: string;
  repoRoot?: string;
  scope?: { paths?: string[]; excludePaths?: string[] };
  kinds?: RefKind[];
}

interface SourceRef {
  kind: RefKind;
  path?: string;
  lineStart?: number;
  lineEnd?: number;
  symbol?: string;
  context: string;
}

interface VerificationResult {
  ref: SourceRef;
  exists: boolean;
  snippet?: string;
  note?: string;
}

interface Finding {
  summary: string;
  evidence: string;
}

interface ToolHistoryEntry {
  tool: string;
  input: Record<string, unknown>;
  output: string;
  durationMs: number;
}

interface VerifyReferencesOutput {
  refs: SourceRef[];
  verifications: VerificationResult[];
  findings: Finding[];
  summary: { total: number; verified: number };
  toolHistory: ToolHistoryEntry[];
  iterations: number;
}

const EXTRACT_REFS_SYSTEM = `あなたは GitHub Issue 本文（または任意のテキスト）から「実装上の参照点」を抽出するアシスタントです。

以下のものをすべて漏れなく抽出してください:
- ファイルパス（例: src/main.rs, src/mcp/server.rs）→ kind=file
- 行番号レンジ付き参照（例: src/main.rs:119-123）→ kind=line-range, path, lineStart, lineEnd
- 関数名・型名・enum variant・定数名（例: host_call_llm, Backend::McpSampling, MAX_INVOKE_DEPTH）→ kind=symbol
- モジュール名（例: src/mcp/tools.rs の callLlm）→ kind=symbol（symbol だけ拾う）

context フィールドには、テキスト内のどの文脈で言及されているか・なぜ言及されているかを 1 文で要約してください。

同じ参照が複数回登場する場合は 1 つにまとめても構いません。
外部 URL（GitHub、ドキュメント等）は抽出対象外です。

必ず emit_refs ツールを 1 回だけ呼び出して結果を返してください。`;

const FINDINGS_SYSTEM = `あなたは「テキスト本文だけでは見えない補足観点」を抽出するアシスタントです。

テキスト本文と、本文内で言及された参照点に対するソースコード検証結果が与えられます。
verifications に含まれるのは「言及された参照点が実在するか」のみで、コードの中身までは見ていません。

read_file / grep_symbol ツールを使って実コードに踏み込み、以下のような事実を発見してください:

- 既存実装の構造が設計の前提を裏付けている事実（例: enum match の分岐構造）
- 想定衝突がコード上で起きないことの裏取り
- リファクタの足場となる関数が既に存在する事実
- 設計が前提とする状態が現状コードと矛盾する事実
- 依存ライブラリ・型システム上の制約（例: Clone か Arc か）

調査の進め方:
- verifications を出発点に、実コードを必要なだけ Read / Grep する
- 仮説を立て、それを裏付けるコードを探す → 観察 → 次の仮説 を繰り返す
- 各 finding には summary（1 文）と evidence（具体的なファイル名・行番号・スニペット）を含める

テキスト本文に既に書かれていることをそのまま繰り返すのは避けてください。

**重要: 終了条件**
- ツール呼び出しは **6〜10 回程度**で十分な material が得られるはずです。それ以上は情報の限界収益が逓減します。
- 「もう少し調べたい」と感じても、5-6 件の具体的な finding が手元に揃ったら **速やかに emit_findings を呼んで終了** してください。
- 完璧な網羅性より、確度の高い具体的な finding を優先してください。
- findings は 0 件でも構いません（特筆すべきことがなければ空配列で emit）。
- 必ず emit_findings を **ちょうど 1 回**呼び出して終了してください。`;

const EXTRACT_REFS_TOOL: ToolDefinition = {
  name: 'emit_refs',
  description: 'Emit the extracted source references.',
  input_schema: {
    type: 'object',
    properties: {
      refs: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            kind: { type: 'string', enum: ['file', 'symbol', 'line-range'] },
            path: { type: 'string' },
            lineStart: { type: 'integer' },
            lineEnd: { type: 'integer' },
            symbol: { type: 'string' },
            context: { type: 'string' },
          },
          required: ['kind', 'context'],
        },
      },
    },
    required: ['refs'],
  },
};

const FINDINGS_TOOLS: ToolDefinition[] = [
  {
    name: 'read_file',
    description:
      'Read a file (or a specific line range) from the repository. Use this to inspect actual code beyond what was already verified — e.g., to see surrounding context, type definitions, related code paths.',
    input_schema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Repo-relative file path' },
        lineStart: { type: 'integer', description: 'Starting line, 1-based (optional)' },
        lineEnd: { type: 'integer', description: 'Ending line, inclusive (optional)' },
      },
      required: ['path'],
    },
  },
  {
    name: 'grep_symbol',
    description:
      'Search for a symbol or substring across the repository using `git grep -F`. Returns matching `path:line: text` lines.',
    input_schema: {
      type: 'object',
      properties: {
        symbol: { type: 'string', description: 'Symbol or substring' },
        maxHits: { type: 'integer', description: 'Max hits to return (default 20)' },
      },
      required: ['symbol'],
    },
  },
  {
    name: 'emit_findings',
    description:
      'Emit the final list of supplementary findings. Call this exactly ONCE when you have enough evidence. After this call, do not call any more tools.',
    input_schema: {
      type: 'object',
      properties: {
        findings: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              summary: { type: 'string', description: '1-sentence summary of the finding' },
              evidence: {
                type: 'string',
                description: 'Concrete grounding (file name, line numbers, snippet)',
              },
            },
            required: ['summary', 'evidence'],
          },
        },
      },
      required: ['findings'],
    },
  },
];

defineSkill(
  async (input: VerifyReferencesInput): Promise<VerifyReferencesOutput> => {
    const repoRoot = input.repoRoot;
    const allowedKinds: RefKind[] = input.kinds ?? ['file', 'symbol', 'line-range'];
    const allowedKindSet = new Set<RefKind>(allowedKinds);
    const excludePaths = input.scope?.excludePaths ?? [];
    const includePaths = input.scope?.paths ?? [];

    const refs = await extractRefs(input.text);
    const filteredRefs = refs.filter((r) => allowedKindSet.has(r.kind));

    const verifications = await Promise.all(
      filteredRefs.map((r) => verifyRef(r, repoRoot, includePaths, excludePaths)),
    );

    const { findings, toolHistory, iterations } = await extractFindings(
      input.text,
      verifications,
      repoRoot,
      includePaths,
      excludePaths,
    );

    return {
      refs: filteredRefs,
      verifications,
      findings,
      summary: {
        total: verifications.length,
        verified: verifications.filter((v) => v.exists).length,
      },
      toolHistory,
      iterations,
    };
  },
);

async function extractRefs(body: string): Promise<SourceRef[]> {
  const client = new Anthropic();
  const res = await client.messages.create({
    model: MODEL,
    max_tokens: MAX_TOKENS,
    system: EXTRACT_REFS_SYSTEM,
    messages: [{ role: 'user', content: body }],
    tools: [EXTRACT_REFS_TOOL],
    tool_choice: { type: 'tool', name: 'emit_refs' },
  });

  for (const block of res.content) {
    if (block.type === 'tool_use' && block.name === 'emit_refs') {
      const inp = block.input as { refs?: SourceRef[] };
      return inp.refs ?? [];
    }
  }
  return [];
}

async function verifyRef(
  ref: SourceRef,
  repoRoot: string | undefined,
  includePaths: string[],
  excludePaths: string[],
): Promise<VerificationResult> {
  if (ref.kind === 'file') {
    if (!ref.path) return { ref, exists: false, note: 'file kind without path' };
    const exists = await fileExists(ref.path, repoRoot);
    return { ref, exists, note: exists ? 'file exists' : 'file not found' };
  }
  if (ref.kind === 'line-range') {
    if (!ref.path || ref.lineStart === undefined || ref.lineEnd === undefined) {
      return { ref, exists: false, note: 'line-range needs path and line numbers' };
    }
    const fileOk = await fileExists(ref.path, repoRoot);
    if (!fileOk) return { ref, exists: false, note: `file not found: ${ref.path}` };
    try {
      const slice = await readLineRange(ref.path, ref.lineStart, ref.lineEnd, repoRoot);
      const lineCount = ref.lineEnd - ref.lineStart + 1;
      if (slice.split('\n').filter((l) => l.length > 0).length === 0 && lineCount > 0) {
        return {
          ref,
          exists: false,
          note: `${ref.path}:${ref.lineStart}-${ref.lineEnd} out of range`,
        };
      }
      return {
        ref,
        exists: true,
        snippet: truncate(slice, 800),
        note: `${ref.path}:${ref.lineStart}-${ref.lineEnd} found`,
      };
    } catch (e) {
      return { ref, exists: false, note: errorMessage(e) };
    }
  }
  if (ref.kind === 'symbol') {
    if (!ref.symbol) return { ref, exists: false, note: 'symbol kind without symbol' };
    const hits = await gitGrepSymbol(ref.symbol, repoRoot, includePaths, excludePaths);
    const exists = hits.length > 0;
    return {
      ref,
      exists,
      snippet: exists ? truncate(hits.slice(0, 5).join('\n'), 800) : undefined,
      note: exists ? `${hits.length} hit(s)` : 'no hits',
    };
  }
  return { ref, exists: false, note: 'unknown ref kind' };
}

async function extractFindings(
  body: string,
  verifications: VerificationResult[],
  repoRoot: string | undefined,
  includePaths: string[],
  excludePaths: string[],
): Promise<{ findings: Finding[]; toolHistory: ToolHistoryEntry[]; iterations: number }> {
  const client = new Anthropic();
  const userMessage = `## テキスト本文

${body}

## verify ステップで得られた検証結果

\`\`\`json
${JSON.stringify(verifications, null, 2)}
\`\`\`

これらは「言及された参照点が実在するか」のチェック結果です。これを起点に、必要なコードを read_file / grep_symbol で読み込み、補足観点を抽出してください。`;

  const messages: Array<{
    role: 'user' | 'assistant';
    content: string | RequestContentBlock[];
  }> = [{ role: 'user', content: userMessage }];

  let findings: Finding[] = [];
  let emitCalled = false;
  const toolHistory: ToolHistoryEntry[] = [];
  let iterations = 0;

  for (let i = 0; i < FINDINGS_MAX_ITERATIONS; i++) {
    iterations = i + 1;
    const isLast = i === FINDINGS_MAX_ITERATIONS - 1;
    const res = await client.messages.create({
      model: MODEL,
      max_tokens: MAX_TOKENS,
      system: FINDINGS_SYSTEM,
      messages,
      tools: FINDINGS_TOOLS,
      tool_choice: isLast ? { type: 'tool', name: 'emit_findings' } : { type: 'auto' },
    });

    messages.push({ role: 'assistant', content: res.content });

    const toolUses = res.content.filter(
      (b): b is Extract<typeof b, { type: 'tool_use' }> => b.type === 'tool_use',
    );
    if (toolUses.length === 0) break;

    const toolResults: RequestContentBlock[] = [];
    for (const block of toolUses) {
      const startedAt = nowMillis();
      if (block.name === 'emit_findings') {
        const inp = block.input as { findings?: Finding[] };
        findings = inp.findings ?? [];
        emitCalled = true;
        toolResults.push({
          type: 'tool_result',
          tool_use_id: block.id,
          content: 'ok',
        });
        toolHistory.push({
          tool: 'emit_findings',
          input: block.input as Record<string, unknown>,
          output: `emitted ${findings.length} findings`,
          durationMs: nowMillis() - startedAt,
        });
        break;
      }
      const result = await executeFindingsTool(
        block.name,
        block.input as Record<string, unknown>,
        repoRoot,
        includePaths,
        excludePaths,
      );
      toolResults.push({
        type: 'tool_result',
        tool_use_id: block.id,
        content: result,
      });
      toolHistory.push({
        tool: block.name,
        input: block.input as Record<string, unknown>,
        output: result,
        durationMs: nowMillis() - startedAt,
      });
    }

    if (emitCalled) break;
    messages.push({ role: 'user', content: toolResults });
  }

  return { findings, toolHistory, iterations };
}

async function executeFindingsTool(
  name: string,
  input: Record<string, unknown>,
  repoRoot: string | undefined,
  includePaths: string[],
  excludePaths: string[],
): Promise<string> {
  if (name === 'read_file') {
    const filePath = input.path as string | undefined;
    if (!filePath) return 'ERROR: missing path';
    const exists = await fileExists(filePath, repoRoot);
    if (!exists) return `ERROR: file not found: ${filePath}`;
    const lineStart = input.lineStart as number | undefined;
    const lineEnd = input.lineEnd as number | undefined;
    if (lineStart !== undefined && lineEnd !== undefined) {
      try {
        const out = await readLineRange(filePath, lineStart, lineEnd, repoRoot);
        return numberLines(out.split('\n'), lineStart);
      } catch (e) {
        return `ERROR: ${errorMessage(e)}`;
      }
    }
    try {
      const out = await execCmd('cat', [resolvePath(filePath, repoRoot)]);
      if (out.length > 6000) {
        const lines = out.split('\n').slice(0, 200);
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
  if (name === 'grep_symbol') {
    const symbol = input.symbol as string | undefined;
    if (!symbol) return 'ERROR: missing symbol';
    const maxHits = (input.maxHits as number | undefined) ?? 20;
    const hits = await gitGrepSymbol(symbol, repoRoot, includePaths, excludePaths);
    if (hits.length === 0) return `no hits for ${symbol}`;
    const head = hits.slice(0, maxHits);
    const remainder = hits.length - head.length;
    return head.join('\n') + (remainder > 0 ? `\n...(${remainder} more hits)` : '');
  }
  return `ERROR: unknown tool: ${name}`;
}

async function fileExists(p: string, repoRoot: string | undefined): Promise<boolean> {
  const fullPath = resolvePath(p, repoRoot);
  try {
    await execCmd('test', ['-e', fullPath]);
    return true;
  } catch {
    return false;
  }
}

async function readLineRange(
  p: string,
  start: number,
  end: number,
  repoRoot: string | undefined,
): Promise<string> {
  const fullPath = resolvePath(p, repoRoot);
  return execCmd('sed', ['-n', `${start},${end}p`, fullPath]);
}

async function gitGrepSymbol(
  symbol: string,
  repoRoot: string | undefined,
  includePaths: string[],
  excludePaths: string[],
): Promise<string[]> {
  const args: string[] = [];
  if (repoRoot) {
    args.push('-C', repoRoot);
  }
  args.push('grep', '-n', '-F', symbol);
  if (includePaths.length > 0 || excludePaths.length > 0) {
    args.push('--');
    for (const p of includePaths) args.push(p);
    for (const ex of excludePaths) args.push(`:!${ex}`);
  }
  try {
    const out = await execCmd('git', args);
    return out.split('\n').filter((l) => l.length > 0);
  } catch {
    return [];
  }
}

function resolvePath(p: string, repoRoot: string | undefined): string {
  if (p.startsWith('/')) return p;
  if (!repoRoot) return p;
  return repoRoot.endsWith('/') ? `${repoRoot}${p}` : `${repoRoot}/${p}`;
}

function numberLines(lines: string[], startLine: number): string {
  return lines
    .map((line, i) => `${String(startLine + i).padStart(4, ' ')}  ${line}`)
    .join('\n');
}

function truncate(s: string, max: number): string {
  return s.length > max ? s.slice(0, max) + '...(truncated)' : s;
}

function errorMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  return String(e);
}

function nowMillis(): number {
  if (typeof Date !== 'undefined' && typeof Date.now === 'function') {
    return Date.now();
  }
  return 0;
}
