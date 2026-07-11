<script lang="ts">
  import ThemeToggle from "$lib/components/ThemeToggle.svelte";
  import type { Snippet } from "svelte";

  interface Props {
    children: Snippet;
    sidebar: Snippet;
    onAddWorkspace?: () => void | Promise<void>;
    settingsNav?: Snippet;
  }

  let { children, sidebar, onAddWorkspace, settingsNav }: Props = $props();
</script>

<div class="app-layout">
  <aside class="tx-sidebar">
    <div class="tx-sidebar-header">
      <div class="flex items-start justify-between gap-2">
        <div>
          <p class="tx-brand-kicker">Coding Tools</p>
          <h1 class="tx-brand-title">桌面控制台</h1>
        </div>
        <ThemeToggle />
      </div>
      {#if onAddWorkspace}
        <button type="button" class="tx-btn-primary tx-btn-sidebar" onclick={onAddWorkspace}>
          添加工作区
        </button>
      {/if}
    </div>

    <div class="tx-sidebar-body">
      {#if settingsNav}
        <p class="tx-sidebar-section-label">设置</p>
        <div class="mb-4">
          {@render settingsNav()}
        </div>
      {/if}
      {#if onAddWorkspace}
        <p class="tx-sidebar-section-label">工作区</p>
      {/if}
      {@render sidebar()}
    </div>
  </aside>

  <main class="tx-main">
    {@render children()}
  </main>
</div>

<svelte:head>
  <title>Coding Tools MCP</title>
</svelte:head>
