defineSchema(
  {
    type: 'object',
    properties: {
      seed: { type: 'integer' },
    },
    required: ['seed'],
    additionalProperties: false,
  },
  {
    type: 'object',
    properties: {
      result: { type: 'string' },
    },
    required: ['result'],
    additionalProperties: false,
  },
);
