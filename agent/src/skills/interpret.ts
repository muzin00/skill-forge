import { interpret, type Interpreted } from '../lib/interpret.js';

interface InterpretInput {
  prompt: string;
  model: string;
  apiKey: string;
}

export async function run(input: InterpretInput): Promise<Interpreted> {
  return interpret(input.prompt, input.model, input.apiKey);
}
