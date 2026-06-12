import { createSignal, createEffect, onMount, For } from "solid-js";
import { healthCheck, listTools, callTool, type ToolDef } from "./api";
import styles from "./App.module.css";

interface ChatMessage {
  role: "user" | "assistant" | "tool-call" | "tool-result";
  content: string;
  name?: string;
}

export default function App() {
  const [connected, setConnected] = createSignal(false);
  const [tools, setTools] = createSignal<ToolDef[]>([]);
  const [messages, setMessages] = createSignal<ChatMessage[]>([]);
  const [input, setInput] = createSignal("");

  onMount(async () => {
    try {
      const h = await healthCheck();
      setConnected(true);
      const t = await listTools();
      setTools(t);
      setMessages([
        { role: "assistant", content: `Connected. ${h.tool_count} tools available.` },
      ]);
    } catch {
      setConnected(false);
      setMessages([
        { role: "assistant", content: "Could not connect to aa server. Start it with `cargo run -p aa-server` on port 3000." },
      ]);
    }
  });

  async function sendMessage() {
    const text = input().trim();
    if (!text) return;
    setInput("");
    const userMsg: ChatMessage = { role: "user", content: text };
    setMessages((prev) => [...prev, userMsg]);

    const currentTools = tools();
    const toolMap = new Map(currentTools.map((t) => [t.name, t]));
    const prevMsgs = [...messages(), userMsg].filter((m) => m.role === "user" || m.role === "assistant");

    let attempt = 0;
    async function loop(history: { role: string; content: string }[]): Promise<void> {
      if (attempt > 5) {
        setMessages((prev) => [
          ...prev,
          { role: "assistant", content: "Too many tool calls, stopping." },
        ]);
        return;
      }
      attempt++;
      setMessages((prev) => [
        ...prev,
        { role: "tool-call", content: `Sending to LLM with ${history.length} messages...` },
      ]);

      try {
        const res = await fetch("/api/chat", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            messages: history,
            tools: currentTools,
          }),
        });
        const data = await res.json();
        if (data.error) {
          const m: ChatMessage = { role: "assistant", content: `Error: ${data.error}` };
          setMessages((prev) => [...prev, m]);
          return;
        }

        if (data.content) {
          const m: ChatMessage = { role: "assistant", content: data.content };
          setMessages((prev) => [...prev, m]);
        }

        if (data.tool_calls && data.tool_calls.length > 0) {
          for (const tc of data.tool_calls) {
            const tcName = tc.function?.name || tc.name;
            const tcArgsStr = tc.function?.arguments || tc.arguments || "{}";
            const m: ChatMessage = { role: "tool-call", content: `🔧 ${tcName}(${tcArgsStr})`, name: tcName };
            setMessages((prev) => [...prev, m]);

            let tcArgs: unknown;
            try {
              tcArgs = JSON.parse(typeof tcArgsStr === "string" ? tcArgsStr : JSON.stringify(tcArgsStr));
            } catch {
              tcArgs = {};
            }

            const result = await callTool(tcName, tcArgs);
            const resultContent = result.is_error
              ? `⚠️ ${result.content}`
              : result.content.slice(0, 500);
            const rm: ChatMessage = { role: "tool-result", content: resultContent, name: tcName };
            setMessages((prev) => [...prev, rm]);

            history.push({
              role: "assistant",
              content: "",
              tool_calls: [{ id: tc.id || "call_1", type: "function", function: { name: tcName, arguments: tcArgsStr } }],
            } as any);
            history.push({
              role: "tool",
              tool_call_id: tc.id || "call_1",
              content: result.content,
            } as any);
          }

          await loop(history);
        }
      } catch (e: unknown) {
        const errMsg = e instanceof Error ? e.message : String(e);
        const m: ChatMessage = { role: "assistant", content: `Error: ${errMsg}` };
        setMessages((prev) => [...prev, m]);
      }
    }

    await loop(
      prevMsgs.map((m) => ({ role: m.role, content: m.content }))
    );
  }

  return (
    <div class={styles.container}>
      <div class={styles.header}>
        <span class={styles.title}>aaBot</span>
        <span classList={{ [styles.statusOk]: connected(), [styles.statusErr]: !connected() }}>
          {connected() ? `${tools().length} tools` : "disconnected"}
        </span>
      </div>

      <div class={styles.chat}>
        <For each={messages()}>
          {(msg) => (
            msg.role === "user" ? (
              <div class={styles.user}>{msg.content}</div>
            ) : msg.role === "assistant" ? (
              <div class={styles.assistant}>{msg.content}</div>
            ) : msg.role === "tool-call" ? (
              <div class={styles.toolCall}>{msg.content}</div>
            ) : (
              <div class={styles.toolResult}>{msg.content}</div>
            )
          )}
        </For>
      </div>

      <div class={styles.inputRow}>
        <input
          class={styles.input}
          placeholder="Type a message..."
          value={input()}
          onInput={(e) => setInput(e.currentTarget.value)}
          onKeyDown={(e) => e.key === "Enter" && sendMessage()}
        />
        <button class={styles.sendBtn} onClick={sendMessage} disabled={!connected()}>
          Send
        </button>
      </div>
    </div>
  );
}
