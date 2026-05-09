declare const execCmd: (cmd: string, args: string[]) => Promise<string>;

export interface ViewIssueInput {
  issue: string;
}

export async function viewIssue(input: ViewIssueInput): Promise<string> {
  return execCmd('gh', ['issue', 'view', input.issue]);
}
