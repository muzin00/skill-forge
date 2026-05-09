defineSchema(
  {
    type: 'object',
    properties: {
      issueNumber: {
        type: 'string',
        description: 'GitHub Issue number or URL',
      },
      base: {
        type: 'string',
        description:
          'Base ref the new branch is created from (local branch or remote-tracking ref like origin/main). Defaults to "main" when omitted.',
      },
    },
    required: ['issueNumber'],
    additionalProperties: false,
  },
  {
    type: 'object',
    properties: {
      branchName: {
        type: 'string',
        description: 'Branch name that was created and checked out',
      },
    },
    required: ['branchName'],
    additionalProperties: false,
  },
);

defineArgs({ positional: 'issueNumber' });
