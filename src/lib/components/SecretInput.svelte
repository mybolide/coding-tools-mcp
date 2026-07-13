<script lang="ts">
  import CopyButton from "$lib/components/CopyButton.svelte";

  interface Props {
    value?: string;
    placeholder?: string;
    readonly?: boolean;
    disabled?: boolean;
    showCopy?: boolean;
    onRegenerate?: (() => void) | undefined;
    regenerating?: boolean;
    monospace?: boolean;
    size?: "sm" | "md";
  }

  let {
    value = $bindable(""),
    placeholder = "",
    readonly = false,
    disabled = false,
    showCopy = true,
    onRegenerate,
    regenerating = false,
    monospace = true,
    size = "md",
  }: Props = $props();

  let visible = $state(true);

  const isLoadingPlaceholder = $derived(value === "加载中…");
  const canReveal = $derived(!disabled && !isLoadingPlaceholder && value.length > 0);
  const inputType = $derived(visible ? "text" : "password");
  const fontClass = $derived(monospace ? "font-mono" : "");
  const textClass = $derived(size === "sm" ? "text-xs" : "text-sm");
</script>

<div class="flex gap-2">
  <div class="tx-secret-input min-w-0 flex-1">
    {#if readonly}
      <input
        type={inputType}
        class="tx-secret-input-field {fontClass} {textClass}"
        {value}
        {placeholder}
        readonly
        {disabled}
        autocomplete="off"
      />
    {:else}
      <input
        type={inputType}
        class="tx-secret-input-field {fontClass} {textClass}"
        bind:value
        {placeholder}
        {disabled}
        autocomplete="off"
      />
    {/if}
    {#if canReveal}
      <button
        type="button"
        class="tx-secret-toggle"
        title={visible ? "隐藏明文" : "显示明文"}
        onclick={() => {
          visible = !visible;
        }}
      >
        {visible ? "隐藏" : "显示"}
      </button>
    {/if}
  </div>
  {#if showCopy && value && !isLoadingPlaceholder}
    <CopyButton {value} />
  {/if}
  {#if onRegenerate}
    <button
      type="button"
      class="shrink-0 rounded-md border border-[var(--color-border)] px-2.5 py-1 text-xs text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-surface-hover)] disabled:opacity-50"
      disabled={regenerating || disabled}
      onclick={() => onRegenerate?.()}
    >
      {regenerating ? "生成中…" : "重新生成"}
    </button>
  {/if}
</div>
