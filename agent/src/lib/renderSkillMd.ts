/**
 * Pure templating for `forge export`. Given a skill's metadata + JSON Schemas,
 * produce the SKILL.md and DESCRIPTION.md file contents. No I/O, no LLM.
 *
 * Imported by the `render-skill-md` builtin tool; could later be reused by
 * target-specific tools if a per-agent variant of SKILL.md is ever needed.
 */

export const DESCRIPTION_MAX_LEN = 1024;

export interface JsonSchema {
  type?: string;
  properties?: Record<string, JsonSchema>;
  required?: string[];
  description?: string;
  [key: string]: unknown;
}

export interface RenderSkillMdInput {
  name: string;
  description: string;
  inputSchema: JsonSchema;
  outputSchema?: JsonSchema | null;
  positionalProp?: string | null;
}

export interface RenderSkillMdOutput {
  skillMd: string;
  descriptionMd: string;
}

export function renderSkillFiles(
  input: RenderSkillMdInput,
): RenderSkillMdOutput {
  const desc = input.description.replace(/^\s+|\s+$/g, '');
  const codepointLen = [...desc].length;
  if (codepointLen > DESCRIPTION_MAX_LEN) {
    throw new Error(
      `skill '${input.name}': DESCRIPTION.md is ${codepointLen} chars but the SKILL.md ` +
        `frontmatter \`description\` field is capped at ${DESCRIPTION_MAX_LEN} chars ` +
        `(Anthropic Agent Skills spec). Shorten the DESCRIPTION.md.`,
    );
  }
  const skillMd = renderSkillMd(
    input.name,
    desc,
    input.inputSchema,
    input.outputSchema ?? null,
    input.positionalProp ?? null,
  );
  const descriptionMd = renderDescriptionMd(input.description);
  return { skillMd, descriptionMd };
}

function renderDescriptionMd(description: string): string {
  return description.replace(/\s+$/, '') + '\n';
}

function renderSkillMd(
  name: string,
  description: string,
  inputSchema: JsonSchema,
  outputSchema: JsonSchema | null,
  positionalProp: string | null,
): string {
  let out = '';

  out += '---\n';
  out += `name: ${name}\n`;
  out += 'description: |\n';
  for (const line of description.split('\n')) {
    out += `  ${line}\n`;
  }
  out += 'allowed-tools: ["Bash"]\n';
  out += '---\n\n';

  out += `# ${name}\n\n`;
  out +=
    'forge skill のラッパー。実装の詳細は [DESCRIPTION.md](./DESCRIPTION.md) を参照。\n\n';

  const quoting = invocationQuoting(inputSchema, positionalProp);
  out += '## 呼び出し方\n\n';
  out += '```bash\n';
  out +=
    quoting === 'single-positional-quoted'
      ? `forge run ${name} "$ARGUMENTS"\n`
      : `forge run ${name} $ARGUMENTS\n`;
  out += '```\n\n';
  out +=
    '`$ARGUMENTS` には `/<command>` 以降のユーザー入力がそのまま入る。' +
    '自然文経由で自動呼び出しされた場合は、ユーザー入力から該当する値を抽出して ' +
    '同じ Bash コマンドに渡すこと。\n\n';

  out += '### stdin から追加 context を注入する\n\n';
  out += '```bash\n';
  out += `echo "補足情報..." | forge run ${name} "$ARGUMENTS"\n`;
  out += '```\n\n';
  out +=
    'stdin の内容は LLM への user input（CLI 引数と併せて単一 user message 内の ' +
    '追加 text block）として供給される。\n\n';

  out += '### 他の skill と連結する\n\n';
  out += '```bash\n';
  out += `forge run other-skill | forge run ${name} "$ARGUMENTS"\n`;
  out += '```\n\n';
  out += '上流 skill の JSON 出力が下流の stdin として届く。\n\n';

  out += '## 入力\n\n';
  out += renderInputProperties(inputSchema, positionalProp);
  out += '- stdin (optional): 追加コンテキスト（pipe 経由）\n\n';

  out += '## 出力 (JSON)\n\n';
  out += outputSchema
    ? renderOutputProperties(outputSchema)
    : '出力スキーマは定義されていない（任意の JSON が返る）。\n';

  return out;
}

function invocationQuoting(
  inputSchema: JsonSchema,
  positionalProp: string | null,
): 'single-positional-quoted' | 'shell-tokenized' {
  const props = inputSchema.properties;
  const propertyCount =
    props && typeof props === 'object' ? Object.keys(props).length : 0;
  if (propertyCount === 1 && positionalProp) {
    return 'single-positional-quoted';
  }
  return 'shell-tokenized';
}

function renderInputProperties(
  schema: JsonSchema,
  positionalProp: string | null,
): string {
  const props = schema.properties;
  if (!props || Object.keys(props).length === 0) {
    return '(入力プロパティなし)\n\n';
  }
  const required = Array.isArray(schema.required) ? schema.required : [];
  let lines = '';
  for (const [name, prop] of Object.entries(props)) {
    const ty = typeof prop?.type === 'string' ? prop.type : 'any';
    const markers: string[] = [];
    markers.push(required.includes(name) ? 'required' : 'optional');
    if (positionalProp === name) markers.push('positional');
    const desc = typeof prop?.description === 'string' ? prop.description : '';
    lines += desc
      ? `- \`${name}\` (${ty}, ${markers.join(', ')}): ${desc}\n`
      : `- \`${name}\` (${ty}, ${markers.join(', ')})\n`;
  }
  return lines;
}

function renderOutputProperties(schema: JsonSchema): string {
  const props = schema.properties;
  if (!props || Object.keys(props).length === 0) {
    return '(出力プロパティなし)\n';
  }
  let lines = '';
  for (const [name, prop] of Object.entries(props)) {
    const ty = typeof prop?.type === 'string' ? prop.type : 'any';
    const desc = typeof prop?.description === 'string' ? prop.description : '';
    lines += desc
      ? `- \`${name}\` (${ty}): ${desc}\n`
      : `- \`${name}\` (${ty})\n`;
  }
  return lines;
}
