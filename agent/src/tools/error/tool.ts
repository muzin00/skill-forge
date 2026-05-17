interface ErrorInput {
  message?: string;
}

defineTool(async (input: ErrorInput): Promise<never> => {
  throw new Error(input.message ?? 'intentional error from error skill');
});
