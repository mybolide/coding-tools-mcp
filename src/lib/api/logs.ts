import { invoke } from "@tauri-apps/api/core";

export interface LogChunk {
  name: string;
  content: string;
}

export type LogService = "mcp" | "actions";

export async function readWorkspaceLogs(
  workspaceId: string,
  service: LogService,
): Promise<LogChunk[]> {
  return invoke<LogChunk[]>("read_workspace_logs", { id: workspaceId, service });
}
