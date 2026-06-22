import { Switch, Match, createEffect, ErrorBoundary } from "solid-js";
import { render, useRenderer } from "@opentui/solid";
import { createDefaultOpenTuiKeymap } from "@opentui/keymap/opentui";
import { createCliRenderer, type CliRenderer, type CliRendererConfig } from "@opentui/core";
import * as Clipboard from "./util/clipboard";
import { AaClient } from "@aa/sdk";
import { ToastProvider, useToast, Toast } from "./ui/toast";
import { Home } from "./routes/home";
import { Session } from "./routes/session";
import { ExitProvider, createExit, type Exit } from "./context/exit";
import { RouteProvider, useRoute } from "./context/route";
import { ArgsProvider, type Args } from "./context/args";

// ---------------------------------------------------------------------------
// Lifecycle types
// ---------------------------------------------------------------------------

export type TuiHandle = {
  ready: Promise<void>
  done: Promise<void>
  exit: Exit
}

type TuiLifecycle = {
  exit: Exit
  exited: Promise<void>
  fail(error: unknown): Promise<never>
}

// ---------------------------------------------------------------------------
// TuiInput & mount
// ---------------------------------------------------------------------------

type TuiInput = {
  url: string
  args: Args
  renderer: CliRenderer
  onSnapshot?: () => Promise<string[]>
  directory?: string
  fetch?: typeof fetch
  headers?: RequestInit["headers"]
  events?: EventSource
}

export function tuiRendererConfig(): CliRendererConfig {
  return {
    backgroundColor: "#000000",
    exitOnCtrlC: false,
  }
}

export function createTuiRenderer() {
  return createCliRenderer(tuiRendererConfig())
}

export function tui(input: TuiInput): TuiHandle {
  const renderer = input.renderer
  const keymap = createDefaultOpenTuiKeymap(renderer)

  const lifecycle = createTuiLifecycle({
    renderer,
    cleanup: async () => {},
  })

  const ready = mountTui({ ...input, keymap, exit: lifecycle.exit }).catch((error) => lifecycle.fail(error))
  const done = waitUntilDone(ready, lifecycle.exited)

  return { ready, done, exit: lifecycle.exit }
}

function createTuiLifecycle(input: {
  renderer: CliRenderer
  cleanup: () => Promise<void>
}): TuiLifecycle {
  let resolveExited!: () => void
  const exited = new Promise<void>((resolve) => {
    resolveExited = resolve
  })
  let exitCompleted = false
  let exiting = false
  let cleanupTask: Promise<void> | undefined

  const completeExit = () => {
    if (exitCompleted) return
    exitCompleted = true
    resolveExited()
  }

  const cleanup = () => {
    cleanupTask ??= (async () => {
      process.off("SIGHUP", onSighup)
      try {
        await input.cleanup()
      } finally {
        if (!input.renderer.isDestroyed) {
          input.renderer.setTerminalTitle("")
          input.renderer.destroy()
        }
      }
    })()
    return cleanupTask
  }

  const exit = createExit(async (reason, message) => {
    exiting = true
    await cleanup()
    const text = message()
    if (text) process.stdout.write(text + "\n")
    completeExit()
  })

  const onSighup = () => {
    void exit()
  }

  input.renderer.once("destroy", () => {
    if (exiting) return
    void cleanup().finally(() => {
      completeExit()
    })
  })
  process.on("SIGHUP", onSighup)

  return {
    exit,
    exited,
    async fail(error) {
      exiting = true
      await cleanup().catch(() => {})
      if (!input.renderer.isDestroyed) input.renderer.destroy()
      completeExit()
      throw error
    },
  }
}

async function waitUntilDone(ready: Promise<void>, exited: Promise<void>) {
  await ready
  await exited
}

async function mountTui(input: TuiInput & { keymap: ReturnType<typeof createDefaultOpenTuiKeymap>; exit: Exit }) {
  const renderer = input.renderer

  await render(() => {
    return (
      <ErrorBoundary fallback={(err) => {
        console.error("TUI mount error:", err);
        return <text>Error: {String(err)}</text>;
      }}>
        <ArgsProvider {...input.args}>
          <ExitProvider exit={input.exit}>
            <ToastProvider>
              <RouteProvider
                initialRoute={
                  input.args.continue
                    ? { type: "session", sessionID: "dummy" }
                    : undefined
                }
              >
                <App onSnapshot={input.onSnapshot} url={input.url} />
              </RouteProvider>
            </ToastProvider>
          </ExitProvider>
        </ArgsProvider>
      </ErrorBoundary>
    )
  }, renderer)
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

export const App = (props: { onSnapshot?: () => Promise<string[]>; url?: string }) => {
  const renderer = useRenderer();
  const toast = useToast();
  const route = useRoute();
  const client = new AaClient(props.url ?? "http://localhost:3000");

  createEffect(() => {
    renderer.console.onCopySelection = async (text: string) => {
      if (!text) return
      await Clipboard.copy(text)
        .then(() => toast.show({ message: "Copied to clipboard", variant: "info" }))
        .catch(toast.error)
      renderer.clearSelection()
    }
  })

  return (
    <box width="100%" height="100%" flexDirection="column">
      <Switch>
        <Match when={route.data.type === "home"}>
          <Home client={client} onStart={() => route.navigate({ type: "session", sessionID: "" })} />
        </Match>
        <Match when={route.data.type === "session"}>
          <Session client={client} onBack={() => route.navigate({ type: "home" })} toast={toast} />
        </Match>
      </Switch>
      <Toast />
    </box>
  );
};
