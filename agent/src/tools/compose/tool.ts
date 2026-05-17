interface ComposeInput {
  value: unknown;
}

defineTool(async (input: ComposeInput): Promise<{ wrapped: unknown }> => {
  const message =
    typeof input.value === 'string' ? input.value : JSON.stringify(input.value);
  const echoed = await invokeSkill('echo', { message });
  return { wrapped: echoed };
});
