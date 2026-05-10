interface ImplementationCheckInput {
  issueNumber: string;
  context?: string;
}

interface ImplementationCheckResult {
  evaluation: string;
  implementable: boolean;
}

defineTask<ImplementationCheckInput, ImplementationCheckResult>({
  allowSkills: ['read-context'],
  allowCommands: [
    'cat',
    'head',
    'tail',
    'git grep',
    'git log',
    'git diff',
    'git show',
    'gh issue',
    'find',
    'ls',
  ],
});
