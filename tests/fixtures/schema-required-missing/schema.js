defineSchema({
  type: 'object',
  properties: {
    userName: { type: 'string' },
  },
  required: ['userName'],
  additionalProperties: false,
});
