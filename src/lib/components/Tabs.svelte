<script lang="ts">
  interface TabItem {
    value: string;
    label: string;
  }

  interface Props {
    items: TabItem[];
    value: string;
    onchange: (value: string) => void;
  }

  let { items, value, onchange }: Props = $props();
</script>

<div class="tx-tabs" role="tablist">
  {#each items as item (item.value)}
    <button
      type="button"
      role="tab"
      aria-selected={value === item.value}
      class="tx-tab"
      class:active={value === item.value}
      onclick={() => onchange(item.value)}
    >
      {item.label}
    </button>
  {/each}
</div>
