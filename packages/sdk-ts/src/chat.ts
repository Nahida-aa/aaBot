import type { HealthResponse, ToolDef, SessionSummary, ChatMsg, SseEvent } from "./client";

export type { HealthResponse, ToolDef, SessionSummary, ChatMsg, SseEvent };

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

  async deleteSession(id: string): Promise<boolean> {
    const res = await fetch(`${this.baseUrl}/sessions/${id}`, { method: "DELETE" });
    return res.ok;
  }

  async *chat(
    messages: ChatMsg[],
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
