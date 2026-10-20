import { homeDir } from "@tauri-apps/api/path";
import { open } from "@tauri-apps/plugin-dialog";

/**
 * Folder picker. macOS NSOpenPanel defaults to Documents/文稿, which
 * retriggers TCC even when Full Disk Access is already granted.
 */
export async function pickWorkspaceDirectory(
  defaultPath?: string,
): Promise<string | null> {
  const trimmed = defaultPath?.trim();
  const fallback = trimmed && trimmed.length > 0 ? trimmed : await homeDir();
  const selected = await open({
    directory: true,
    multiple: false,
    defaultPath: fallback,
  });
  if (!selected || Array.isArray(selected)) {
    return null;
  }
  return selected;
}
