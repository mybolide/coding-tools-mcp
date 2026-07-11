<script lang="ts">
  import StatusOrb from "$lib/components/StatusOrb.svelte";
  import type { RuntimeState } from "$lib/types";

  interface Props {
    mcp: RuntimeState;
    actions: RuntimeState;
  }

  let { mcp, actions }: Props = $props();
</script>

<span class="inline-flex items-center gap-1.5" title="MCP / Actions">
  <StatusOrb state={mcp} />
  <StatusOrb state={actions} />
</span>
