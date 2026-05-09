interface ValidateBranchNameInput {
  branchName: string;
}

interface ValidateBranchNameOutput {
  valid: boolean;
  errors: string[];
}

const ALLOWED_PREFIXES = [
  'feature',
  'fix',
  'chore',
  'docs',
  'refactor',
  'test',
  'poc',
];

const ISSUE_NUMBER_PATTERN = /^\d+$/;
const SUMMARY_PATTERN = /^[a-z0-9]+(-[a-z0-9]+)*$/;

defineSkill(
  async (input: ValidateBranchNameInput): Promise<ValidateBranchNameOutput> => {
    const errors: string[] = [];
    const segments = input.branchName.split('/');

    if (segments.length !== 3) {
      errors.push(
        `branch name must have exactly 3 slash-separated segments {prefix}/{issue-number}/{summary} (got ${segments.length})`,
      );
      return { valid: false, errors };
    }

    const [prefix, issueNumber, summary] = segments as [string, string, string];

    if (!ALLOWED_PREFIXES.includes(prefix)) {
      errors.push(
        `prefix "${prefix}" is not in allowed set: ${ALLOWED_PREFIXES.join(', ')}`,
      );
    }

    if (!ISSUE_NUMBER_PATTERN.test(issueNumber)) {
      errors.push(
        `issue-number "${issueNumber}" must consist of one or more digits`,
      );
    }

    if (!SUMMARY_PATTERN.test(summary)) {
      errors.push(
        `summary "${summary}" must be lowercase kebab-case (lowercase letters, digits, hyphens; no leading/trailing/consecutive hyphens)`,
      );
    }

    return { valid: errors.length === 0, errors };
  },
);
