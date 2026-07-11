import { message } from "@tauri-apps/plugin-dialog";
import type { RuntimeStatus } from "$lib/types";

export function isPortConflictError(error: unknown): boolean {
  const text = error instanceof Error ? error.message : String(error);
  return text.includes("已被占用");
}

export async function alertPortConflict(error: unknown): Promise<void> {
  const text = error instanceof Error ? error.message : String(error);
  await message(text, { title: "端口不可用", kind: "warning" });
}

export async function runServiceToggle(
  running: boolean,
  start: () => Promise<RuntimeStatus>,
  stop: () => Promise<RuntimeStatus>,
): Promise<RuntimeStatus | null> {
  try {
    return running ? await stop() : await start();
  } catch (error) {
    if (isPortConflictError(error)) {
      await alertPortConflict(error);
      return null;
    }
    throw error;
  }
}
