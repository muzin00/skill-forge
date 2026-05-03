defineSchema({
  type: 'object',
  properties: {
    userName: { type: 'string' },
    count: { type: 'integer' },
    ratio: { type: 'number' },
    enabled: { type: 'boolean' },
    tags: { type: 'array', items: { type: 'string' } },
    color: { type: 'string', enum: ['red', 'green', 'blue'] },
    optionalNote: { type: 'string', default: 'unused' },
  },
  required: ['userName', 'count', 'ratio'],
  additionalProperties: false,
});
