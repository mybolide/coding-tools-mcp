import { invoke } from "@tauri-apps/api/core";

export interface SoftwareStatus {
  kind: string;
  name: string;
  installed: boolean;
  path: string;
  managed: boolean;
}

export interface DownloadConfig {
  githubMirror: string;
  proxyMode: string;
  proxyUrl: string;
}

export async function listSoftware(): Promise<SoftwareStatus[]> {
  return invoke("list_software");
}

export async function installSoftware(kind: string): Promise<SoftwareStatus> {
  return invoke("install_software", { kind });
}

export async function uninstallSoftware(kind: string): Promise<SoftwareStatus> {
  return invoke("uninstall_software", { kind });
}

export async function getDownloadConfig(): Promise<DownloadConfig> {
  return invoke("get_download_config");
}

export async function setDownloadConfig(config: DownloadConfig): Promise<void> {
  return invoke("set_download_config", { config });
}
