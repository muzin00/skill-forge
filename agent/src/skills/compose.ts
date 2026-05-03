interface ComposeInput {
  value: unknown;
}

defineSkill(async (input: ComposeInput): Promise<{ wrapped: unknown }> => {
  const echoed = await invokeSkill('echo', input.value);
  return { wrapped: echoed };
});
