import { spawn, type ChildProcess } from "child_process";
import { resolve } from "path";

const SERVER_URL = "http://localhost:3000";
const HEALTH_PATH = `${SERVER_URL}/health`;

/**
 * Check if the Rust server is already running.
 */
async function checkRunning(): Promise<boolean> {
  try {
    const res = await fetch(HEALTH_PATH, { signal: AbortSignal.timeout(2000) });
    return res.ok;
  } catch {
    return false;
  }
}

/**
 * Start the aa-server process.
 * Returns the child process handle.
 */
function startServer(): ChildProcess {
  // Determine the path to the aa-server binary
  const workspaceRoot = resolve(import.meta.dir, "..", "..", "..", "..");
  const serverBin = resolve(workspaceRoot, "target/debug/aa-server");

  const proc = spawn(serverBin, [], {
    stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env },
  });

  proc.stdout?.on("data", (data: Buffer) => {
    // Only forward important messages, not ANSI noise
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

/**
 * Wait for the server to become healthy.
 * Polls `/health` until it responds or timeout.
 */
async function waitForReady(timeoutMs = 15000): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if (await checkRunning()) return;
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error("Server failed to start within timeout");
}

/**
 * Ensure the server is running.
 * - Checks if already running
 * - If not, spawns a new server process
 * - Waits for it to be ready
 * Returns a cleanup function to stop the server on exit.
 */
export async function ensureServer(): Promise<() => void> {
  const alreadyRunning = await checkRunning();

  if (alreadyRunning) {
    console.log("[tui] Server already running");
    return () => {}; // no cleanup needed
  }

  console.log("[tui] Starting server...");
  const proc = startServer();

  try {
    await waitForReady();
    console.log("[tui] Server ready");
    return () => {
      console.log("[tui] Stopping server...");
      proc.kill("SIGTERM");
      // Force kill after 3 seconds if not stopped
      setTimeout(() => {
        try { proc.kill("SIGKILL"); } catch {}
      }, 3000);
    };
  } catch (err) {
    proc.kill("SIGTERM");
    throw err;
  }
}
