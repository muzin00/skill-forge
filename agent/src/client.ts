export interface MessagesCreateParams {
  model: string;
  max_tokens: number;
  messages: Array<{ role: 'user' | 'assistant'; content: string }>;
  system?: string;
}

export interface MessagesCreateResponse {
  id: string;
  type: 'message';
  role: 'assistant';
  content: Array<{ type: 'text'; text: string }>;
  model: string;
  stop_reason: string | null;
  stop_sequence: string | null;
  usage: { input_tokens: number; output_tokens: number };
}

export interface AnthropicOptions {
  apiKey: string;
  baseUrl?: string;
  version?: string;
}

export class AnthropicAPIError extends Error {
  status: number;
  body: string;
  requestId?: string;

  constructor(status: number, body: string, requestId?: string) {
    super(`HTTP ${status}: ${body}`);
    this.name = 'AnthropicAPIError';
    this.status = status;
    this.body = body;
    this.requestId = requestId;
  }
}

export class Anthropic {
  private apiKey: string;
  private baseUrl: string;
  private version: string;

  messages: {
    create(params: MessagesCreateParams): Promise<MessagesCreateResponse>;
  };

  constructor(opts: AnthropicOptions) {
    this.apiKey = opts.apiKey;
    this.baseUrl = opts.baseUrl ?? 'https://api.anthropic.com';
    this.version = opts.version ?? '2023-06-01';

    this.messages = {
      create: (params) => this.createMessage(params),
    };
  }

  private async createMessage(
    params: MessagesCreateParams,
  ): Promise<MessagesCreateResponse> {
    const url = `${this.baseUrl}/v1/messages`;
    const res = await fetch(url, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        'x-api-key': this.apiKey,
        'anthropic-version': this.version,
      },
      body: JSON.stringify(params),
    });

    if (!res.ok) {
      const body = await res.text();
      const requestId = res.headers.get('request-id') ?? undefined;
      throw new AnthropicAPIError(res.status, body, requestId);
    }

    return (await res.json()) as MessagesCreateResponse;
  }
}
