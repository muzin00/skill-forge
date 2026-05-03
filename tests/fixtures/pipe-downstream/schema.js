defineSchema(
  {
    type: 'object',
    properties: {
      value: { type: 'integer' },
    },
    required: ['value'],
    additionalProperties: false,
  },
  {
    type: 'object',
    properties: {
      doubled: { type: 'integer' },
    },
    required: ['doubled'],
    additionalProperties: false,
  },
);
