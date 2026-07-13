<script lang="ts">
  import CopyButton from "$lib/components/CopyButton.svelte";

  interface Props {
    label: string;
    value: string;
    hint?: string;
    loading?: boolean;
  }

  let { label, value, hint = "", loading = false }: Props = $props();

  const display = $derived(loading ? "加载中…" : value || "未配置");
  const canCopy = $derived(!loading && value.length > 0);
</script>

<div class="tx-info-block">
  <div class="tx-info-row">
    <span class="tx-info-label">{label}</span>
    {#if canCopy}
      <CopyButton {value} />
    {/if}
  </div>
  <p class="tx-mono mt-1.5 truncate text-sm text-[var(--color-text-secondary)]">{display}</p>
  {#if hint}
    <p class="mt-1 text-[11px] text-[var(--color-text-muted)]">{hint}</p>
  {/if}
</div>
