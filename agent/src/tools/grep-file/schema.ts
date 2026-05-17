defineSchema({
  type: 'object',
  properties: {
    query: { type: 'string', description: 'Substring to search for (literal, not regex)' },
    maxHits: { type: 'integer', description: 'Max hits to return (default 20)' },
  },
  required: ['query'],
  additionalProperties: false,
});
