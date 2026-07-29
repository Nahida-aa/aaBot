import { Switch, Match, createEffect, ErrorBoundary } from "solid-js";
import { render, useRenderer } from "@opentui/solid";
import { createDefaultOpenTuiKeymap } from "@opentui/keymap/opentui";
import { createCliRenderer, type CliRenderer, type CliRendererConfig } from "@opentui/core";
import * as Clipboard from "./util/clipboard";
import { client, health, deleteSession } from "@aa/sdk";
import { ToastProvider, useToast, Toast } from "./ui/toast";
import { DialogProvider, useDialog, Dialog } from "./ui/dialog";
import { ConfigDialog } from "./component/dialog/config";
import { SessionListDialog } from "./component/dialog/session-list";
import { WorkspaceListDialog } from "./component/dialog/workspace-list";
import { CommandPalette, type PaletteCommand } from "./component/command-palette";
import { Home } from "./routes/home";
import { Session } from "./routes/session";
import { ExitProvider, createExit, useExit, type Exit } from "./context/exit";
import { WorkspaceProvider } from "./context/workspace";
import { RouteProvider, useRoute } from "./context/route";
import { ArgsProvider, type Args } from "./context/args";

// ---------------------------------------------------------------------------
// Lifecycle types
// ---------------------------------------------------------------------------

export type TuiHandle = {
  ready: Promise<void>;
  done: Promise<void>;
  exit: Exit;
};

type TuiLifecycle = {
  exit: Exit;
  exited: Promise<void>;
  fail(error: unknown): Promise<never>;
};

// ---------------------------------------------------------------------------
// TuiInput & mount
// ---------------------------------------------------------------------------

type TuiInput = {
  url: string;
  args: Args;
  renderer: CliRenderer;
  onSnapshot?: () => Promise<string[]>;
  directory?: string;
  fetch?: typeof fetch;
  headers?: RequestInit["headers"];
  events?: EventSource;
};

export function tuiRendererConfig(): CliRendererConfig {
  return {
    backgroundColor: "#000000",
    exitOnCtrlC: true,
  };
}

export function createTuiRenderer() {
  return createCliRenderer(tuiRendererConfig());
}

export function tui(input: TuiInput): TuiHandle {
  const renderer = input.renderer;
  const keymap = createDefaultOpenTuiKeymap(renderer);

  const lifecycle = createTuiLifecycle({
    renderer,
    cleanup: async () => {},
  });

  const ready = mountTui({ ...input, keymap, exit: lifecycle.exit }).catch((error) =>
    lifecycle.fail(error),
  );
  const done = waitUntilDone(ready, lifecycle.exited);

  return { ready, done, exit: lifecycle.exit };
}

function createTuiLifecycle(input: {
  renderer: CliRenderer;
  cleanup: () => Promise<void>;
}): TuiLifecycle {
  let resolveExited!: () => void;
  const exited = new Promise<void>((resolve) => {
    resolveExited = resolve;
  });
  let exitCompleted = false;
  let exiting = false;
  let cleanupTask: Promise<void> | undefined;

  const completeExit = () => {
    if (exitCompleted) return;
    exitCompleted = true;
    resolveExited();
  };

  const cleanup = () => {
    cleanupTask ??= (async () => {
      process.off("SIGHUP", onSighup);
      process.off("SIGINT", onSighup);
      try {
        await input.cleanup();
      } finally {
        if (!input.renderer.isDestroyed) {
          input.renderer.setTerminalTitle("");
          input.renderer.destroy();
        }
      }
    })();
    return cleanupTask;
  };

  const exit = createExit(async (reason, message) => {
    exiting = true;
    await cleanup();
    const text = message();
    if (text) process.stdout.write(text + "\n");
    completeExit();
  });

  const onSighup = () => {
    void exit();
  };

  input.renderer.once("destroy", () => {
    if (exiting) return;
    void cleanup().finally(() => {
      completeExit();
    });
  });
  process.on("SIGHUP", onSighup);
  process.on("SIGINT", onSighup);

  return {
    exit,
    exited,
    async fail(error) {
      exiting = true;
      await cleanup().catch(() => {});
      if (!input.renderer.isDestroyed) input.renderer.destroy();
      completeExit();
      throw error;
    },
  };
}

async function waitUntilDone(ready: Promise<void>, exited: Promise<void>) {
  await ready;
  await exited;
}

async function mountTui(
  input: TuiInput & { keymap: ReturnType<typeof createDefaultOpenTuiKeymap>; exit: Exit },
) {
  const renderer = input.renderer;

  await render(() => {
    return (
      <ErrorBoundary
        fallback={(err) => {
          console.error("TUI mount error:", err);
          return <text>Error: {String(err)}</text>;
        }}
      >
        <ArgsProvider {...input.args}>
          <ExitProvider exit={input.exit}>
            <ToastProvider>
              <DialogProvider>
                <WorkspaceProvider>
                  <RouteProvider
                    initialRoute={
                      input.args.continue ? { type: "session", sessionID: "dummy" } : undefined
                    }
                  >
                    <App onSnapshot={input.onSnapshot} url={input.url} />
                  </RouteProvider>
                </WorkspaceProvider>
              </DialogProvider>
            </ToastProvider>
          </ExitProvider>
        </ArgsProvider>
      </ErrorBoundary>
    );
  }, renderer);
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

export const App = (props: { onSnapshot?: () => Promise<string[]>; url?: string }) => {
  const renderer = useRenderer();
  const toast = useToast();
  const dialog = useDialog();
  const route = useRoute();
  const exit = useExit();
  client.setConfig({ baseUrl: props.url ?? "http://localhost:3000" });

  const handlePaletteCommand = (cmd: PaletteCommand) => {
    switch (cmd.action) {
      case "config":
        health()
          .then((h) => {
            dialog.push(() => (
              <ConfigDialog
                current={{
                  provider: h.data?.provider ?? "",
                  model: h.data?.model ?? "",
                  baseUrl: "",
                  hasApiKey: false,
                }}
                onSave={(c) => {
                  toast.show({ message: `Config: ${c.provider}/${c.model}`, variant: "info" });
                }}
              />
            ));
          })
          .catch(() => {
            toast.show({ message: "Server not available", variant: "error" });
          });
        break;
      case "session-list":
        dialog.push(() => (
          <SessionListDialog
            onSelect={(id) => route.navigate({ type: "session", sessionID: id })}
          />
        ));
        break;
      case "delete-session":
        dialog.push(() => (
          <SessionListDialog
            onSelect={async (id) => {
              const { data } = await deleteSession({ path: { id } });
              toast.show({
                message: data ? "Session deleted" : "Delete failed",
                variant: data ? "info" : "error",
              });
            }}
          />
        ));
        break;
      case "workspace":
        dialog.push(() => <WorkspaceListDialog />);
        break;
      case "back-home":
        route.navigate({ type: "home" });
        break;
      case "new-session":
        route.navigate({ type: "session", sessionID: "" });
        break;
      case "help":
        toast.show({ message: "Ctrl+K: Palette · Esc: Close · ↑↓: Select", variant: "info" });
        break;
    }
  };

  const openPalette = () => {
    dialog.push(() => <CommandPalette onCommand={handlePaletteCommand} />);
  };

  createEffect(() => {
    renderer.console.onCopySelection = async (text: string) => {
      if (!text) return;
      await Clipboard.copy(text)
        .then(() => toast.show({ message: "Copied to clipboard", variant: "info" }))
        .catch(toast.error);
      renderer.clearSelection();
    };
  });

  return (
    <box
      width="100%"
      height="100%"
      flexDirection="column"
      on:keypress={(e: { name?: string; ctrl?: boolean }) => {
        if ((e.name === "k" || e.name === "K") && e.ctrl) {
          openPalette();
        } else if (e.name === "c" && e.ctrl) {
          void exit();
        }
      }}
    >
      <Switch>
        <Match when={route.data.type === "home"}>
          <Home
            onStart={() => route.navigate({ type: "session", sessionID: "" })}
            onContinue={(id) => route.navigate({ type: "session", sessionID: id })}
          />
        </Match>
        <Match when={route.data.type === "session"}>
          <Session
            onBack={() => route.navigate({ type: "home" })}
            toast={toast}
            sessionID={(route.data as { type: "session"; sessionID: string }).sessionID}
          />
        </Match>
      </Switch>
      <Dialog />
      <Toast />
    </box>
  );
};
