<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import { open } from "@tauri-apps/plugin-dialog";
  import AppShell from "$lib/components/AppShell.svelte";
  import ToastHost from "$lib/components/ToastHost.svelte";
  import WorkspaceNavItem from "$lib/components/WorkspaceNavItem.svelte";
  import {
    createWorkspace,
    getActionsRuntimeStatus,
    getRuntimeStatus,
    listWorkspaces,
  } from "$lib/api/workspaces";
  import { getLastWorkspaceId } from "$lib/api/settings";
  import { actionsRuntimeStates, mcpRuntimeStates, workspaces } from "$lib/stores/app";
  import type { RuntimeState } from "$lib/types";

  let { children } = $props();
  let refreshInFlight = false;

  async function refreshWorkspaces(): Promise<void> {
    if (refreshInFlight) return;
    refreshInFlight = true;
    try {
      const items = await listWorkspaces();
      workspaces.set(items);

      const mcpStates: Record<string, RuntimeState> = {};
      const actionsStates: Record<string, RuntimeState> = {};
      await Promise.all(
        items.map(async (item) => {
          try {
            const [mcp, actions] = await Promise.all([
              getRuntimeStatus(item.id),
              getActionsRuntimeStatus(item.id),
            ]);
            mcpStates[item.id] = mcp.state;
            actionsStates[item.id] = actions.state;
          } catch {
            mcpStates[item.id] = "stopped";
            actionsStates[item.id] = "stopped";
          }
        }),
      );
      mcpRuntimeStates.set(mcpStates);
      actionsRuntimeStates.set(actionsStates);
    } catch {
      // A transient IPC failure must not erase the last known sidebar state.
    } finally {
      refreshInFlight = false;
    }
  }

  async function addWorkspace() {
    const selected = await open({ directory: true, multiple: false });
    if (!selected || Array.isArray(selected)) return;
    const profile = await createWorkspace(selected);
    await refreshWorkspaces();
    goto(`/workspace/${profile.id}`);
  }

  function openWorkspace(id: string) {
    goto(`/workspace/${id}`);
  }

  function openFrpSettings() {
    goto("/settings/frp");
  }

  function openSoftwareSettings() {
    goto("/settings/software");
  }

  function openGeneralSettings() {
    goto("/settings/general");
  }

  function openKeysSettings() {
    goto("/settings/keys");
  }

  onMount(() => {
    let disposed = false;

    const initialLoad = async () => {
      await refreshWorkspaces();
      if (disposed) return;
      const path = $page.url.pathname;
      if (path === "/") {
        const lastId = await getLastWorkspaceId();
        if (disposed) return;
        if (lastId && $workspaces.some((item) => item.id === lastId)) {
          goto(`/workspace/${lastId}`);
        } else if ($workspaces.length > 0) {
          goto(`/workspace/${$workspaces[0].id}`);
        }
      }
    };

    void initialLoad();
    const timer = window.setInterval(() => {
      if (!disposed) void refreshWorkspaces();
    }, 10_000);

    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  });
</script>

<AppShell onAddWorkspace={addWorkspace}>
  {#snippet settingsNav()}
    <button
      type="button"
      class="tx-settings-link {$page.url.pathname === '/settings/general' ? 'active' : ''}"
      onclick={openGeneralSettings}
    >
      通用
    </button>
    <button
      type="button"
      class="tx-settings-link {$page.url.pathname === '/settings/keys' ? 'active' : ''}"
      onclick={openKeysSettings}
    >
      共享密钥
    </button>
    <button
      type="button"
      class="tx-settings-link {$page.url.pathname === '/settings/frp' ? 'active' : ''}"
      onclick={openFrpSettings}
    >
      FRP 配置
    </button>
    <button
      type="button"
      class="tx-settings-link {$page.url.pathname === '/settings/software' ? 'active' : ''}"
      onclick={openSoftwareSettings}
    >
      软件管理
    </button>
  {/snippet}
  {#snippet sidebar()}
    <div class="space-y-1">
      {#each $workspaces as workspace (workspace.id)}
        <WorkspaceNavItem
          workspace={workspace}
          active={$page.url.pathname === `/workspace/${workspace.id}`}
          mcpState={$mcpRuntimeStates[workspace.id] ?? "stopped"}
          actionsState={$actionsRuntimeStates[workspace.id] ?? "stopped"}
          onClick={() => openWorkspace(workspace.id)}
        />
      {/each}
    </div>
  {/snippet}

  {#snippet children()}
    {@render children()}
  {/snippet}
</AppShell>

<ToastHost />
