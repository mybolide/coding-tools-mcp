<script lang="ts">
  import { onMount } from "svelte";
  import { readWorkspaceLogs, type LogChunk, type LogService } from "$lib/api/logs";

  interface Props {
    workspaceId: string;
    service: LogService;
    autoRefresh?: boolean;
    title?: string;
  }

  let { workspaceId, service, autoRefresh = true, title }: Props = $props();

  let chunks = $state<LogChunk[]>([]);
  let busy = $state(false);
  let error = $state("");

  const heading = $derived(title ?? (service === "mcp" ? "MCP 日志" : "Actions 日志"));

  async function refresh() {
    if (busy || !workspaceId) return;
    busy = true;
    error = "";
    try {
      chunks = await readWorkspaceLogs(workspaceId, service);
    } catch (err) {
      error = String(err);
      chunks = [];
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    if (autoRefresh) {
      void refresh();
    }
  });
</script>

<section class="tx-card p-5">
  <div class="flex items-start justify-between gap-3">
    <div>
      <h3 class="font-semibold">{heading}</h3>
      <p class="mt-1 text-sm text-[var(--color-text-muted)]">最近 8KB 尾部输出</p>
    </div>
    <button
      type="button"
      class="tx-btn-ghost shrink-0 disabled:opacity-50"
      disabled={busy}
      onclick={refresh}
    >
      {busy ? "刷新中…" : "刷新"}
    </button>
  </div>

  {#if error}
    <p
      class="mt-4 rounded-lg border border-[var(--color-error)]/30 bg-[var(--color-error)]/10 px-3 py-2 text-sm text-[var(--color-error)]"
    >
      {error}
    </p>
  {/if}

  {#if chunks.length > 0}
    <div class="mt-4 grid gap-3">
      {#each chunks as chunk (chunk.name)}
        <div class="overflow-hidden rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)]">
          <p class="border-b border-[var(--color-border)] px-3 py-1.5 font-mono text-xs text-[var(--color-text-muted)]">
            {chunk.name}
          </p>
          <pre
            class="max-h-48 overflow-auto whitespace-pre-wrap break-words p-3 font-mono text-xs leading-relaxed"
          >{chunk.content || "（空）"}</pre>
        </div>
      {/each}
    </div>
  {:else if !busy && !error}
    <p class="mt-4 text-sm text-[var(--color-text-muted)]">当前还没有日志</p>
  {/if}
</section>
