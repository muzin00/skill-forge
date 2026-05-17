const PROMPT = `# Echo task

The user message is a JSON object with a "message" field which is an array of strings.

Call the "output" tool exactly once with the array elements joined by a single space as the "message" field.
Do not modify, summarize, or rephrase any token — preserve every token verbatim and join them with one ASCII space.`;

interface EchoTaskInput {
  message: string[];
}

interface EchoTaskOutput {
  message: string;
}

defineSkill<EchoTaskInput, EchoTaskOutput>({
  prompt: PROMPT,
  allowTools: ['echo'],
  maxIterations: 3,
});
