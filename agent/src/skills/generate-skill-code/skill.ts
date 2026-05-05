import { generateSkillCode, type Generated } from '../../lib/codegen.js';

interface GenerateSkillCodeInput {
  prompt: string;
  model: string;
  apiKey: string;
}

defineSkill(async (input: GenerateSkillCodeInput): Promise<Generated> => {
  return generateSkillCode(input.prompt, input.model, input.apiKey);
});
