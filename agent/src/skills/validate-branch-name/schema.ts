defineSchema(
  {
    type: 'object',
    properties: {
      branchName: { type: 'string', description: 'Branch name to validate' },
    },
    required: ['branchName'],
    additionalProperties: false,
  },
  {
    type: 'object',
    properties: {
      valid: {
        type: 'boolean',
        description: 'true if branch name satisfies all rules',
      },
      errors: {
        type: 'array',
        items: { type: 'string' },
        description: 'Reasons why the branch name is invalid (empty when valid)',
      },
    },
    required: ['valid', 'errors'],
    additionalProperties: false,
  },
);

defineArgs({ positional: 'branchName' });
