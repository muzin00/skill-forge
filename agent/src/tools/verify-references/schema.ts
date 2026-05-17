defineSchema(
  {
    type: 'object',
    properties: {
      text: { type: 'string' },
      repoRoot: { type: 'string' },
      scope: {
        type: 'object',
        properties: {
          paths: { type: 'array', items: { type: 'string' } },
          excludePaths: { type: 'array', items: { type: 'string' } },
        },
        additionalProperties: false,
      },
      kinds: {
        type: 'array',
        items: { type: 'string', enum: ['file', 'symbol', 'line-range'] },
      },
    },
    required: ['text'],
    additionalProperties: false,
  },
  {
    type: 'object',
    properties: {},
    additionalProperties: true,
  },
);
