import {
  generateCodeFromSignature,
  type Generated,
  type SignatureEntry,
} from '../lib/codegen.js';

interface GenerateCodeFromSignatureInput {
  prompt: string;
  signature: SignatureEntry[];
  model: string;
  apiKey: string;
}

export async function run(
  input: GenerateCodeFromSignatureInput,
): Promise<Generated> {
  return generateCodeFromSignature(
    input.prompt,
    input.signature,
    input.model,
    input.apiKey,
  );
}
