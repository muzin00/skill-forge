defineSchema(
  {
    type: 'object',
    properties: {
      name: { type: 'string', description: 'Skill name (directory name)' },
      description: {
        type: 'string',
        description: 'DESCRIPTION.md content for the skill',
      },
      inputSchema: {
        type: 'object',
        description: 'JSON Schema describing the skill input',
        additionalProperties: true,
      },
      outputSchema: {
        type: 'object',
        description: 'JSON Schema describing the skill output (optional)',
        additionalProperties: true,
      },
      positionalProp: {
        type: 'string',
        description: 'Name of the property used as the CLI positional argument, if any',
      },
    },
    required: ['name', 'description', 'inputSchema'],
    additionalProperties: false,
  },
  {
    type: 'object',
    properties: {
      skillMd: {
        type: 'string',
        description: 'Full SKILL.md content (frontmatter + body)',
      },
      descriptionMd: {
        type: 'string',
        description: 'DESCRIPTION.md content (normalized with trailing newline)',
      },
    },
    required: ['skillMd', 'descriptionMd'],
    additionalProperties: false,
  },
);
