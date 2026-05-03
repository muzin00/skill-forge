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
      value: { type: 'integer' },
    },
    required: ['value'],
    additionalProperties: false,
  },
);
