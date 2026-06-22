import { createContext, useContext, type ParentProps, type JSX, Show, createEffect, onCleanup } from "solid-js"
import { createStore, reconcile } from "solid-js/store"
import { useTerminalDimensions } from "@opentui/solid"

// ── Dialog stack entry ──────────────────────────────────────────

type DialogEntry = {
  element: () => JSX.Element
  onClose?: () => void
}

// ── Dialog controller ────────────────────────────────────────────

const DIALOG_SIZES: Record<string, number> = {
  small: 40,
  medium: 60,
  large: 80,
  xlarge: 100,
}

function createDialog() {
  const [store, setStore] = createStore({
    stack: [] as DialogEntry[],
    size: "medium" as string,
  })

  function push(element: () => JSX.Element, onClose?: () => void) {
    setStore("stack", (s) => [...s, { element, onClose }])
  }

  function replace(element: () => JSX.Element, onClose?: () => void) {
    setStore("stack", reconcile([{ element, onClose }]))
  }

  function pop() {
    const top = store.stack[store.stack.length - 1]
    if (top?.onClose) top.onClose()
    setStore("stack", (s) => s.slice(0, -1))
  }

  function clear() {
    setStore("stack", reconcile([]))
  }

  function setSize(size: string) {
    setStore("size", size)
  }

  return { store, push, replace, pop, clear, setSize }
}

type DialogCtx = ReturnType<typeof createDialog>

const ctx = createContext<DialogCtx>()

export function DialogProvider(props: ParentProps) {
  const value = createDialog()
  return <ctx.Provider value={value}>{props.children}</ctx.Provider>
}

export function useDialog() {
  const v = useContext(ctx)
  if (!v) throw new Error("useDialog must be used within DialogProvider")
  return v
}

// ── Dialog overlay renderer ─────────────────────────────────────

export function Dialog() {
  const dialog = useDialog()
  const dims = useTerminalDimensions()

  const isOpen = () => dialog.store.stack.length > 0
  const current = () => dialog.store.stack[dialog.store.stack.length - 1]
  const width = () => DIALOG_SIZES[dialog.store.size] ?? DIALOG_SIZES.medium

  return (
    <Show when={isOpen()}>
      <box
        position="absolute"
        top={0}
        left={0}
        width={dims().width}
        height={dims().height}
        backgroundColor="#000000dd"
        alignItems="center"
        justifyContent="center"
        on:keypress={(e: { name?: string }) => {
          if (e.name === "escape") dialog.pop()
        }}
      >
        <box
          width={width()}
          maxHeight={dims().height - 4}
          flexDirection="column"
          backgroundColor="#1a1a2e"
          borderColor="#3b82f6"
          border={["left", "right"]}
        >
          {current()?.element()}
        </box>
      </box>
    </Show>
  )
}
