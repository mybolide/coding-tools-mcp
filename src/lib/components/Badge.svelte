<script lang="ts">
  interface Props {
    label: string;
  }

  let { label }: Props = $props();
</script>

<span
  class="inline-flex items-center rounded-sm border border-[var(--color-border)] px-2 py-0.5 text-xs text-[var(--color-text-secondary)]"
>
  {label}
</span>
