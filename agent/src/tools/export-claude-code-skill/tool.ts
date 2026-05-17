interface ExportClaudeCodeSkillInput {
  skillName: string;
  homeDir: string;
}

interface ExportClaudeCodeSkillOutput {
  destPath: string;
}

defineTool(
  async (
    input: ExportClaudeCodeSkillInput,
  ): Promise<ExportClaudeCodeSkillOutput> => {
    const home = input.homeDir.replace(/\/+$/, '');
    return { destPath: `${home}/.claude/skills/${input.skillName}` };
  },
);
