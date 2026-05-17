interface EchoInput {
  message: string;
}

defineTool(async (input: EchoInput): Promise<Record<string, never>> => {
  const stdout = await execCmd('echo', [input.message]);
  log(stdout.replace(/\n$/, ''));
  return {};
});
