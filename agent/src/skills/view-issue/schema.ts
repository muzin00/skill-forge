defineSchema({
  type: 'object',
  properties: {
    issue: { type: 'string', description: 'Issue number or URL' },
  },
  required: ['issue'],
  additionalProperties: false,
});
