defineSkill(async () => {
  let caught = null;
  try {
    await invokeSkill('error', { message: 'boom' });
  } catch (e) {
    caught = { code: e.code, message: e.message };
  }

  const composed = await invokeSkill('compose', { value: { hello: 'nested' } });

  let denied = null;
  try {
    await invokeSkill('does-not-exist', {});
  } catch (e) {
    denied = { code: e.code, message: e.message };
  }

  return { caught, composed, denied };
});
