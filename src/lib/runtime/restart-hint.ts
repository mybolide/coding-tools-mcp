import { message } from "@tauri-apps/plugin-dialog";

export async function promptServiceRestart(
  serviceRunning: boolean,
  serviceLabel: string,
): Promise<void> {
  if (!serviceRunning) return;
  await message(`配置已保存。请停止并重新启动${serviceLabel}，更改才会生效。`, {
    title: "需要重启服务",
    kind: "info",
  });
}
