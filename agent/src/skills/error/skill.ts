interface ErrorInput {
  message?: string;
}

defineSkill(async (input: ErrorInput): Promise<never> => {
  throw new Error(input.message ?? 'intentional error from error skill');
});
