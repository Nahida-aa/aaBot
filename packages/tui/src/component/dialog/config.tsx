import { createSignal, Show } from "solid-js"
import { useDialog } from "../../ui/dialog"
import { DialogSelect, type SelectOption } from "../../ui/dialog-select"

interface ConfigData {
  provider: string
  model: string
  baseUrl: string
  hasApiKey: boolean
}

const PROVIDER_OPTIONS: SelectOption<string>[] = [
  { title: "Ollama", description: "Local LLMs (default)", value: "ollama", category: "Local" },
  { title: "OpenAI", description: "GPT-4o, GPT-4.1 series", value: "openai", category: "Cloud" },
  { title: "Custom", description: "Any OpenAI-compatible API", value: "custom", category: "Cloud" },
]

const MODELS_BY_PROVIDER: Record<string, SelectOption<string>[]> = {
  ollama: [
    { title: "gemma4:31b-cloud", description: "Default local model", value: "gemma4:31b-cloud" },
    { title: "llama3.3:70b", description: "Llama 3.3 70B", value: "llama3.3:70b" },
    { title: "mistral:latest", description: "Mistral latest", value: "mistral:latest" },
    { title: "codellama", description: "Code-focused Llama", value: "codellama" },
  ],
  openai: [
    { title: "gpt-4o-mini", description: "Fast & cheap (default)", value: "gpt-4o-mini" },
    { title: "gpt-4o", description: "Full GPT-4o", value: "gpt-4o" },
    { title: "gpt-4.1", description: "Latest GPT-4.1", value: "gpt-4.1" },
    { title: "gpt-4.1-mini", description: "GPT-4.1 mini", value: "gpt-4.1-mini" },
    { title: "gpt-4.1-nano", description: "GPT-4.1 nano (fastest)", value: "gpt-4.1-nano" },
  ],
  custom: [],
}

interface ConfigDialogProps {
  current: ConfigData
  onSave: (config: ConfigData) => void
}

export function ConfigDialog(props: ConfigDialogProps) {
  const dialog = useDialog()
  const [provider, setProvider] = createSignal(props.current.provider)
  const [model, setModel] = createSignal(props.current.model)
  const [baseUrl, setBaseUrl] = createSignal(props.current.baseUrl)

  const isDirty = () =>
    provider() !== props.current.provider ||
    model() !== props.current.model ||
    baseUrl() !== props.current.baseUrl

  return (
    <box flexDirection="column" paddingLeft={1} paddingRight={1} paddingTop={1} paddingBottom={1}>
      {/* Title */}
      <box height={1}>
        <text fg="#3b82f6">Config — Model & Provider</text>
      </box>

      <box height={1} />

      {/* Provider row */}
      <box height={1} flexDirection="row">
        <text fg="#888">Provider</text>
        <box width={2} />
        <text fg="#ccc">{provider()}</text>
        <box width={2} />
        <box
          on:press={() => {
            dialog.push(
              () => (
                <DialogSelect
                  title="Select Provider"
                  options={PROVIDER_OPTIONS}
                  placeholder="Search providers..."
                  onSelect={(v: string) => {
                    setProvider(v)
                    if (v === "ollama") setModel("gemma4:31b-cloud")
                    else if (v === "openai") setModel("gpt-4o-mini")
                    else setModel("")
                    if (v === "ollama") setBaseUrl("http://localhost:11434")
                    else if (v === "openai") setBaseUrl("https://api.openai.com/v1")
                    else setBaseUrl("")
                  }}
                />
              ),
            )
          }}
        >
          <text fg="#3b82f6">[change]</text>
        </box>
      </box>

      <box height={1} />

      {/* Model row */}
      <box height={1} flexDirection="row">
        <text fg="#888">Model</text>
        <box width={3} />
        <text fg="#ccc">{model() || "(none)"}</text>
        <box width={2} />
        <box
          on:press={() => {
            const models = MODELS_BY_PROVIDER[provider()] ?? []
            dialog.push(
              () => (
                <DialogSelect
                  title="Select Model"
                  options={models.length > 0 ? models : [{ title: "Custom model — type below", value: "" }]}
                  placeholder="Type or select model..."
                  emptyMessage="Type a custom model name"
                  onSelect={(v: string) => {
                    if (v) setModel(v)
                  }}
                />
              ),
              () => {
                // If user presses Esc without selecting, allow custom model entry
                const current = model()
                if (!current || current === "(default)") {
                  dialog.push(() => (
                    <box flexDirection="column" paddingLeft={1} paddingRight={1} paddingTop={1} paddingBottom={1}>
                      <box height={1}><text fg="#3b82f6">Enter Model Name</text></box>
                      <box height={1} />
                      <box height={1} flexDirection="row">
                        <text fg="#888">&gt; </text>
                        <textarea
                          initialValue={model()}
                          placeholder="e.g. gpt-4o-mini"
                          placeholderColor="#555"
                          minHeight={1}
                          maxHeight={1}
                          flexGrow={1}
                          onContentChange={(v) => {
                            if (typeof v === "string") setModel(v)
                          }}
                        />
                      </box>
                      <box height={1} />
                      <text fg="#555">Enter to confirm</text>
                    </box>
                  ))
                }
              },
            )
          }}
        >
          <text fg="#3b82f6">[change]</text>
        </box>
      </box>

      <box height={1} />

      {/* Base URL row */}
      <box height={1} flexDirection="row">
        <text fg="#888">Base URL</text>
        <box width={2} />
        <textarea
          initialValue={baseUrl()}
          placeholder="https://api.openai.com/v1"
          placeholderColor="#555"
          minHeight={1}
          maxHeight={1}
          flexGrow={1}
          onContentChange={(v) => {
            if (typeof v === "string") setBaseUrl(v)
          }}
        />
      </box>

      <box height={1} />

      {/* API key status */}
      <box height={1} flexDirection="row">
        <text fg="#888">API Key</text>
        <box width={2} />
        <text fg={props.current.hasApiKey ? "#4ade80" : "#f87171"}>
          {props.current.hasApiKey ? "✓ Set" : "✗ Not set"}
        </text>
      </box>

      <box height={1} />
      <box height={1} borderColor="#555" border={["top"]} />

      {/* Footer */}
      <box height={1} flexDirection="row" justifyContent="flex-end">
        <Show when={isDirty()}>
          <box
            backgroundColor="#3b82f6"
            paddingLeft={1}
            paddingRight={1}
            on:press={() => {
              props.onSave({ provider: provider(), model: model(), baseUrl: baseUrl(), hasApiKey: props.current.hasApiKey })
              dialog.pop()
            }}
          >
            <text fg="#000"> Save </text>
          </box>
          <box width={2} />
        </Show>
        <box
          paddingLeft={1}
          paddingRight={1}
          on:press={() => dialog.pop()}
        >
          <text fg="#888">Cancel</text>
        </box>
      </box>
    </box>
  )
}
