import { invoke } from "@tauri-apps/api/core";

export interface FrpProfileDto {
  id: string;
  name: string;
  server: string;
  serverPort: number;
  hasToken: boolean;
}

export interface FrpProfileInput {
  id: string;
  name: string;
  server: string;
  serverPort: number;
}

export async function listFrpProfiles(): Promise<FrpProfileDto[]> {
  return invoke<FrpProfileDto[]>("list_frp_profiles");
}

export async function saveFrpProfile(
  profile: FrpProfileInput,
  token?: string,
): Promise<FrpProfileDto> {
  return invoke<FrpProfileDto>("save_frp_profile", { profile, token });
}

export async function getLastWorkspaceId(): Promise<string> {
  return invoke<string>("get_last_workspace_id");
}

export async function setLastWorkspace(id: string): Promise<void> {
  return invoke("set_last_workspace", { id });
}

export async function deleteFrpProfile(id: string): Promise<void> {
  return invoke("delete_frp_profile", { id });
}
