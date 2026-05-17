interface PrCreateInput {
  base?: string;
}

interface PrCreateResult {
  prUrl: string;
}

defineSkill<PrCreateInput, PrCreateResult>({
  allowTools: [],
  allowCommands: [
    'git rev-parse',
    'git log',
    'git diff',
    'git push',
    'gh pr',
  ],
});
