import { type Component, createMemo, createSignal, onCleanup, onMount, Show } from "solid-js";
import { useTerminalDimensions } from "@opentui/solid";

export interface PromptRef {
  current: { input: string };
  submit: () => void;
  set: (info: { input: string }) => void;
  focus: () => void;
}

interface PromptProps {
  ref?: (r: PromptRef | undefined) => void;
  onSubmit?: (value: string) => void;
  onContentChange?: (value: string) => void;
  initialValue?: string;
  disabled?: boolean;
  visible?: boolean;
  right?: any;
  placeholders?: { normal: string[] };
  showPlaceholder?: boolean;
  model?: string;
  provider?: string;
}

export const Prompt: Component<PromptProps> = (props) => {
  const dimensions = useTerminalDimensions();
  let input: any;
  const [text, setText] = createSignal(props.initialValue ?? "");

  const maxHeight = createMemo(() =>
    Math.max(6, Math.floor(dimensions().height / 3)),
  );

  const placeholderText = createMemo(() => {
    if (props.showPlaceholder === false) return undefined;
    if (!props.placeholders?.normal?.length) return "Ask anything...";
    const idx = Math.floor((Date.now() / 6500) % props.placeholders.normal.length);
    return `Ask anything... "${props.placeholders.normal[idx]}"`;
  });

  const ref: PromptRef = {
    get current() { return { input: text() }; },
    submit: () => input?.submit?.(),
    set: (info) => {
      setText(info.input);
      if (input) input.setText?.(info.input);
    },
    focus: () => input?.focus?.(),
  };

  onMount(() => {
    props.ref?.(ref);
    onCleanup(() => props.ref?.(undefined));
  });

  const handleContentChange = (v: any) => {
    const val = typeof v === "string" ? v : v?.plainText ?? "";
    setText(val);
    props.onContentChange?.(val);
  };

  const handleSubmit = () => {
    const val = text().trim();
    if (val) {
      setText("");
      if (input) input.setText?.("");
    }
    props.onSubmit?.(val);
  };

  return (
    <box width="100%">
      <box width="100%" border={["left"]} borderColor={"#3b82f6"}>
        <box
          paddingLeft={2}
          paddingRight={2}
          paddingTop={1}
          flexShrink={0}
          backgroundColor="#0d1117"
          flexGrow={1}
          width="100%"
        >
          <textarea
            width="100%"
            placeholder={placeholderText()}
            placeholderColor="#555"
            textColor="#c9d1d9"
            focusedTextColor="#c9d1d9"
            minHeight={1}
            maxHeight={maxHeight()}
            ref={(r: any) => { input = r; }}
            onContentChange={handleContentChange}
            onSubmit={handleSubmit}
          />
          <box flexDirection="row" flexShrink={0} paddingTop={1} gap={1} justifyContent="space-between">
            <box flexDirection="row" gap={1}>
              <Show when={(props.model ?? props.provider) ? true : false}>
                <text fg="#555">{props.model}</text>
                <Show when={props.provider}>
                  <text fg="#555">· {props.provider}</text>
                </Show>
              </Show>
            </box>
            <Show when={props.right}>
              <box flexDirection="row" gap={1} alignItems="center">
                {props.right}
              </box>
            </Show>
          </box>
        </box>
      </box>
      <box
        height={1}
        border={["left"]}
        borderColor={"#3b82f6"}
      >
        <box height={1} border={["bottom"]} borderColor="#0d1117" />
      </box>
    </box>
  );
};
