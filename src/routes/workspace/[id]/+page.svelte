<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import ActionsAuthForm from "$lib/components/ActionsAuthForm.svelte";
  import ActionsPolicyForm, {
    type ActionsPolicyDraft,
  } from "$lib/components/ActionsPolicyForm.svelte";
  import AuthConfigForm from "$lib/components/AuthConfigForm.svelte";
  import HealthPanel from "$lib/components/HealthPanel.svelte";
  import LogViewer from "$lib/components/LogViewer.svelte";
  import RuntimePolicyForm, {
    type RuntimePolicyDraft,
  } from "$lib/components/RuntimePolicyForm.svelte";
  import ServicePanel from "$lib/components/ServicePanel.svelte";
  import StatusOrb from "$lib/components/StatusOrb.svelte";
  import Tabs from "$lib/components/Tabs.svelte";
  import TunnelConfigForm, {
    type TunnelFormConfig,
  } from "$lib/components/TunnelConfigForm.svelte";
  import TunnelStrip from "$lib/components/TunnelStrip.svelte";
  import WorkspaceMetaForm from "$lib/components/WorkspaceMetaForm.svelte";
  import {
    deleteWorkspace,
    getActionsRuntimeStatus,
    getRuntimeStatus,
    listWorkspaces,
    startActionsRuntime,
    startRuntime,
    restartRuntime,
    restartActionsRuntime,
    stopActionsRuntime,
    stopRuntime,
    updateWorkspace,
  } from "$lib/api/workspaces";
  import { listFrpProfiles, setLastWorkspace, type FrpProfileDto } from "$lib/api/settings";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { restartTunnel } from "$lib/api/tunnel";
  import { runServiceToggle } from "$lib/runtime/service";
  import { promptServiceRestart } from "$lib/runtime/restart-hint";
  import { actionsRuntimeStates, mcpRuntimeStates, workspaces } from "$lib/stores/app";
  import {
    actionsConfig,
    actionsLocalEndpoint,
    actionsOAuthAuthorizeUrl,
    actionsOAuthTokenUrl,
    actionsOpenApiUrl,
    actionsPrivacyUrl,
    frpPublicUrl,
    mcpLocalEndpoint,
    type AuthConfig,
    type ActionsAuthDraft,
    type RuntimeState,
    type WorkspaceProfile,
  } from "$lib/types";

  type ServiceTab = "mcp" | "actions";
  type SubTab = "config" | "logs" | "health";

  let profile = $state<WorkspaceProfile | null>(null);
  let mcpStatus = $state<RuntimeState>("stopped");
  let actionsStatus = $state<RuntimeState>("stopped");
  let mcpBusy = $state(false);
  let actionsBusy = $state(false);
  let mcpLocal = $state("");
  let mcpPublic = $state("");
  let actionsLocal = $state("");
  let actionsPublic = $state("");
  let frpProfiles = $state<FrpProfileDto[]>([]);

  let activeService = $state<ServiceTab>("mcp");
  let mcpSubTab = $state<SubTab>("config");
  let actionsSubTab = $state<SubTab>("config");

  const subTabs = [
    { value: "config", label: "配置" },
    { value: "logs", label: "日志" },
    { value: "health", label: "健康" },
  ];

  const workspaceId = $derived($page.params.id);
  const actions = $derived(profile ? actionsConfig(profile) : null);

  const mcpTunnelForm = $derived<TunnelFormConfig>({
    type: profile?.tunnel.type ?? "none",
    public_url: profile?.tunnel.public_url ?? "",
    frp_server: profile?.tunnel.frp_server ?? "",
    frp_subdomain: profile?.tunnel.frp_subdomain ?? "",
    frp_profile_id: profile?.tunnel.frp_profile_id ?? "",
    frp_server_port: profile?.tunnel.frp_server_port ?? 7000,
    cloudflare_mode: profile?.tunnel.cloudflare_mode ?? "quick",
  });

  const actionsTunnelForm = $derived<TunnelFormConfig>({
    type: actions?.tunnel_type ?? "none",
    public_url: actions?.public_url ?? "",
    frp_server: actions?.frp_server ?? "",
    frp_subdomain: actions?.frp_subdomain ?? "",
    frp_profile_id: actions?.frp_profile_id ?? "",
    frp_server_port: actions?.frp_server_port ?? 7000,
    cloudflare_mode: actions?.cloudflare_mode ?? "quick",
  });

  function stateLabel(state: RuntimeState): string {
    switch (state) {
      case "running":
        return "运行中";
      case "starting":
        return "启动中";
      case "stopping":
        return "停止中";
      case "error":
        return "错误";
      default:
        return "已停止";
    }
  }

  function applyMcpRuntime(runtime: { state: RuntimeState; localEndpoint: string; publicEndpoint: string }) {
    mcpStatus = runtime.state;
    mcpLocal = runtime.localEndpoint;
    mcpPublic = runtime.publicEndpoint;
    if (workspaceId) {
      mcpRuntimeStates.update((current) => ({ ...current, [workspaceId]: runtime.state }));
    }
  }

  function applyActionsRuntime(runtime: {
    state: RuntimeState;
    localEndpoint: string;
    publicEndpoint: string;
  }) {
    actionsStatus = runtime.state;
    actionsLocal = runtime.localEndpoint;
    actionsPublic = runtime.publicEndpoint;
    if (workspaceId) {
      actionsRuntimeStates.update((current) => ({ ...current, [workspaceId]: runtime.state }));
    }
  }

  async function load() {
    if (!workspaceId) return;
    const items = await listWorkspaces();
    workspaces.set(items);
    frpProfiles = await listFrpProfiles();
    profile = items.find((item) => item.id === workspaceId) ?? null;
    if (profile) {
      await setLastWorkspace(profile.id);
    }
    if (!profile) {
      goto("/");
      return;
    }

    const [mcpRuntime, actionsRuntime] = await Promise.all([
      getRuntimeStatus(workspaceId),
      getActionsRuntimeStatus(workspaceId),
    ]);
    applyMcpRuntime(mcpRuntime);
    applyActionsRuntime(actionsRuntime);
  }

  async function toggleMcp() {
    if (!workspaceId || mcpBusy) return;
    mcpBusy = true;
    try {
      const runtime = await runServiceToggle(
        mcpStatus === "running",
        () => startRuntime(workspaceId),
        () => stopRuntime(workspaceId),
      );
      if (runtime) applyMcpRuntime(runtime);
    } finally {
      mcpBusy = false;
    }
  }

  async function toggleActions() {
    if (!workspaceId || actionsBusy) return;
    actionsBusy = true;
    try {
      const runtime = await runServiceToggle(
        actionsStatus === "running",
        () => startActionsRuntime(workspaceId),
        () => stopActionsRuntime(workspaceId),
      );
      if (runtime) applyActionsRuntime(runtime);
    } finally {
      actionsBusy = false;
    }
  }

  async function saveMcpPort(port: number) {
    if (!profile || profile.runtime.local_port === port) return;
    const next: WorkspaceProfile = {
      ...profile,
      runtime: { ...profile.runtime, local_port: port },
    };
    await updateWorkspace(next);
    profile = next;
    mcpLocal = mcpLocalEndpoint(port);
    await load();
  }

  async function saveActionsPort(port: number) {
    if (!profile) return;
    const current = actionsConfig(profile);
    if (current.local_port === port) return;
    const next: WorkspaceProfile = {
      ...profile,
      actions: { ...current, local_port: port },
    };
    await updateWorkspace(next);
    profile = next;
    actionsLocal = actionsLocalEndpoint(port);
    await load();
  }

  function publicEndpointFromTunnel(config: TunnelFormConfig, suffix: string): string {
    const base = frpPublicUrl(
      config.type,
      config.frp_subdomain,
      config.frp_server,
      config.frp_profile_id,
      frpProfiles,
      config.public_url,
    );
    if (base) {
      return `${base.replace(/\/$/, "")}${suffix}`;
    }
    return "";
  }

  async function restartTunnelIfFrp(config: TunnelFormConfig, service: "mcp" | "actions") {
    if (!workspaceId || config.type !== "frp") return;
    try {
      const status = await restartTunnel(workspaceId, service);
      if (status.publicUrl) {
        if (service === "mcp") {
          mcpPublic = `${status.publicUrl.replace(/\/$/, "")}/mcp`;
        } else {
          actionsPublic = `${status.publicUrl.replace(/\/$/, "")}/openapi.json`;
        }
      }
    } catch {
      // Tunnel may not be running; ignore restart errors.
    }
  }

  async function saveMcpTunnel(config: TunnelFormConfig) {
    if (!profile) return;
    const next: WorkspaceProfile = {
      ...profile,
      tunnel: {
        ...profile.tunnel,
        type: config.type,
        public_url: config.public_url,
        frp_server: config.frp_server,
        frp_subdomain: config.frp_subdomain,
        frp_profile_id: config.frp_profile_id,
        frp_server_port: config.frp_server_port,
        cloudflare_mode: config.cloudflare_mode,
      },
    };
    await updateWorkspace(next);
    profile = next;
    mcpPublic = publicEndpointFromTunnel(config, "/mcp");
    await load();
    await restartTunnelIfFrp(config, "mcp");
    await promptServiceRestart(mcpStatus === "running", "MCP 服务");
  }

  async function saveActionsTunnel(config: TunnelFormConfig) {
    if (!profile) return;
    const current = actionsConfig(profile);
    const next: WorkspaceProfile = {
      ...profile,
      actions: {
        ...current,
        tunnel_type: config.type,
        public_url: config.public_url,
        frp_server: config.frp_server,
        frp_subdomain: config.frp_subdomain,
        frp_profile_id: config.frp_profile_id,
        frp_server_port: config.frp_server_port,
        cloudflare_mode: config.cloudflare_mode,
      },
    };
    await updateWorkspace(next);
    profile = next;
    actionsPublic = publicEndpointFromTunnel(config, "/openapi.json");
    await load();
    await restartTunnelIfFrp(config, "actions");
    await promptServiceRestart(actionsStatus === "running", "Actions 服务");
  }

  async function saveMcpPolicy(draft: RuntimePolicyDraft) {
    if (!profile) return;
    const next: WorkspaceProfile = {
      ...profile,
      runtime: {
        ...profile.runtime,
        tool_profile: draft.toolProfile,
        permission_mode: draft.permissionMode,
      },
    };
    await updateWorkspace(next);
    profile = next;
    await load();
    await promptServiceRestart(mcpStatus === "running", "MCP 服务");
  }

  async function saveActionsPolicy(draft: ActionsPolicyDraft) {
    if (!profile) return;
    const current = actionsConfig(profile);
    const next: WorkspaceProfile = {
      ...profile,
      actions: {
        ...current,
        allowed_commands: draft.allowedCommands,
        max_patch_bytes: draft.maxPatchBytes,
        permission_mode: draft.permissionMode,
      },
    };
    await updateWorkspace(next);
    profile = next;
    await load();
    await promptServiceRestart(actionsStatus === "running", "Actions 服务");
  }

  async function saveMcpAuth(auth: AuthConfig) {
    if (!profile || !workspaceId) return;
    const next: WorkspaceProfile = { ...profile, auth };
    await updateWorkspace(next);
    profile = next;
    await load();
    if (mcpStatus === "running") {
      try { await restartRuntime(workspaceId); } catch { /* ignore */ }
    }
  }

  async function saveActionsAuth(draft: ActionsAuthDraft) {
    if (!profile || !workspaceId) return;
    const current = actionsConfig(profile);
    const next: WorkspaceProfile = {
      ...profile,
      actions: {
        ...current,
        auth_type: draft.authType,
        oauth_client_id: draft.oauthClientId || current.oauth_client_id,
        oauth_scopes: draft.oauthScopes,
        use_shared_secrets: draft.useSharedSecrets,
      },
    };
    await updateWorkspace(next);
    profile = next;
    await load();
    if (actionsStatus === "running") {
      try { await restartActionsRuntime(workspaceId); } catch { /* ignore */ }
    }
  }

  async function saveWorkspaceName(name: string) {
    if (!profile || profile.name === name) return;
    const next: WorkspaceProfile = { ...profile, name };
    await updateWorkspace(next);
    profile = next;
    workspaces.update((items) =>
      items.map((item) => (item.id === next.id ? { ...item, name: next.name } : item)),
    );
  }

  async function removeWorkspace() {
    if (!profile || !workspaceId) return;
    const confirmed = await confirm(`确定删除工作区「${profile.name}」？此操作不可撤销。`, {
      title: "删除工作区",
      kind: "warning",
      okLabel: "删除",
      cancelLabel: "取消",
    });
    if (!confirmed) return;
    await deleteWorkspace(workspaceId);
    workspaces.update((items) => items.filter((item) => item.id !== workspaceId));
    goto("/");
  }

  onMount(load);
</script>

{#if profile && actions}
  <section class="page-scroll">
    <header class="page-header">
      <div class="flex items-start justify-between gap-4">
        <div>
          <p class="page-kicker">工作区</p>
          <h2 class="page-title">{profile.name}</h2>
        </div>
        <button
          type="button"
          class="tx-btn-ghost text-[var(--danger)]"
          onclick={() => void removeWorkspace()}
        >
          删除工作区
        </button>
      </div>

      <div class="mt-4 flex flex-wrap items-center gap-2">
        <button
          type="button"
          class="tx-status-pill"
          class:active={activeService === "mcp"}
          onclick={() => (activeService = "mcp")}
        >
          <StatusOrb state={mcpStatus} />
          <span class="font-medium">MCP</span>
          <span class="text-[var(--color-text-muted)]">{stateLabel(mcpStatus)}</span>
        </button>
        <button
          type="button"
          class="tx-status-pill"
          class:active={activeService === "actions"}
          onclick={() => (activeService = "actions")}
        >
          <StatusOrb state={actionsStatus} />
          <span class="font-medium">Actions</span>
          <span class="text-[var(--color-text-muted)]">{stateLabel(actionsStatus)}</span>
        </button>
      </div>
    </header>

    <div class="page-body">
      {#if activeService === "mcp"}
        <div class="mt-4 flex flex-col gap-3">
          <ServicePanel
            title="MCP"
            subtitle="Streamable HTTP · 工具运行时"
            status={mcpStatus}
            port={profile.runtime.local_port}
            portEditable={true}
            busy={mcpBusy}
            localEndpoint={mcpLocal || mcpLocalEndpoint(profile.runtime.local_port)}
            publicEndpoint={mcpPublic}
            publicLabel="公网 MCP"
            onToggle={toggleMcp}
            onPortChange={saveMcpPort}
          />
          <TunnelStrip
            workspaceId={workspaceId!}
            service="mcp"
            tunnelType={profile.tunnel.type}
            publicUrl={profile.tunnel.public_url}
            onPublicUrlChange={(url) => {
              profile = { ...profile!, tunnel: { ...profile!.tunnel, public_url: url } };
              mcpPublic = url ? `${url.replace(/\/$/, "")}/mcp` : "";
            }}
          />
        </div>

        <div class="mt-5">
          <Tabs
            items={subTabs}
            value={mcpSubTab}
            onchange={(v) => {
              mcpSubTab = v as SubTab;
            }}
          />
        </div>

        {#if mcpSubTab === "config"}
          <div class="tx-card mt-4 grid gap-6 p-5">
            <div>
              <p class="tx-section-label">隧道</p>
              <TunnelConfigForm
                workspaceId={workspaceId!}
                service="mcp"
                config={mcpTunnelForm}
                onSave={saveMcpTunnel}
              />
            </div>
            <div>
              <p class="tx-section-label">认证</p>
              <AuthConfigForm
                workspaceId={workspaceId!}
                auth={profile.auth}
                onSaveProfile={saveMcpAuth}
              />
            </div>
            <div>
              <p class="tx-section-label">策略</p>
              <RuntimePolicyForm
                toolProfile={profile.runtime.tool_profile}
                permissionMode={profile.runtime.permission_mode}
                onSave={saveMcpPolicy}
              />
            </div>
          </div>
        {:else if mcpSubTab === "logs"}
          <div class="mt-4">
            <LogViewer workspaceId={workspaceId!} service="mcp" />
          </div>
        {:else}
          <div class="mt-4">
            <HealthPanel workspaceId={workspaceId!} />
          </div>
        {/if}
      {:else}
        <div class="mt-4 flex flex-col gap-3">
          <ServicePanel
            title="Actions"
            subtitle="OpenAPI 网关 · ChatGPT Actions"
            status={actionsStatus}
            port={actions.local_port}
            portEditable={true}
            busy={actionsBusy}
            localEndpoint={actionsLocal || actionsLocalEndpoint(actions.local_port)}
            publicEndpoint={actionsPublic || actionsOpenApiUrl(profile, frpProfiles)}
            publicLabel="OpenAPI"
            onToggle={toggleActions}
            onPortChange={saveActionsPort}
          />
          <TunnelStrip
            workspaceId={workspaceId!}
            service="actions"
            tunnelType={actions.tunnel_type}
            publicUrl={actions.public_url}
            onPublicUrlChange={(url) => {
              const next = actionsConfig({ ...profile!, actions: { ...actions, public_url: url } });
              profile = { ...profile!, actions: next };
              actionsPublic = url ? `${url.replace(/\/$/, "")}/openapi.json` : "";
            }}
          />
        </div>

        <div class="mt-5">
          <Tabs
            items={subTabs}
            value={actionsSubTab}
            onchange={(v) => {
              actionsSubTab = v as SubTab;
            }}
          />
        </div>

        {#if actionsSubTab === "config"}
          <div class="tx-card mt-4 grid gap-6 p-5">
            <div>
              <p class="tx-section-label">隧道</p>
              <TunnelConfigForm
                workspaceId={workspaceId!}
                service="actions"
                config={actionsTunnelForm}
                onSave={saveActionsTunnel}
              />
            </div>
            <div>
              <p class="tx-section-label">认证</p>
              <ActionsAuthForm
                workspaceId={workspaceId!}
                authType={actions.auth_type}
                oauthClientId={actions.oauth_client_id ?? ""}
                oauthScopes={actions.oauth_scopes ?? ""}
                openapiUrl={actionsOpenApiUrl(profile, frpProfiles)}
                privacyUrl={actionsPrivacyUrl(profile, frpProfiles)}
                oauthAuthorizeUrl={actionsOAuthAuthorizeUrl(profile, frpProfiles)}
                oauthTokenUrl={actionsOAuthTokenUrl(profile, frpProfiles)}
                useSharedSecrets={actions.use_shared_secrets ?? false}
                onSave={saveActionsAuth}
              />
            </div>
            <div>
              <p class="tx-section-label">策略</p>
              <ActionsPolicyForm
                allowedCommands={actions.allowed_commands ?? ""}
                maxPatchBytes={actions.max_patch_bytes ?? 200_000}
                permissionMode={actions.permission_mode}
                onSave={saveActionsPolicy}
              />
            </div>
          </div>
        {:else if actionsSubTab === "logs"}
          <div class="mt-4">
            <LogViewer workspaceId={workspaceId!} service="actions" />
          </div>
        {:else}
          <div class="mt-4">
            <HealthPanel workspaceId={workspaceId!} />
          </div>
        {/if}
      {/if}
    </div>

    <footer class="border-t border-[var(--color-border)] px-8 py-4 text-xs text-[var(--color-text-muted)]">
      MCP 默认端口 28766，Actions 默认 8787，可同时运行。
    </footer>
  </section>
{/if}
