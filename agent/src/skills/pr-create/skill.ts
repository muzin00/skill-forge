interface PrCreateInput {
  base?: string;
}

interface PrCreateResult {
  prUrl: string;
}

defineTask<PrCreateInput, PrCreateResult>({
  allowSkills: [],
  allowCommands: [
    'git rev-parse',
    'git log',
    'git diff',
    'git push',
    'gh pr',
  ],
});
