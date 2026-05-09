interface EchoInput {
  message: string;
}

defineSkill(async (input: EchoInput): Promise<Record<string, never>> => {
  const stdout = await execCmd('echo', [input.message]);
  log(stdout.replace(/\n$/, ''));
  return {};
});
