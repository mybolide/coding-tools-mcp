import { invoke } from "@tauri-apps/api/core";

export type WorkspaceSecretKey =
  | "oauth_client_secret"
  | "oauth_password"
  | "oauth_token_secret"
  | "bearer_token"
  | "cloudflare_token"
  | "actions_cloudflare_token"
  | "actions_api_key"
  | "actions_oauth_client_secret"
  | "actions_oauth_password"
  | "actions_oauth_token_secret"
  | "actions_frp_token"
  | "frp_token";

export async function getWorkspaceSecret(
  id: string,
  key: WorkspaceSecretKey,
): Promise<string | null> {
  return invoke<string | null>("get_workspace_secret", { id, key });
}

export async function setWorkspaceSecret(
  id: string,
  key: WorkspaceSecretKey,
  value: string,
): Promise<void> {
  return invoke("set_workspace_secret", { id, key, value });
}

export async function regenerateWorkspaceSecret(
  id: string,
  key: WorkspaceSecretKey,
): Promise<string> {
  return invoke<string>("regenerate_workspace_secret", { id, key });
}

/** @deprecated use WorkspaceSecretKey */
export type SecretKey = WorkspaceSecretKey;

/** @deprecated use getWorkspaceSecret */
export const getSecret = getWorkspaceSecret;

/** @deprecated use setWorkspaceSecret */
export const setSecret = setWorkspaceSecret;

/** @deprecated use regenerateWorkspaceSecret */
export const regenerateSecret = regenerateWorkspaceSecret;

// ── Shared secrets ───────────────────────────────────────────────────────

export type SharedSecretKey =
  | "bearer_token"
  | "oauth_client_secret"
  | "oauth_password"
  | "oauth_token_secret"
  | "actions_api_key"
  | "actions_oauth_client_secret"
  | "actions_oauth_password"
  | "actions_oauth_token_secret";

export async function getSharedSecret(key: SharedSecretKey): Promise<string | null> {
  return invoke<string | null>("get_shared_secret", { key });
}

export async function setSharedSecret(key: SharedSecretKey, value: string): Promise<void> {
  return invoke("set_shared_secret", { key, value });
}

export async function regenerateSharedSecret(key: SharedSecretKey): Promise<string> {
  return invoke<string>("regenerate_shared_secret", { key });
}

export async function secretIsSet(id: string, key: WorkspaceSecretKey): Promise<boolean> {
  const value = await getWorkspaceSecret(id, key);
  return Boolean(value);
}
