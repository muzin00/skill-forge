defineSchema(
  {
    type: 'object',
    properties: {
      value: {
        type: 'string',
        description:
          'Raw context value. A value starting with "@" is treated as a file path (the rest after "@" is read with `cat`); any other value is treated as a literal string.',
      },
    },
    required: ['value'],
    additionalProperties: false,
  },
  {
    type: 'object',
    properties: {
      content: {
        type: 'string',
        description: 'Resolved context content',
      },
      source: {
        type: 'string',
        enum: ['literal', 'file'],
        description: 'Whether content came from the literal value or a file',
      },
      sourcePath: {
        type: 'string',
        description: 'File path when source is "file" (omitted otherwise)',
      },
    },
    required: ['content', 'source'],
    additionalProperties: false,
  },
);

defineArgs({ positional: 'value' });
