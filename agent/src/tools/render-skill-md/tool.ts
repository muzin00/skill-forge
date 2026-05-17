import {
  renderSkillFiles,
  type RenderSkillMdInput,
  type RenderSkillMdOutput,
} from '../../lib/renderSkillMd.js';

defineTool(
  async (input: RenderSkillMdInput): Promise<RenderSkillMdOutput> => {
    return renderSkillFiles(input);
  },
);
