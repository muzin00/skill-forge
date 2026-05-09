defineSchema({
  type: 'object',
  properties: {
    prompt: {
      type: 'string',
      description: 'System message: instructions for the LLM',
    },
    context: {
      type: 'string',
      description: 'User message: the material to work with',
    },
    allowSkills: {
      type: 'array',
      items: { type: 'string' },
      description: 'Names of skills the LLM can call as tools',
    },
    allowCommands: {
      type: 'array',
      items: { type: 'string' },
      description:
        'Command prefixes the LLM can call as tools. Each entry (e.g. "gh issue") becomes a tool named by joining with "-" (e.g. "gh-issue") whose input is { args: string[] } and which runs `<entry> <args...>` via execCmd. Args are passed as argv (no shell evaluation).',
    },
    outputSchema: {
      type: 'object',
      description:
        'JSON Schema for the structured output (used as input_schema of the output tool)',
      additionalProperties: true,
    },
    model: { type: 'string', description: 'LLM model name (optional)' },
    maxTokens: {
      type: 'integer',
      description: 'Max tokens per LLM call (optional, default 4096)',
    },
    maxIterations: {
      type: 'integer',
      description: 'Max loop iterations (optional, default 15)',
    },
  },
  required: ['prompt', 'context', 'allowSkills', 'outputSchema'],
  additionalProperties: false,
});
