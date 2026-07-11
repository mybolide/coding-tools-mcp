<script lang="ts">
  import CopyButton from "$lib/components/CopyButton.svelte";
  import ServiceStatusPair from "$lib/components/ServiceStatusPair.svelte";
  import {
    actionsConfig,
    actionsLocalEndpoint,
    actionsOpenApiUrl,
    mcpLocalEndpoint,
    type RuntimeState,
    type WorkspaceProfile,
  } from "$lib/types";

  interface Props {
    workspace: WorkspaceProfile;
    active: boolean;
    mcpState: RuntimeState;
    actionsState: RuntimeState;
    onClick: () => void;
  }

  let { workspace, active, mcpState, actionsState, onClick }: Props = $props();

  const mcpPort = $derived(workspace.runtime.local_port);
  const actionsPort = $derived(actionsConfig(workspace).local_port);
  const mcpEndpoint = $derived(mcpLocalEndpoint(mcpPort));
  const openApiUrl = $derived(
    actionsOpenApiUrl(workspace) || `${actionsLocalEndpoint(actionsPort)}/openapi.json`,
  );
</script>

<div class="tx-nav-item" class:active>
  <button type="button" class="tx-nav-button" onclick={onClick}>
    <ServiceStatusPair mcp={mcpState} actions={actionsState} />
    <div class="min-w-0 flex-1">
      <span class="block truncate text-sm font-medium">{workspace.name}</span>
      <span class="mt-0.5 block text-xs opacity-70">
        MCP {mcpPort} · Actions {actionsPort}
      </span>
    </div>
  </button>
  <div
    class="flex flex-wrap items-center gap-1.5 px-3 pb-2.5"
    role="presentation"
    onclick={(event) => event.stopPropagation()}
  >
    <CopyButton value={mcpEndpoint} label="MCP" />
    <CopyButton value={openApiUrl} label="OpenAPI" />
  </div>
</div>
