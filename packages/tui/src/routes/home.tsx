import { type Component, createSignal, onMount } from "solid-js";
import type { AaClient } from "@aa/sdk";

interface HomeProps {
  client: AaClient;
  onStart: () => void;
}

export const Home: Component<HomeProps> = (props) => {
  const [connected, setConnected] = createSignal(false);
  const [toolCount, setToolCount] = createSignal(0);

  onMount(async () => {
    try {
      const h = await props.client.health();
      setConnected(true);
      setToolCount(h.tool_count);
    } catch {
      setConnected(false);
    }
  });

  const submit = () => {
    props.onStart();
  };

  return (
    <box
      width="100%"
      height="100%"
      flexDirection="column"
      alignItems="center"
      justifyContent="center"
    >
      <text>aaBot – AI Assistant</text>

      <box height={1} />

      {connected()
        ? <text fg="green">● Connected ({toolCount()} tools)</text>
        : <text fg="red">● Disconnected</text>}

      <box height={2} />

      <box width="60%">
        <textarea
          placeholder="Type a message to start..."
          onSubmit={submit}
          minHeight={1}
          maxHeight={3}
          flexGrow={1}
        />
      </box>
    </box>
  );
};
