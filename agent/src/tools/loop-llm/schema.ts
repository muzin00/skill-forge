defineSchema({
  type: 'object',
  properties: {
    prompt: {
      type: 'string',
      description: 'System message: instructions for the LLM',
    },
    context: {
      type: 'array',
      items: { type: 'string' },
      minItems: 1,
      description:
        'User message: array of text blocks, each becomes a separate text content block in a single user message',
    },
    allowTools: {
      type: 'array',
      items: { type: 'string' },
      description: 'Names of registered tools / skills the LLM can call as tools',
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
  required: ['prompt', 'context', 'allowTools', 'outputSchema'],
  additionalProperties: false,
});
