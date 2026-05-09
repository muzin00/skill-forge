defineSchema(
  {
    type: 'object',
    properties: {
      message: {
        type: 'array',
        items: { type: 'string' },
        description: 'Message tokens to echo',
      },
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

defineArgs({ positional: 'message' });
