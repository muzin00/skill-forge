declare module 'skill-forge:runtime/skill-loader-host' {
  export function getSource(): string;
  export function getDescription(skillName: string): string;
}

declare module 'skill-forge:runtime/schema-loader-host' {
  export function getSchemaSource(): string;
  export function getInputSchemaJson(skillName: string): string;
}

declare module 'skill-forge:runtime/llm-host' {
  export function callLlm(prompt: string, inputJson: string): string;
}

declare module 'skill-forge:runtime/exec-host' {
  export function execCmd(cmd: string, args: string[]): string;
}

declare module 'skill-forge:runtime/anthropic-host' {
  export function messages(bodyJson: string): string;
}

declare module 'skill-forge:runtime/invoke-host' {
  export function invoke(skillName: string, argsJson: string): string;
}

declare module 'skill-forge:runtime/log-host' {
  export function log(message: string): void;
}

declare module 'skill-forge:runtime/instruction-loader-host' {
  export function getInstruction(): string;
}
