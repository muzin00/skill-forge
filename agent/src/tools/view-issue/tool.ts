import { viewIssue, type ViewIssueInput } from '../../lib/viewIssue.js';

defineTool(async (input: ViewIssueInput): Promise<string> => {
  return viewIssue(input);
});
