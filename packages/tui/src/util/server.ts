/**
 * Ensure the server is running at the given URL.
 *
 * In local mode, the CLI starts the server in-process before spawning
 * the TUI and passes the URL via AA_SERVER_URL. In attach mode, the
 * remote URL is passed the same way.
 *
 * This function just verifies the server is reachable.
 */
export async function ensureServer(serverUrl: string): Promise<() => void> {
  const running = await checkRunning(serverUrl);

  if (!running) {
    throw new Error(
      `Server not reachable at ${serverUrl}.\n` +
        "Start the server first:\n" +
        "  aa serve\n" +
        "Or connect to a remote server:\n" +
        "  aa attach <url>",
    );
  }

  console.log(`[tui] Connected to server at ${serverUrl}`);
  return () => {}; // server lifecycle managed by CLI
}

async function checkRunning(serverUrl: string): Promise<boolean> {
  try {
    const res = await fetch(`${serverUrl}/health`, { signal: AbortSignal.timeout(2000) });
    return res.ok;
  } catch {
    return false;
  }
}
