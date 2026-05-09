import { viewIssue, type ViewIssueInput } from '../../lib/viewIssue.js';

defineSkill(async (input: ViewIssueInput): Promise<string> => {
  return viewIssue(input);
});
