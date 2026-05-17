defineSchema(
  {
    type: 'object',
    properties: {
      skillName: {
        type: 'string',
        description: 'Skill name (canonical dir name under ~/.skill-forge/exports/)',
      },
      homeDir: {
        type: 'string',
        description: "User's HOME directory as an absolute path",
      },
    },
    required: ['skillName', 'homeDir'],
    additionalProperties: false,
  },
  {
    type: 'object',
    properties: {
      destPath: {
        type: 'string',
        description: 'Absolute path where forge should place the symlink for Claude Code to discover this skill',
      },
    },
    required: ['destPath'],
    additionalProperties: false,
  },
);
