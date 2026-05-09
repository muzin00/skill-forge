defineSchema(
  {
    type: 'object',
    properties: {
      message: { type: 'string', description: 'Message to echo' },
    },
    required: ['message'],
    additionalProperties: false,
  },
  {
    type: 'object',
    properties: {},
    additionalProperties: false,
  },
);
