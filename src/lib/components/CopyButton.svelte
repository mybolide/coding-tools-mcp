<script lang="ts">
  interface Props {
    value: string;
    label?: string;
    onCopy?: () => void;
  }

  let { value, label = "复制", onCopy }: Props = $props();
  let copied = $state(false);

  async function copy() {
    await navigator.clipboard.writeText(value);
    copied = true;
    onCopy?.();
    setTimeout(() => {
      copied = false;
    }, 1500);
  }
</script>

<button
  type="button"
  class="tx-btn-ghost shrink-0 px-2.5 py-1 text-xs"
  onclick={copy}
>
  {copied ? "已复制" : label}
</button>
