import {
  generateSkillCode,
  type Generated,
  type SignatureEntry,
} from '../../lib/codegen.js';

interface GenerateSkillCodeInput {
  prompt: string;
  signature?: SignatureEntry[];
  model: string;
  apiKey: string;
}

defineSkill(async (input: GenerateSkillCodeInput): Promise<Generated> => {
  return generateSkillCode(input.prompt, input.model, input.apiKey, input.signature);
});
