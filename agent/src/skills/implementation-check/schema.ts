defineSchema({
  type: 'object',
  properties: {
    issueNumber: {
      type: 'string',
      description: 'GitHub Issue number or URL to evaluate',
    },
  },
  required: ['issueNumber'],
  additionalProperties: false,
});
