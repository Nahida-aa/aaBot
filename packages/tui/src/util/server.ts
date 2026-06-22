import { spawn, type ChildProcess } from "child_process";
import { resolve } from "path";
import { existsSync } from "fs";

/**
 * Ensure the server is running at the given URL.
 * - Checks if already running via /health
 * - If not, spawns a new server process
 * - Waits for it to be ready
 * Returns a cleanup function to stop the server on exit.
 */
export async function ensureServer(serverUrl: string): Promise<() => void> {
  const alreadyRunning = await checkRunning(serverUrl);

  if (alreadyRunning) {
    console.log("[tui] Server already running");
    return () => {}; // no cleanup needed
  }

  console.log("[tui] Starting server...");
  const proc = startServer();

  try {
    await waitForReady(serverUrl);
    console.log("[tui] Server ready");
    return () => {
      console.log("[tui] Stopping server...");
      proc.kill("SIGTERM");
      setTimeout(() => {
        try { proc.kill("SIGKILL"); } catch {}
      }, 3000);
    };
  } catch (err) {
    proc.kill("SIGTERM");
    throw err;
  }
}

async function checkRunning(serverUrl: string): Promise<boolean> {
  try {
    const res = await fetch(`${serverUrl}/health`, { signal: AbortSignal.timeout(2000) });
    return res.ok;
  } catch {
    return false;
  }
}

function startServer(): ChildProcess {
  // Resolve workspace root from this file's location
  // src/util/server.ts -> src/util -> src -> packages/tui -> packages -> <workspace root>
  const workspaceRoot = resolve(import.meta.dir, "..", "..", "..", "..");

  const debugPath = resolve(workspaceRoot, "target/debug/aa-server");
  const releasePath = resolve(workspaceRoot, "target/release/aa-server");
  const serverBin = existsSync(debugPath) ? debugPath : releasePath;

  const proc = spawn(serverBin, [], {
    stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env },
  });

  proc.stdout?.on("data", (data: Buffer) => {
    const msg = data.toString().trim();
    if (msg) console.log(`[server] ${msg}`);
  });

  proc.stderr?.on("data", (data: Buffer) => {
    const msg = data.toString().trim();
    if (msg) console.error(`[server:err] ${msg}`);
  });

  proc.on("exit", (code) => {
    console.error(`[server] exited with code ${code}`);
  });

  return proc;
}

async function waitForReady(serverUrl: string, timeoutMs = 15000): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if (await checkRunning(serverUrl)) return;
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error("Server failed to start within timeout");
}
