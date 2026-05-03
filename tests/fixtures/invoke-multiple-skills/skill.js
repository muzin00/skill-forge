defineSkill(async (input) => {
  const echoed = await invokeSkill('echo', { hello: 'world', n: input.n ?? 42 });

  let caught = null;
  try {
    await invokeSkill('error', { message: 'boom' });
  } catch (e) {
    caught = { code: e.code, message: e.message };
  }

  const a = await invokeSkill('echo', { tag: 'A' });
  const b = await invokeSkill('echo', { tag: 'B' });

  const composed = await invokeSkill('compose', { value: { hello: 'nested' } });

  let denied = null;
  try {
    await invokeSkill('does-not-exist', {});
  } catch (e) {
    denied = { code: e.code, message: e.message };
  }

  return { echoed, caught, sequential: { a, b }, composed, denied };
});
