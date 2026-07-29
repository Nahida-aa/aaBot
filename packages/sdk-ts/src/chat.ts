import type { HealthResponse, ToolDef, SessionSummary, ChatMsg, SseEvent } from "./client";
import { createClient } from "./client/client";
import type { Client } from "./client/client";
import {
  health as genHealth,
  listSessions as genListSessions,
  listTools as genListTools,
} from "./client/sdk.gen";

export type { HealthResponse, ToolDef, SessionSummary, ChatMsg, SseEvent };

export class AaClient {
  private sdk: Client;
  readonly baseUrl: string;

  constructor(baseUrl = "http://localhost:3000") {
    this.baseUrl = baseUrl;
    this.sdk = createClient({ baseUrl, throwOnError: true });
  }

  async health(): Promise<HealthResponse> {
    const { data } = await genHealth({ client: this.sdk as any });
    return data!;
  }

  async listTools(): Promise<ToolDef[]> {
    const { data } = await genListTools({ client: this.sdk as any });
    return data!;
  }

  async listSessions(): Promise<SessionSummary[]> {
    const { data } = await genListSessions({ client: this.sdk as any });
    return data ?? [];
  }

  async deleteSession(id: string): Promise<boolean> {
    const res = await fetch(`${this.baseUrl}/sessions/${id}`, { method: "DELETE" });
    return res.ok;
  }

  async *chat(messages: ChatMsg[], tools: ToolDef[], threadId?: string): AsyncGenerator<SseEvent> {
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
