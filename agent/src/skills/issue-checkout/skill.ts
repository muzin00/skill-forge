interface IssueCheckoutInput {
  issueNumber: string;
}

interface IssueCheckoutResult {
  branchName: string;
}

defineTask<IssueCheckoutInput, IssueCheckoutResult>({
  allowSkills: ['validate-branch-name'],
  allowCommands: [
    'gh issue',
    'git checkout',
    'git rev-parse',
    'git ls-remote',
  ],
});
