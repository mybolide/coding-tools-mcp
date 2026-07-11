<script lang="ts">
  import type { RuntimeState } from "$lib/types";

  interface Props {
    state: RuntimeState;
  }

  let { state }: Props = $props();

  const colorClass: Record<RuntimeState, string> = {
    running: "bg-[var(--color-success)]",
    starting: "bg-[var(--color-warning)]",
    stopping: "bg-[var(--color-warning)]",
    stopped: "bg-[var(--color-text-muted)]",
    error: "bg-[var(--color-error)]",
  };
</script>

<span
  class="inline-block h-2.5 w-2.5 rounded-full {colorClass[state]}"
  class:animate-pulse={state === "running"}
  aria-label={state}
></span>
