<script lang="ts">
  import { FolderPlus } from "@lucide/svelte";

  interface Props {
    onAdd?: () => void;
  }

  let { onAdd }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center px-8 py-8 text-center">
  <div
    class="tx-card mb-6 flex h-16 w-16 items-center justify-center rounded-2xl border border-dashed"
  >
    <FolderPlus size={28} class="text-[var(--color-text-muted)]" />
  </div>
  <h2 class="text-2xl font-bold tracking-tight">添加你的第一个工作区</h2>
  <p class="mt-2 max-w-md text-sm text-[var(--color-text-muted)]">
    选择项目目录后，可分别启动 MCP 与 Actions 本地服务。
  </p>
  {#if onAdd}
    <button type="button" class="tx-btn-primary mt-6" onclick={onAdd}>
      选择目录
    </button>
  {/if}
</div>
