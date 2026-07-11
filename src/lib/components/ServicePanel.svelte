<script lang="ts">
  import CopyButton from "$lib/components/CopyButton.svelte";
  import StatusOrb from "$lib/components/StatusOrb.svelte";
  import type { RuntimeState } from "$lib/types";

  interface Props {
    title: string;
    subtitle: string;
    status: RuntimeState;
    port: number;
    portEditable?: boolean;
    busy?: boolean;
    localEndpoint: string;
    publicEndpoint?: string;
    publicLabel?: string;
    onToggle: () => void | Promise<void>;
    onPortChange?: (port: number) => void | Promise<void>;
  }

  let {
    title,
    subtitle,
    status,
    port,
    portEditable = false,
    busy = false,
    localEndpoint,
    publicEndpoint = "",
    publicLabel = "公网",
    onToggle,
    onPortChange,
  }: Props = $props();

  let draftPort = $state(0);

  $effect(() => {
    draftPort = port;
  });

  const running = $derived(status === "running");
  const canEditPort = $derived(portEditable && !running && status !== "starting");

  async function commitPort() {
    if (!onPortChange || draftPort === port) return;
    if (draftPort < 1024 || draftPort > 65535) {
      draftPort = port;
      return;
    }
    await onPortChange(draftPort);
  }
</script>

<article class="tx-card p-5">
  <div class="flex items-start justify-between gap-3">
    <div class="min-w-0">
      <div class="flex items-center gap-2">
        <StatusOrb state={status} />
        <h3 class="text-[15px] font-semibold tracking-tight">{title}</h3>
      </div>
      <p class="mt-1 text-sm text-[var(--color-text-muted)]">{subtitle}</p>
    </div>
    <button
      type="button"
      class="tx-btn-primary shrink-0"
      class:tx-btn-danger={running}
      disabled={busy || status === "starting" || status === "stopping"}
      onclick={onToggle}
    >
      {#if busy}
        处理中…
      {:else if running}
        停止
      {:else}
        启动
      {/if}
    </button>
  </div>

  <div class="mt-5 grid gap-3">
    <div class="flex items-center justify-between gap-3 rounded-[10px] bg-[var(--surface-hover)] px-3 py-2.5">
      <span class="text-xs font-medium text-[var(--color-text-muted)]">端口</span>
      {#if canEditPort}
        <input
          type="number"
          min="1024"
          max="65535"
          class="tx-input w-24 text-right"
          bind:value={draftPort}
          onchange={commitPort}
        />
      {:else}
        <span class="tx-mono text-sm">{port}</span>
      {/if}
    </div>

    <div class="rounded-[10px] bg-[var(--surface-hover)] px-3 py-2.5">
      <div class="flex items-center justify-between gap-2">
        <p class="text-xs font-medium text-[var(--color-text-muted)]">本地地址</p>
        <CopyButton value={localEndpoint} />
      </div>
      <p class="tx-mono mt-1 truncate text-sm">{localEndpoint}</p>
    </div>

    {#if publicEndpoint || publicLabel}
      <div class="rounded-[10px] bg-[var(--surface-hover)] px-3 py-2.5">
        <div class="flex items-center justify-between gap-2">
          <p class="text-xs font-medium text-[var(--color-text-muted)]">{publicLabel}</p>
          {#if publicEndpoint}
            <CopyButton value={publicEndpoint} />
          {/if}
        </div>
        <p class="tx-mono mt-1 truncate text-sm text-[var(--color-text-secondary)]">
          {publicEndpoint || "未配置隧道"}
        </p>
      </div>
    {/if}
  </div>
</article>
