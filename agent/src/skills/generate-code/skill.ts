import { generateCode, type Generated } from '../../lib/codegen.js';

interface GenerateCodeInput {
  prompt: string;
  model: string;
  apiKey: string;
}

defineSkill(async (input: GenerateCodeInput): Promise<Generated> => {
  return generateCode(input.prompt, input.model, input.apiKey);
});
