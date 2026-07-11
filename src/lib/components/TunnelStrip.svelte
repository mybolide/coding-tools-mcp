<script lang="ts">
  import { message } from "@tauri-apps/plugin-dialog";
  import { getFrpSnippet, startTunnel, stopTunnel, type TunnelStatus } from "$lib/api/tunnel";
  import CopyButton from "$lib/components/CopyButton.svelte";

  interface Props {
    workspaceId: string;
    service: "mcp" | "actions";
    tunnelType: string;
    publicUrl: string;
    onPublicUrlChange?: (url: string) => void;
  }

  let {
    workspaceId,
    service,
    tunnelType,
    publicUrl,
    onPublicUrlChange,
  }: Props = $props();

  let status = $state<TunnelStatus | null>(null);
  let busy = $state(false);
  let frpSnippet = $state("");

  const running = $derived(status?.state === "running");
  const displayUrl = $derived(status?.publicUrl || publicUrl);

  async function toggleTunnel() {
    if (busy) return;
    busy = true;
    try {
      status = running
        ? await stopTunnel(workspaceId, service)
        : await startTunnel(workspaceId, service);
      if (status.publicUrl) {
        onPublicUrlChange?.(status.publicUrl);
      }
    } catch (error) {
      await message(String(error), { title: "隧道操作失败", kind: "error" });
    } finally {
      busy = false;
    }
  }

  async function loadFrpSnippet() {
    try {
      frpSnippet = await getFrpSnippet(workspaceId, service);
    } catch (error) {
      await message(String(error), { title: "无法生成 FRP 配置", kind: "error" });
    }
  }
</script>

<div class="tx-panel px-3 py-3">
  <div class="flex items-center justify-between gap-2">
    <div>
      <p class="text-xs font-medium text-[var(--color-text-secondary)]">远程隧道</p>
      <p class="text-[11px] text-[var(--color-text-muted)]">
        {tunnelType === "cloudflare" ? "Cloudflare" : tunnelType === "frp" ? "FRP" : "未配置"}
      </p>
    </div>
    {#if tunnelType === "frp" || tunnelType === "cloudflare"}
      <button
        type="button"
        class="tx-btn-ghost px-2.5 py-1 text-xs disabled:opacity-50"
        disabled={busy}
        onclick={toggleTunnel}
      >
        {busy ? "…" : running ? "断开" : "连接"}
      </button>
    {/if}
  </div>

  {#if displayUrl}
    <div class="mt-2 flex items-center justify-between gap-2">
      <p class="truncate font-mono text-xs">{displayUrl}</p>
      <CopyButton value={displayUrl} label="复制" />
    </div>
  {/if}

  {#if tunnelType === "cloudflare"}
    <p class="mt-2 text-[11px] text-[var(--color-text-muted)]">
      Cloudflare 由应用自动启动 cloudflared；Quick 模式会从日志解析 trycloudflare.com 地址。
    </p>
  {/if}

  {#if tunnelType === "frp"}
    <p class="mt-2 text-[11px] text-[var(--color-text-muted)]">
      FRP 由应用自动启动 frpc。在全局「FRP 配置」中设置服务器后，工作区选择配置并填写子域名即可连接。
    </p>
        <button
      type="button"
      class="mt-2 text-xs text-[var(--color-accent)] hover:underline"
      onclick={loadFrpSnippet}
    >
      生成 FRP 片段
    </button>
    {#if frpSnippet}
      <div class="mt-2 flex items-start justify-between gap-2">
        <pre
          class="max-h-32 min-w-0 flex-1 overflow-auto rounded border border-[var(--color-border)] p-2 font-mono text-[10px] text-[var(--color-text-secondary)]"
        >{frpSnippet}</pre>
        <CopyButton value={frpSnippet} label="复制" />
      </div>
    {/if}
  {/if}
</div>
