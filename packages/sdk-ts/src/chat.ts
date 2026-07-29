import type { ChatMsg, SseEvent, ToolDef } from "./client";
import { client as defaultClient } from "./client/client.gen";
import type { Client } from "./client/client";

export type { ChatMsg, SseEvent, ToolDef };

export async function* chat(
  messages: ChatMsg[],
  tools: ToolDef[],
  threadId?: string,
  options?: { client?: Client },
): AsyncGenerator<SseEvent> {
  const c = options?.client ?? defaultClient;
  const cfg = c.getConfig();
  const baseUrl = cfg.baseUrl ?? "http://localhost:3000";
  const url = `${baseUrl}/chat`;

  const res = await fetch(url, {
    method: "POST",
    headers: {
      "content-type": "application/json",
    },
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
