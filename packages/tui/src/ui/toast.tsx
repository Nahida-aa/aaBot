import { createContext, useContext, type ParentProps, Show } from "solid-js"
import { createStore } from "solid-js/store"
import { useTerminalDimensions } from "@opentui/solid"
import { TextAttributes } from "@opentui/core"

type ToastInput = {
  title?: string
  message: string
  variant: "info" | "success" | "warning" | "error"
  duration?: number
}

type ToastOptions = Required<ToastInput>

const DEFAULT_TOAST_DURATION = 5000

const variantColors: Record<string, string> = {
  info: "#3b82f6",
  success: "#22c55e",
  warning: "#eab308",
  error: "#ef4444",
}

const toastBg = "#1a1a2e"
const toastText = "#e0e0e0"

function init() {
  const [store, setStore] = createStore({
    currentToast: null as ToastOptions | null,
  })

  let timeoutHandle: ReturnType<typeof setTimeout> | null = null

  const toast = {
    show(options: ToastInput) {
      const toastOptions: ToastOptions = {
        title: options.title ?? "",
        message: options.message,
        variant: options.variant,
        duration: options.duration ?? DEFAULT_TOAST_DURATION,
      }
      setStore("currentToast", toastOptions)
      if (timeoutHandle) clearTimeout(timeoutHandle)
      timeoutHandle = setTimeout(() => {
        setStore("currentToast", null)
      }, toastOptions.duration).unref?.()
    },
    error: (err: unknown) => {
      if (err instanceof Error)
        return toast.show({ variant: "error", message: err.message })
      toast.show({ variant: "error", message: "An unknown error has occurred" })
    },
    get currentToast(): ToastOptions | null {
      return store.currentToast
    },
  }
  return toast
}

type ToastContext = ReturnType<typeof init>

const ctx = createContext<ToastContext>()

export function ToastProvider(props: ParentProps) {
  const value = init()
  return <ctx.Provider value={value}>{props.children}</ctx.Provider>
}

export function useToast() {
  const value = useContext(ctx)
  if (!value) {
    throw new Error("useToast must be used within a ToastProvider")
  }
  return value
}

export function Toast() {
  const toast = useToast()
  const dimensions = useTerminalDimensions()

  return (
    <Show when={toast.currentToast}>
      {(current) => (
        <box
          position="absolute"
          justifyContent="center"
          alignItems="flex-start"
          top={2}
          right={2}
          maxWidth={Math.min(60, dimensions().width - 6)}
          paddingLeft={2}
          paddingRight={2}
          paddingTop={1}
          paddingBottom={1}
          backgroundColor={toastBg}
          borderColor={variantColors[current().variant]}
          border={["left", "right"]}
        >
          <Show when={current().title}>
            <text attributes={TextAttributes.BOLD} marginBottom={1} fg={toastText}>
              {current().title}
            </text>
          </Show>
          <text fg={toastText} wrapMode="word" width="100%">
            {current().message}
          </text>
        </box>
      )}
    </Show>
  )
}
