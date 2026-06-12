const BASE = "/api";

export interface ToolDef {
  name: string;
  description: string;
  parameters: unknown;
}

export interface ToolResult {
  content: string;
  is_error: boolean;
  metadata: Record<string, unknown>;
}

export async function healthCheck(): Promise<{ status: string; tool_count: number }> {
  const res = await fetch(`${BASE}/health`);
  return res.json();
}

export async function listTools(): Promise<ToolDef[]> {
  const res = await fetch(`${BASE}/tools`);
  return res.json();
}

export async function callTool(name: string, args: unknown): Promise<ToolResult> {
  const res = await fetch(`${BASE}/tools/${encodeURIComponent(name)}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ arguments: args }),
  });
  if (!res.ok) {
    const text = await res.text();
    return { content: `Error: ${text}`, is_error: true, metadata: {} };
  }
  return res.json();
}
