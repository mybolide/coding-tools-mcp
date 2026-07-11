import { invoke } from "@tauri-apps/api/core";

export type TunnelService = "mcp" | "actions";

export interface TunnelStatus {
  state: string;
  publicUrl: string;
  tunnelPid: number | null;
}

export async function getFrpSnippet(id: string, service: TunnelService): Promise<string> {
  return invoke<string>("get_frp_snippet", { id, service });
}

export async function startTunnel(id: string, service: TunnelService): Promise<TunnelStatus> {
  return invoke<TunnelStatus>("start_tunnel", { id, service });
}

export async function stopTunnel(id: string, service: TunnelService): Promise<TunnelStatus> {
  return invoke<TunnelStatus>("stop_tunnel", { id, service });
}

export async function restartTunnel(id: string, service: TunnelService): Promise<TunnelStatus> {
  return invoke<TunnelStatus>("restart_tunnel", { id, service });
}
