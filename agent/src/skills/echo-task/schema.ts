defineSchema(
  {
    type: 'object',
    properties: {
      message: { type: 'string', description: 'Message to echo back' },
    },
    required: ['message'],
    additionalProperties: false,
  },
  {
    type: 'object',
    properties: {
      message: { type: 'string', description: 'The echoed message' },
    },
    required: ['message'],
    additionalProperties: false,
  },
);
