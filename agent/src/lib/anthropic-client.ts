export interface ToolDefinition {
  name: string;
  description?: string;
  input_schema: Record<string, unknown>;
}

export type ToolChoice =
  | { type: 'auto' }
  | { type: 'any' }
  | { type: 'tool'; name: string };

export interface MessagesCreateParams {
  model: string;
  max_tokens: number;
  messages: Array<{
    role: 'user' | 'assistant';
    content: string | RequestContentBlock[];
  }>;
  system?: string;
  tools?: ToolDefinition[];
  tool_choice?: ToolChoice;
}

export type ContentBlock =
  | { type: 'text'; text: string }
  | { type: 'tool_use'; id: string; name: string; input: unknown };

export type RequestContentBlock =
  | { type: 'text'; text: string }
  | { type: 'tool_use'; id: string; name: string; input: unknown }
  | { type: 'tool_result'; tool_use_id: string; content: string; is_error?: boolean };

export interface MessagesCreateResponse {
  id: string;
  type: 'message';
  role: 'assistant';
  content: ContentBlock[];
  model: string;
  stop_reason: string | null;
  stop_sequence: string | null;
  usage: { input_tokens: number; output_tokens: number };
}

export class AnthropicAPIError extends Error {
  status: number;
  body: string;

  constructor(status: number, body: string) {
    super(`HTTP ${status}: ${body}`);
    this.name = 'AnthropicAPIError';
    this.status = status;
    this.body = body;
  }
}

declare const globalThis: {
  anthropicMessages: (bodyJson: string) => string;
};

export class Anthropic {
  messages: {
    create(params: MessagesCreateParams): Promise<MessagesCreateResponse>;
  };

  constructor() {
    this.messages = {
      create: (params) => this.createMessage(params),
    };
  }

  private async createMessage(
    params: MessagesCreateParams,
  ): Promise<MessagesCreateResponse> {
    let raw: string;
    try {
      raw = globalThis.anthropicMessages(JSON.stringify(params));
    } catch (e) {
      let msg = e instanceof Error ? e.message : String(e);
      msg = msg.replace(/^(?:Error:\s*)+/, '');
      const httpMatch = /^HTTP (\d+): (.*)$/s.exec(msg);
      if (httpMatch) {
        throw new AnthropicAPIError(Number(httpMatch[1]), httpMatch[2]);
      }
      throw new AnthropicAPIError(0, msg);
    }

    return JSON.parse(raw) as MessagesCreateResponse;
  }
}
