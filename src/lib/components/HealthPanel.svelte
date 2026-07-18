<script lang="ts">
  import { onMount } from "svelte";
  import { runHealthChecks, type HealthItem } from "$lib/api/health";

  interface Props {
    workspaceId: string;
    onRunCheck?: (workspaceId: string) => Promise<HealthItem[]>;
    onFailure?: (items: HealthItem[]) => void | Promise<void>;
    autoRefresh?: boolean;
    refreshIntervalMs?: number;
  }

  let {
    workspaceId,
    onRunCheck,
    onFailure,
    autoRefresh = true,
    refreshIntervalMs = 15_000,
  }: Props = $props();

  let items = $state<HealthItem[]>([]);
  let busy = $state(false);
  let error = $state("");
  let lastCheckedAt = $state<Date | null>(null);
  let failureSignature = "";
  const failedCount = $derived(items.filter((item) => !item.ok).length);

  function applyResults(nextItems: HealthItem[]) {
    items = nextItems;
    lastCheckedAt = new Date();
    const failedItems = nextItems.filter((item) => !item.ok);
    const nextSignature = failedItems.map((item) => `${item.label}:${item.detail}`).join("|");
    if (failedItems.length > 0 && nextSignature !== failureSignature) {
      failureSignature = nextSignature;
      void onFailure?.(failedItems);
    } else if (failedItems.length === 0) {
      failureSignature = "";
    }
  }

  async function runCheck() {
    if (busy || !workspaceId) return;
    busy = true;
    error = "";
    try {
      const nextItems = onRunCheck ? await onRunCheck(workspaceId) : await runHealthChecks(workspaceId);
      applyResults(nextItems);
    } catch (err) {
      error = String(err);
      items = [];
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    if (!autoRefresh) return;
    void runCheck();
    const timer = window.setInterval(() => void runCheck(), refreshIntervalMs);
    return () => window.clearInterval(timer);
  });
</script>

<section class="tx-card p-5">
  <div class="flex items-start justify-between gap-3">
    <div>
      <h3 class="font-semibold">健康检查</h3>
      <p class="mt-1 text-sm text-[var(--color-text-muted)]">
        MCP、Actions 本地/公网 endpoint 与 OAuth 元数据
        {#if lastCheckedAt} · 最近 {lastCheckedAt.toLocaleTimeString()}{/if}
      </p>
    </div>
    <button
      type="button"
      class="tx-btn-ghost shrink-0 disabled:opacity-50"
      disabled={busy}
      onclick={runCheck}
    >
      {busy ? "检查中…" : "运行健康检查"}
    </button>
  </div>

  {#if error}
    <p class="mt-4 rounded-lg border border-[var(--color-error)]/30 bg-[var(--color-error)]/10 px-3 py-2 text-sm text-[var(--color-error)]">
      {error}
    </p>
  {/if}

  {#if failedCount > 0}
    <div class="tx-alert tx-alert--error mt-4" role="status">
      发现 {failedCount} 项连接检查失败。请先查看失败项；公网失败通常表示隧道或连接器仍未恢复。
    </div>
  {/if}

  {#if items.length > 0}
    <ul class="mt-4 grid gap-2">
      {#each items as item (item.label)}
        <li
          class="flex items-start justify-between gap-3 rounded-lg bg-[var(--color-bg)] px-3 py-2"
        >
          <div class="min-w-0">
            <p class="text-sm font-medium">{item.label}</p>
            <p class="mt-0.5 text-xs text-[var(--color-text-muted)]">{item.detail}</p>
            {#if !item.ok && item.hint}
              <p class="mt-1 text-xs text-[var(--color-accent)]">{item.hint}</p>
            {/if}
          </div>
          <span
            class="shrink-0 rounded-sm px-2 py-0.5 text-xs font-medium"
            class:health-ok={item.ok}
            class:health-fail={!item.ok}
          >
            {item.ok ? "通过" : "失败"}
          </span>
        </li>
      {/each}
    </ul>
  {:else if !busy && !error}
    <p class="mt-4 text-sm text-[var(--color-text-muted)]">尚未运行检查。</p>
  {/if}
</section>

<style>
  .health-ok {
    background: color-mix(in oklch, var(--color-success) 15%, transparent);
    color: var(--color-success);
  }

  .health-fail {
    background: color-mix(in oklch, var(--color-error) 15%, transparent);
    color: var(--color-error);
  }
</style>
