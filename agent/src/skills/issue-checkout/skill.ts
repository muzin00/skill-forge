interface IssueCheckoutInput {
  issueNumber: string;
}

interface IssueCheckoutResult {
  branchName: string;
}

defineSkill<IssueCheckoutInput, IssueCheckoutResult>({
  allowTools: ['validate-branch-name'],
  allowCommands: [
    'gh issue',
    'git checkout',
    'git rev-parse',
    'git ls-remote',
  ],
});
