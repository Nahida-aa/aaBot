import { type Component, createSignal, createMemo, createResource, For, createEffect, onCleanup } from "solid-js";
import { SyntaxStyle, CliRenderEvents } from "@opentui/core";
import { useRenderer } from "@opentui/solid";
import type { AaClient, ChatMessage, ToolDef } from "@aa/sdk";
import { copy } from "../../util/selection";

interface SessionProps {
  client: AaClient;
  onBack: () => void;
  toast: {
    show: (input: { message: string; variant: "info" | "success" | "warning" | "error" }) => void;
    error: (err: unknown) => void;
  };
}

interface Msg {
  id: string;
  role: "user" | "assistant" | "tool";
  content: string;
  isStreaming?: boolean;
  isTool?: boolean;
  toolName?: string;
}

const syntaxStyle = SyntaxStyle.create();

export const Session: Component<SessionProps> = (props) => {
  const renderer = useRenderer();

  // Auto-copy on selection completion
  createEffect(() => {
    const handler = (selection: any) => {
      if (selection && !selection.isDragging) {
        copy(renderer, props.toast);
      }
    };
    renderer.on(CliRenderEvents.SELECTION, handler);
    onCleanup(() => renderer.off(CliRenderEvents.SELECTION, handler));
  });

  const [messages, setMessages] = createSignal<Msg[]>([]);
  const [input, setInput] = createSignal("");
  const [streaming, setStreaming] = createSignal(false);

  const [tools] = createResource(() => props.client.listTools());

  const chatHistory = createMemo(() => {
    const msgs = messages();
    const history: ChatMessage[] = [];
    for (const m of msgs) {
      if (m.isStreaming || m.isTool) continue;
      const content = m.content || "";
      if (m.role === "assistant") {
        history.push({ role: "assistant", content });
      } else if (m.role === "user") {
        history.push({ role: "user", content });
      }
    }
    return history;
  });

  const submit = async () => {
    const text = input().trim();
    if (!text || streaming()) return;

    setMessages((prev) => [
      ...prev,
      { id: crypto.randomUUID(), role: "user", content: text },
    ]);
    setInput("");
    setStreaming(true);

    const assistantId = crypto.randomUUID();
    setMessages((prev) => [
      ...prev,
      { id: assistantId, role: "assistant", content: "", isStreaming: true },
    ]);

    try {
      const toolDefs = tools() ?? [];
      const history = chatHistory();
      history.push({ role: "user", content: text });

      let fullContent = "";

      for await (const event of props.client.chat(history, toolDefs)) {
        switch (event.type) {
          case "TEXT_MESSAGE_CONTENT":
            fullContent += event.delta;
            updateMsg(assistantId, { content: fullContent });
            break;
          case "TOOL_CALL_START":
            setMessages((prev) => [
              ...prev,
              {
                id: event.toolCallId,
                role: "tool",
                content: "",
                isTool: true,
                toolName: event.toolCallName,
                isStreaming: true,
              },
            ]);
            break;
          case "TOOL_CALL_ARGS":
            setMessages((prev) =>
              prev.map((m) =>
                m.id === event.toolCallId
                  ? { ...m, content: m.content + event.delta }
                  : m,
              ),
            );
            break;
          case "TOOL_CALL_END":
            updateMsg(event.toolCallId, {
              content: JSON.stringify(event.result).slice(0, 500),
              isStreaming: false,
            });
            break;
          case "RUN_ERROR":
            updateMsg(assistantId, {
              content: `Error: ${event.message}`,
              isStreaming: false,
            });
            setStreaming(false);
            return;
          case "RUN_FINISHED":
            updateMsg(assistantId, { isStreaming: false });
            setStreaming(false);
            break;
        }
      }
    } catch (err) {
      updateMsg(assistantId, {
        content: `Error: ${err}`,
        isStreaming: false,
      });
      setStreaming(false);
    }
  };

  function updateMsg(id: string, patch: Partial<Msg>) {
    setMessages((prev) =>
      prev.map((m) => (m.id === id ? { ...m, ...patch } : m)),
    );
  }

  return (
    <box
      width="100%"
      height="100%"
      flexDirection="column"
      on:keypress={(e) => {
        if (e.name === "escape") props.onBack();
      }}
    >
      {/* Header */}
      <box height={1} flexDirection="row">
        <text fg="cyan">aaBot</text>
        <text> — AI Assistant</text>
        <box flexGrow={1} />
        <text fg="#666">Esc: back</text>
      </box>

      {/* Messages */}
      <scrollbox flexGrow={1} stickyScroll stickyStart="bottom">
        <For each={messages()}>
          {(msg) => (
            <box flexDirection="column">
              {msg.isTool ? (
                <text fg="yellow">
                  ↻ {msg.toolName}: {msg.content || "running..."}
                </text>
              ) : (
                <>
                  <text fg={msg.role === "user" ? "green" : "cyan"}>
                    {msg.role === "user" ? "You" : "AI"}
                  </text>
                  {msg.content ? (
                    <markdown content={msg.content} syntaxStyle={syntaxStyle} />
                  ) : null}
                  {msg.isStreaming && <text fg="#666">▊</text>}
                </>
              )}
              <box height={1} />
            </box>
          )}
        </For>
      </scrollbox>

      {/* Input */}
      <box height={3} flexDirection="row">
        <textarea
          initialValue={input()}
          placeholder="Type a message..."
          placeholderColor="#666"
          minHeight={1}
          maxHeight={3}
          flexGrow={1}
          onContentChange={(v) => { if (typeof v === "string") setInput(v); }}
          onSubmit={submit}
        />
      </box>
    </box>
  );
};
