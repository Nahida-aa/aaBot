import { type Component, createSignal, createResource, For, Show, onMount } from "solid-js";
import type { AaClient, SessionSummary } from "@aa/sdk";

interface HomeProps {
  client: AaClient;
  onStart: () => void;
  onContinue: (sessionId: string) => void;
}

export const Home: Component<HomeProps> = (props) => {
  const [connected, setConnected] = createSignal(false);
  const [toolCount, setToolCount] = createSignal(0);
  const [sessions] = createResource(() => props.client.listSessions());

  onMount(async () => {
    try {
      const h = await props.client.health();
      setConnected(true);
      setToolCount(h.tool_count);
    } catch {
      setConnected(false);
    }
  });

  const recentSessions = () => {
    const s = sessions();
    return s ? s.slice(0, 5) : [];
  };

  return (
    <box
      width="100%"
      height="100%"
      flexDirection="column"
    >
      {/* Header */}
      <box height={1} flexDirection="row">
        <text fg="cyan">aaBot</text>
        <box flexGrow={1} />
        {connected()
          ? <text fg="green">● Connected ({toolCount()} tools)</text>
          : <text fg="red">● Disconnected</text>}
      </box>

      {/* Main content */}
      <box flexGrow={1} flexDirection="column" alignItems="center" justifyContent="center">
        <text fg="cyan">aaBot – AI Assistant</text>
        <box height={2} />

        <Show when={recentSessions().length > 0}>
          <text fg="#888">Recent sessions:</text>
          <For each={recentSessions()}>
            {(s) => (
              <box
                flexDirection="row"
                on:press={() => props.onContinue(s.session_id)}
              >
                <text fg="#aaa">{s.session_id.slice(0, 8)}</text>
                <text fg="#666"> {s.model} · {s.message_count} msgs</text>
              </box>
            )}
          </For>
          <box height={2} />
        </Show>

        <box width="60%">
          <textarea
            placeholder="Type a message to start a new session..."
            onSubmit={() => props.onStart()}
            minHeight={1}
            maxHeight={3}
            flexGrow={1}
          />
        </box>

        <box height={2} />
        <text fg="#555">↑↓ select session · Enter to start · Esc:back</text>
      </box>
    </box>
  );
};
