defineSchema({
  type: 'object',
  properties: {
    path: { type: 'string', description: 'Repo-relative file path' },
    lineStart: { type: 'integer', description: 'Starting line, 1-based (optional)' },
    lineEnd: { type: 'integer', description: 'Ending line, inclusive (optional)' },
  },
  required: ['path'],
  additionalProperties: false,
});
