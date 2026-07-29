import {
  type Component,
  createSignal,
  createMemo,
  createResource,
  For,
  createEffect,
  onCleanup,
  onMount,
} from "solid-js";
import { SyntaxStyle, CliRenderEvents } from "@opentui/core";
import { useRenderer } from "@opentui/solid";
import type { ChatMsg, ToolDef } from "@aa/sdk";
import { health, listTools, chat } from "@aa/sdk";
import { copy } from "../../util/selection";
import { Sidebar } from "../../component/sidebar";
import { Prompt } from "../../component/prompt";

interface SessionProps {
  onBack: () => void;
  toast: {
    show: (input: { message: string; variant: "info" | "success" | "warning" | "error" }) => void;
    error: (err: unknown) => void;
  };
  sessionID?: string;
  model?: string;
  provider?: string;
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
  const [model, setModel] = createSignal(props.model ?? "");
  const [provider, setProvider] = createSignal(props.provider ?? "");

  createEffect(() => {
    const handler = (selection: any) => {
      if (selection && !selection.isDragging) {
        copy(renderer, props.toast);
      }
    };
    renderer.on(CliRenderEvents.SELECTION, handler);
    onCleanup(() => renderer.off(CliRenderEvents.SELECTION, handler));
  });

  onMount(async () => {
    try {
      const { data: h } = await health();
      if (h) {
        if (!props.model) setModel(h.model);
        if (!props.provider) setProvider(h.provider);
      }
    } catch {}
  });

  const [messages, setMessages] = createSignal<Msg[]>([]);
  const [streaming, setStreaming] = createSignal(false);
  const [status, setStatus] = createSignal<"connected" | "error">("connected");
  const [statusText, setStatusText] = createSignal("");

  const [tools] = createResource(() =>
    listTools()
      .then((r) => r.data ?? [])
      .catch<ToolDef[]>(() => []),
  );
  const threadId = props.sessionID || crypto.randomUUID();

  const chatHistory = createMemo(() => {
    const msgs = messages();
    const history: ChatMsg[] = [];
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

  const submit = async (text: string) => {
    if (!text.trim() || streaming()) return;

    setMessages((prev) => [...prev, { id: crypto.randomUUID(), role: "user", content: text }]);
    setStreaming(true);
    setStatusText("waiting for response...");

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

      for await (const event of chat(history, toolDefs, threadId)) {
        switch (event.type) {
          case "TEXT_MESSAGE_CONTENT":
            fullContent += event.delta;
            updateMsg(assistantId, { content: fullContent });
            setStatusText("streaming...");
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
            setStatusText(`running tool: ${event.toolCallName}...`);
            break;
          case "TOOL_CALL_ARGS":
            setMessages((prev) =>
              prev.map((m) =>
                m.id === event.toolCallId ? { ...m, content: m.content + event.delta } : m,
              ),
            );
            break;
          case "TOOL_CALL_END":
            {
              const preview =
                typeof event.result === "string"
                  ? event.result.length > 200
                    ? event.result.slice(0, 200) + "..."
                    : event.result
                  : JSON.stringify(event.result).slice(0, 200);
              updateMsg(event.toolCallId, {
                content: preview,
                isStreaming: false,
              });
            }
            setStatusText("");
            break;
          case "RUN_ERROR":
            updateMsg(assistantId, {
              content: `Error: ${event.message}`,
              isStreaming: false,
            });
            setStatus("error");
            setStatusText(event.message);
            setStreaming(false);
            return;
          case "RUN_FINISHED":
            updateMsg(assistantId, { isStreaming: false });
            setStatus("connected");
            setStatusText("");
            setStreaming(false);
            break;
        }
      }
    } catch (err) {
      updateMsg(assistantId, {
        content: `Error: ${err}`,
        isStreaming: false,
      });
      setStatus("error");
      setStatusText(String(err));
      setStreaming(false);
    }
  };

  function updateMsg(id: string, patch: Partial<Msg>) {
    setMessages((prev) => prev.map((m) => (m.id === id ? { ...m, ...patch } : m)));
  }

  return (
    <box width="100%" height="100%" flexDirection="row">
      {/* Sidebar */}
      <Sidebar
        currentSessionId={threadId}
        onNewSession={() => {
          window.location.reload();
        }}
        onSelectSession={(id) => {
          props.onBack();
          // navigate to different session via route
        }}
        onBack={props.onBack}
      />

      {/* Main content */}
      <box
        flexGrow={1}
        height="100%"
        flexDirection="column"
        on:keypress={(e) => {
          if (e.name === "escape") props.onBack();
        }}
      >
        {/* Header */}
        <box height={1} flexDirection="row">
          <text fg="#555">{threadId.slice(0, 8)}</text>
          <box flexGrow={1} />
          {status() === "error" ? (
            <text fg="red">● err</text>
          ) : streaming() ? (
            <text fg="yellow">● busy</text>
          ) : (
            <text fg="green">● ok</text>
          )}
          <text fg="#666"> Esc:back</text>
        </box>

        {/* Messages */}
        <scrollbox flexGrow={1} stickyScroll stickyStart="bottom">
          {messages().length === 0 ? (
            <box flexDirection="column">
              <box height={2} />
              <text fg="#666">Start a conversation. Type a message below.</text>
              <text fg="#666">I can read/write files, grep/search code,</text>
              <text fg="#666">run shell commands, and fetch web pages.</text>
              <box height={2} />
            </box>
          ) : (
            <For each={messages()}>
              {(msg) => (
                <box flexDirection="column">
                  {msg.isTool ? (
                    <box flexDirection="row">
                      <text fg="yellow">↻ {msg.toolName}</text>
                      <text fg="#888"> {msg.content || "running..."}</text>
                    </box>
                  ) : (
                    <>
                      <text fg={msg.role === "user" ? "#4ade80" : "#22d3ee"}>
                        {msg.role === "user" ? "You" : "AI"}
                      </text>
                      {msg.content ? (
                        <markdown content={msg.content} syntaxStyle={syntaxStyle} />
                      ) : null}
                      {msg.isStreaming && !msg.content ? <text fg="#666">▊</text> : null}
                    </>
                  )}
                  <box height={1} />
                </box>
              )}
            </For>
          )}
        </scrollbox>

        {/* Status bar */}
        <box height={1} flexDirection="row">
          {statusText() ? <text fg="#666">{statusText()}</text> : null}
          <box flexGrow={1} />
        </box>

        {/* Input */}
        <box flexShrink={0}>
          <Prompt model={model()} provider={provider()} onSubmit={(value) => submit(value)} />
        </box>
      </box>
    </box>
  );
};
