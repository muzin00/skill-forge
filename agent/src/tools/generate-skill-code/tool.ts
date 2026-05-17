import { generateSkillCode, type Generated } from '../../lib/codegen.js';

interface GenerateSkillCodeInput {
  prompt: string;
  model: string;
}

defineTool(async (input: GenerateSkillCodeInput): Promise<Generated> => {
  return generateSkillCode(input.prompt, input.model);
});
