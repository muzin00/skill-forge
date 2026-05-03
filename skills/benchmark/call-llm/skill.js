defineSkill(async (input) => {
  const reply = await callLlm('__BENCH_MOCK__', input);
  return { reply };
});
