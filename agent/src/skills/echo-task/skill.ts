import { defineTask } from '../../lib/defineTask.js';

const PROMPT = `# Echo task

The user message is a JSON object with a "message" field.

Call the "output" tool exactly once with the same "message" field, returning the input verbatim.
Do not modify, summarize, or rephrase the message — pass it through as-is.`;

interface EchoTaskInput {
  message: string;
}

interface EchoTaskOutput {
  message: string;
}

defineTask<EchoTaskInput, EchoTaskOutput>({
  prompt: PROMPT,
  allowSkills: ['echo'],
  maxIterations: 3,
});
