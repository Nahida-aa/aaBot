import type { HealthResponse, ToolDef } from "./client";

export interface ChatMessage {
  role: string;
  content?: string;
  tool_calls?: Array<{
    id: string;
    type: string;
    function: { name: string; arguments: string };
  }>;
  tool_call_id?: string;
  name?: string;
}

export interface SessionSummary {
  session_id: string;
  model: string;
  provider: string;
  message_count: number;
  updated_at: string;
  created_at: string;
}

export type SseEvent =
  | { type: "RUN_STARTED"; threadId: string; runId: string }
  | { type: "RUN_FINISHED"; threadId: string; runId: string; finishReason: string }
  | { type: "RUN_ERROR"; threadId: string; runId: string; message: string }
  | { type: "TEXT_MESSAGE_START"; messageId: string }
  | { type: "TEXT_MESSAGE_CONTENT"; messageId: string; delta: string }
  | { type: "TEXT_MESSAGE_END"; messageId: string }
  | { type: "TOOL_CALL_START"; toolCallId: string; toolCallName: string }
  | { type: "TOOL_CALL_ARGS"; toolCallId: string; delta: string }
  | { type: "TOOL_CALL_END"; toolCallId: string; input: unknown; result: string };

export class AaClient {
  baseUrl: string;

  constructor(baseUrl = "http://localhost:3000") {
    this.baseUrl = baseUrl;
  }

  async health(): Promise<HealthResponse> {
    const res = await fetch(`${this.baseUrl}/health`);
    return res.json();
  }

  async listTools(): Promise<ToolDef[]> {
    const res = await fetch(`${this.baseUrl}/tools`);
    return res.json();
  }

  async listSessions(): Promise<SessionSummary[]> {
    const res = await fetch(`${this.baseUrl}/sessions`);
    if (!res.ok) return [];
    return res.json();
  }

  async *chat(
    messages: ChatMessage[],
    tools: ToolDef[],
    threadId?: string,
  ): AsyncGenerator<SseEvent> {
    const res = await fetch(`${this.baseUrl}/chat`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        thread_id: threadId ?? crypto.randomUUID(),
        run_id: crypto.randomUUID(),
        messages,
        tools,
      }),
    });

    if (!res.ok) {
      const text = await res.text();
      throw new Error(`chat failed: ${res.status} ${text}`);
    }

    const reader = res.body?.getReader();
    if (!reader) throw new Error("no response body");

    const decoder = new TextDecoder();
    let buf = "";

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buf += decoder.decode(value, { stream: true });
      const lines = buf.split("\n");
      buf = lines.pop() ?? "";

      for (const line of lines) {
        if (!line.startsWith("data: ")) continue;
        const json = line.slice(6).trim();
        if (!json || json === "[DONE]") continue;
        try {
          yield JSON.parse(json) as SseEvent;
        } catch {
          // skip malformed
        }
      }
    }
  }
}
