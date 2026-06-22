import { type Component, createSignal, createResource, For, Show, onMount } from "solid-js";
import type { AaClient, SessionSummary } from "@aa/sdk";
import { useDialog } from "../ui/dialog";
import { ConfigDialog } from "../component/dialog/config";
import { SessionListDialog } from "../component/dialog/session-list";
import { CommandPalette, type PaletteCommand } from "../component/command-palette";
import { useToast } from "../ui/toast";

interface HomeProps {
  client: AaClient;
  onStart: () => void;
  onContinue: (sessionId: string) => void;
}

export const Home: Component<HomeProps> = (props) => {
  const [connected, setConnected] = createSignal(false);
  const [toolCount, setToolCount] = createSignal(0);
  const [provider, setProvider] = createSignal("");
  const [model, setModel] = createSignal("");
  const [sessions] = createResource(() => props.client.listSessions());
  const dialog = useDialog();
  const toast = useToast();

  onMount(async () => {
    try {
      const h = await props.client.health();
      setConnected(true);
      setToolCount(h.tool_count);
      setProvider(h.provider);
      setModel(h.model);
    } catch {
      setConnected(false);
    }
  });

  const recentSessions = () => {
    const s = sessions();
    return s ? s.slice(0, 5) : [];
  };

  const openConfig = () => {
    dialog.push(() => (
      <ConfigDialog
        current={{ provider: provider(), model: model(), baseUrl: "", hasApiKey: false }}
        onSave={(c) => {
          setProvider(c.provider)
          setModel(c.model)
          toast.show({ message: `Config: ${c.provider}/${c.model}`, variant: "info" })
        }}
      />
    ))
  }

  const openPalette = () => {
    dialog.push(
      () => (
        <CommandPalette
          onCommand={(cmd: PaletteCommand) => {
            switch (cmd.action) {
              case "config": openConfig(); break
              case "session-list":
                dialog.push(() => (
                  <SessionListDialog
                    client={props.client}
                    onSelect={(id) => props.onContinue(id)}
                  />
                ))
                break
              case "back-home": /* already home */ break
              case "new-session": props.onStart(); break
              case "help":
                toast.show({ message: "Ctrl+K: Palette · Esc: Close · ↑↓: Select", variant: "info" })
                break
            }
          }}
        />
      ),
    )
  }

  return (
    <box
      width="100%"
      height="100%"
      flexDirection="column"
    >
      {/* Header */}
      <box height={1} flexDirection="row">
        <text fg="cyan">aaBot</text>
        <box width={1} />
        <Show when={connected()}>
          <text fg="#555">{provider()}/{model()}</text>
        </Show>
        <box flexGrow={1} />
        {connected()
          ? <text fg="green">● {toolCount()} tools</text>
          : <text fg="red">● Disconnected</text>}
        <box width={1} />
        <box on:press={openPalette}>
          <text fg="#3b82f6">[Ctrl+K]</text>
        </box>
        <box width={1} />
        <box on:press={openConfig}>
          <text fg="#3b82f6">[Config]</text>
        </box>
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
