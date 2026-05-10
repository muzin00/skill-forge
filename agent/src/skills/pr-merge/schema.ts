defineSchema(
  {
    type: 'object',
    properties: {},
    additionalProperties: false,
  },
  {
    type: 'object',
    properties: {
      merged: {
        type: 'boolean',
        description: 'true if the PR was merged in this invocation',
      },
      prUrl: {
        type: 'string',
        description: 'URL of the PR (omitted only if no PR was found)',
      },
    },
    required: ['merged'],
    additionalProperties: false,
  },
);
