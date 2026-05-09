defineSchema(
  {
    type: 'object',
    properties: {
      base: {
        type: 'string',
        description:
          'Base branch the PR targets (local branch or remote-tracking ref like origin/main). Defaults to "main" when omitted.',
      },
    },
    additionalProperties: false,
  },
  {
    type: 'object',
    properties: {
      prUrl: {
        type: 'string',
        description: 'URL of the created pull request',
      },
    },
    required: ['prUrl'],
    additionalProperties: false,
  },
);
