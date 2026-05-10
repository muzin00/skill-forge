defineSchema(
  {
    type: 'object',
    properties: {
      issueNumber: {
        type: 'string',
        description: 'GitHub Issue number or URL to evaluate',
      },
      context: {
        type: 'string',
        description:
          'Optional supplemental context. Passed verbatim to the read-context skill, which interprets a leading "@" as a file path and any other value as literal text.',
      },
    },
    required: ['issueNumber'],
    additionalProperties: false,
  },
  {
    type: 'object',
    properties: {
      evaluation: {
        type: 'string',
        description: '評価結果のテキスト',
      },
      implementable: {
        type: 'boolean',
        description: '実装可能かどうか',
      },
    },
    required: ['evaluation', 'implementable'],
    additionalProperties: false,
  },
);

defineArgs({ positional: 'issueNumber' });
