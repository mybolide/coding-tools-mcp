<script lang="ts">
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
  import ChatGptSessionPrompt from "$lib/components/ChatGptSessionPrompt.svelte";
  import ServicePanel from "$lib/components/ServicePanel.svelte";
  import GptQuickCopy from "$lib/components/GptQuickCopy.svelte";
  import StatusOrb from "$lib/components/StatusOrb.svelte";
  import Tabs from "$lib/components/Tabs.svelte";
  import TunnelConfigForm, {
    type TunnelFormConfig,
    type SaveTunnelOptions,
  } from "$lib/components/TunnelConfigForm.svelte";
  import WorkspaceMetaForm from "$lib/components/WorkspaceMetaForm.svelte";
  import { runHealthChecks, type HealthItem } from "$lib/api/health";
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
  import { restartTunnel, startTunnel, stopTunnel } from "$lib/api/tunnel";
  import { runServiceToggle, notifyStartFailure } from "$lib/runtime/service";
  import { showToast } from "$lib/stores/toast";
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
  let mcpStatusMessage = $state("");
  let actionsStatusMessage = $state("");
  let mcpBusy = $state(false);
  let actionsBusy = $state(false);
  let mcpLocal = $state("");
  let mcpPublic = $state("");
  let actionsLocal = $state("");
  let actionsPublic = $state("");
  let frpProfiles = $state<FrpProfileDto[]>([]);
  let statusRefreshing = $state(false);
  let reconnectBusy = $state(false);

  let activeService = $state<ServiceTab>("mcp");
  let mcpSubTab = $state<SubTab>("config");
  let actionsSubTab = $state<SubTab>("config");
  let loadGeneration = 0;

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
    use_proxy: profile?.tunnel.use_proxy ?? true,
  });

  const actionsTunnelForm = $derived<TunnelFormConfig>({
    type: actions?.tunnel_type ?? "none",
    public_url: actions?.public_url ?? "",
    frp_server: actions?.frp_server ?? "",
    frp_subdomain: actions?.frp_subdomain ?? "",
    frp_profile_id: actions?.frp_profile_id ?? "",
    frp_server_port: actions?.frp_server_port ?? 7000,
    cloudflare_mode: actions?.cloudflare_mode ?? "quick",
    use_proxy: actions?.use_proxy ?? true,
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

  function applyMcpRuntime(
    runtime: { state: RuntimeState; localEndpoint: string; publicEndpoint: string; localMessage?: string },
    id = workspaceId,
  ) {
    if (!id || id !== workspaceId) return;
    mcpStatus = runtime.state;
    mcpStatusMessage = runtime.localMessage ?? "";
    mcpLocal = runtime.localEndpoint;
    mcpPublic = runtime.publicEndpoint;
    mcpRuntimeStates.update((current) => ({ ...current, [id]: runtime.state }));
  }

  function applyActionsRuntime(runtime: {
    state: RuntimeState;
    localEndpoint: string;
    publicEndpoint: string;
    localMessage?: string;
  },
    id = workspaceId,
  ) {
    if (!id || id !== workspaceId) return;
    actionsStatus = runtime.state;
    actionsStatusMessage = runtime.localMessage ?? "";
    actionsLocal = runtime.localEndpoint;
    actionsPublic = runtime.publicEndpoint;
    actionsRuntimeStates.update((current) => ({ ...current, [id]: runtime.state }));
  }

  async function refreshRuntimeStatuses(id = workspaceId, force = false): Promise<void> {
    if (
      !id ||
      id !== workspaceId ||
      statusRefreshing ||
      (!force && (mcpBusy || actionsBusy || reconnectBusy))
    ) {
      return;
    }
    statusRefreshing = true;
    try {
      const [mcpRuntime, actionsRuntime] = await Promise.all([
        getRuntimeStatus(id),
        getActionsRuntimeStatus(id),
      ]);
      if (id !== workspaceId) return;
      applyMcpRuntime(mcpRuntime, id);
      applyActionsRuntime(actionsRuntime, id);
    } catch {
      // Polling is best-effort; keep the last visible state on transient IPC errors.
    } finally {
      statusRefreshing = false;
    }
  }

  async function load(id = workspaceId) {
    if (!id) return;
    const generation = ++loadGeneration;
    const items = await listWorkspaces();
    if (generation !== loadGeneration || id !== workspaceId) return;
    workspaces.set(items);
    frpProfiles = await listFrpProfiles();
    if (generation !== loadGeneration || id !== workspaceId) return;
    const nextProfile = items.find((item) => item.id === id) ?? null;
    if (generation !== loadGeneration || id !== workspaceId) return;
    profile = nextProfile;
    if (nextProfile) {
      await setLastWorkspace(nextProfile.id);
    }
    if (generation !== loadGeneration || id !== workspaceId) return;
    if (!nextProfile) {
      await goto("/");
      return;
    }

    const [mcpRuntime, actionsRuntime] = await Promise.all([
      getRuntimeStatus(id),
      getActionsRuntimeStatus(id),
    ]);
    if (generation !== loadGeneration || id !== workspaceId) return;
    applyMcpRuntime(mcpRuntime, id);
    applyActionsRuntime(actionsRuntime, id);
  }

  async function refreshProfile(id = workspaceId): Promise<WorkspaceProfile | null> {
    if (!id) return null;
    const items = await listWorkspaces();
    if (id !== workspaceId) return null;
    workspaces.set(items);
    const nextProfile = items.find((item) => item.id === id) ?? null;
    profile = nextProfile;
    return nextProfile;
  }

  function tunnelConfigured(type: string | undefined): boolean {
    return type === "cloudflare" || type === "frp";
  }

  async function afterServiceStart(
    service: "mcp" | "actions",
    runtime: { state: RuntimeState; publicEndpoint: string },
    id: string,
  ) {
    const nextProfile = await refreshProfile(id);
    if (id !== workspaceId) return;
    const tunnelType =
      service === "mcp"
        ? nextProfile?.tunnel.type
        : nextProfile
          ? actionsConfig(nextProfile).tunnel_type
          : undefined;
    if (runtime.state === "running" && tunnelConfigured(tunnelType) && !runtime.publicEndpoint) {
      showToast(
        "本地服务已启动，但隧道未能自动连接。请检查代理设置与隧道配置，或查看日志。",
        { title: "隧道未连接", kind: "warning", duration: 8000 },
      );
    }
  }

  async function toggleMcp() {
    const id = workspaceId;
    if (!id || mcpBusy) return;
    const wasRunning = mcpStatus === "running";
    mcpBusy = true;
    try {
      const runtime = await runServiceToggle(
        wasRunning,
        () => startRuntime(id),
        () => stopRuntime(id),
        "MCP",
      );
      if (runtime && id === workspaceId) {
        applyMcpRuntime(runtime, id);
        if (!wasRunning) {
          if (runtime.state === "running") {
            await afterServiceStart("mcp", runtime, id);
          } else {
            notifyStartFailure("MCP", runtime);
          }
        }
      }
    } finally {
      mcpBusy = false;
    }
  }

  async function toggleActions() {
    const id = workspaceId;
    if (!id || actionsBusy) return;
    const wasRunning = actionsStatus === "running";
    actionsBusy = true;
    try {
      const runtime = await runServiceToggle(
        wasRunning,
        () => startActionsRuntime(id),
        () => stopActionsRuntime(id),
        "Actions",
      );
      if (runtime && id === workspaceId) {
        applyActionsRuntime(runtime, id);
        if (!wasRunning) {
          if (runtime.state === "running") {
            await afterServiceStart("actions", runtime, id);
          } else {
            notifyStartFailure("Actions", runtime);
          }
        }
      }
    } finally {
      actionsBusy = false;
    }
  }

  function healthFailureNotice(items: HealthItem[]) {
    const labels = items.slice(0, 2).map((item) => item.label).join("、");
    const suffix = items.length > 2 ? ` 等 ${items.length} 项` : "";
    showToast(`${labels}${suffix}检查失败，请查看健康面板详情。`, {
      title: "连接仍有问题",
      kind: "warning",
      duration: 8000,
    });
  }

  async function reconnectAll() {
    const id = workspaceId;
    if (!id || reconnectBusy || mcpBusy || actionsBusy || statusRefreshing) return;
    if (mcpStatus === "starting" || mcpStatus === "stopping" || actionsStatus === "starting" || actionsStatus === "stopping") {
      showToast("服务正在切换状态，请稍后再重连。", { title: "请稍候", kind: "info" });
      return;
    }

    const mcpSelected = Boolean(profile) && (
      mcpStatus === "running" ||
      mcpStatus === "error" ||
      (profile?.runtime.auto_start ?? true)
    );
    const actionsSelected = Boolean(profile) && (
      actionsStatus === "running" ||
      actionsStatus === "error" ||
      (actions?.auto_start ?? true)
    );
    if (!mcpSelected && !actionsSelected) {
      showToast("没有启用自动启动的服务，也没有处于异常状态的服务。", { title: "无需重连", kind: "info" });
      return;
    }

    const confirmed = await confirm(
      "将重启运行中或异常的服务，并启动已启用自动启动的服务，然后重新连接已配置隧道。继续？",
      {
        title: "重新连接所有",
        kind: "warning",
        okLabel: "重新连接",
        cancelLabel: "取消",
      },
    );
    if (!confirmed || id !== workspaceId) return;

    reconnectBusy = true;
    const failures: string[] = [];
    let reconnected = 0;

    const reconnectService = async (
      label: string,
      service: "mcp" | "actions",
      wasRunning: boolean,
      tunnelType: string,
      restartRuntimeForService: (workspaceId: string) => Promise<{
        state: RuntimeState;
        localEndpoint: string;
        publicEndpoint: string;
        localMessage?: string;
      }>,
      startRuntimeForService: (workspaceId: string) => Promise<{
        state: RuntimeState;
        localEndpoint: string;
        publicEndpoint: string;
        localMessage?: string;
      }>,
      applyRuntime: (runtime: {
        state: RuntimeState;
        localEndpoint: string;
        publicEndpoint: string;
        localMessage?: string;
      }) => void,
      setPublicEndpoint: (url: string) => void,
    ) => {
      try {
        const runtime = wasRunning
          ? await restartRuntimeForService(id)
          : await startRuntimeForService(id);
        if (id !== workspaceId) return;
        applyRuntime(runtime);
        if (runtime.state !== "running") {
          failures.push(`${label} 未能恢复：${runtime.localMessage || "服务未运行"}`);
          return;
        }

        if (tunnelConfigured(tunnelType)) {
          let tunnel = await restartTunnel(id, service);
          if (tunnel.state !== "running") {
            tunnel = await startTunnel(id, service);
          }
          if (tunnel.state !== "running") {
            throw new Error(tunnel.state || "隧道未进入运行状态");
          }
          if (tunnel.publicUrl) {
            setPublicEndpoint(`${tunnel.publicUrl.replace(/\/$/, "")}/${service === "mcp" ? "mcp" : "openapi.json"}`);
          }
        }
        reconnected += 1;
      } catch (error) {
        failures.push(`${label}：${String(error)}`);
      }
    };

    try {
      if (mcpSelected && profile) {
        await reconnectService(
          "MCP",
          "mcp",
          mcpStatus === "running",
          profile.tunnel.type,
          restartRuntime,
          startRuntime,
          (runtime) => applyMcpRuntime(runtime, id),
          (url) => (mcpPublic = url),
        );
      }
      if (actionsSelected && profile) {
        await reconnectService(
          "Actions",
          "actions",
          actionsStatus === "running",
          actionsConfig(profile).tunnel_type,
          restartActionsRuntime,
          startActionsRuntime,
          (runtime) => applyActionsRuntime(runtime, id),
          (url) => (actionsPublic = url),
        );
      }

      await refreshRuntimeStatuses(id, true);
      try {
        const health = await runHealthChecks(id);
        const failedHealth = health.filter((item) => !item.ok);
        if (failedHealth.length > 0) {
          failures.push(`健康检查有 ${failedHealth.length} 项失败`);
        }
      } catch (error) {
        failures.push(`健康检查：${String(error)}`);
      }

      if (failures.length > 0) {
        showToast(failures.slice(0, 3).join("；"), {
          title: reconnected > 0 ? "已部分重连" : "重连失败",
          kind: reconnected > 0 ? "warning" : "error",
          duration: 10_000,
        });
      } else {
        showToast(`已重新连接 ${reconnected} 个服务及其隧道。`, {
          title: "重连完成",
          kind: "success",
          duration: 6000,
        });
      }
    } finally {
      reconnectBusy = false;
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

  async function restartTunnelIfConfigured(
    targetWorkspaceId: string,
    config: TunnelFormConfig,
    service: "mcp" | "actions",
  ) {
    if (config.type === "none") {
      await stopTunnel(targetWorkspaceId, service);
      return;
    }
    const status = await restartTunnel(targetWorkspaceId, service);
    if (workspaceId !== targetWorkspaceId) return;
    if (status.publicUrl) {
      if (service === "mcp") {
        mcpPublic = `${status.publicUrl.replace(/\/$/, "")}/mcp`;
      } else {
        actionsPublic = `${status.publicUrl.replace(/\/$/, "")}/openapi.json`;
      }
    }
  }

  async function saveMcpTunnel(config: TunnelFormConfig, options?: SaveTunnelOptions) {
    if (!profile) return;
    const targetWorkspaceId = workspaceId;
    if (!targetWorkspaceId) return;
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
        use_proxy: config.use_proxy,
      },
    };
    await updateWorkspace(next);
    if (!options?.skipTunnelRestart) {
      await restartTunnelIfConfigured(targetWorkspaceId, config, "mcp");
    }
    if (workspaceId !== targetWorkspaceId) return;
    profile = next;
    mcpPublic = publicEndpointFromTunnel(config, "/mcp");
    if (!options?.skipTunnelRestart && !options?.skipServicePrompt) {
      await load();
      if (workspaceId !== targetWorkspaceId) return;
    }
    if (!options?.skipServicePrompt) {
      await promptServiceRestart(mcpStatus === "running", "MCP 服务");
    }
  }

  async function saveActionsTunnel(config: TunnelFormConfig, options?: SaveTunnelOptions) {
    if (!profile) return;
    const targetWorkspaceId = workspaceId;
    if (!targetWorkspaceId) return;
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
        use_proxy: config.use_proxy,
      },
    };
    await updateWorkspace(next);
    if (!options?.skipTunnelRestart) {
      await restartTunnelIfConfigured(targetWorkspaceId, config, "actions");
    }
    if (workspaceId !== targetWorkspaceId) return;
    profile = next;
    actionsPublic = publicEndpointFromTunnel(config, "/openapi.json");
    if (!options?.skipTunnelRestart && !options?.skipServicePrompt) {
      await load();
      if (workspaceId !== targetWorkspaceId) return;
    }
    if (!options?.skipServicePrompt) {
      await promptServiceRestart(actionsStatus === "running", "Actions 服务");
    }
  }

  async function saveMcpPolicy(draft: RuntimePolicyDraft) {
    if (!profile) return;
    const next: WorkspaceProfile = {
      ...profile,
      runtime: {
        ...profile.runtime,
        tool_profile: draft.toolProfile,
        permission_mode: draft.permissionMode,
        auto_start: draft.autoStart,
        auto_recover: draft.autoRecover,
        allowed_commands: draft.allowedCommands,
        workspace_local_entries: draft.workspaceLocalEntries,
        workspace_script_extensions: draft.workspaceScriptExtensions,
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
        auto_start: draft.autoStart,
        auto_recover: draft.autoRecover,
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

  async function saveWorkspacePath(path: string) {
    if (!profile || profile.path === path) return;
    const next: WorkspaceProfile = { ...profile, path };
    await updateWorkspace(next);
    profile = next;
    showToast("工作区目录已更新", { kind: "success" });
    await promptServiceRestart(mcpStatus === "running", "MCP 服务");
    await promptServiceRestart(actionsStatus === "running", "Actions 服务");
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

  $effect(() => {
    const id = workspaceId;
    if (!id) return;
    profile = null;
    void load(id);
    const timer = window.setInterval(() => void refreshRuntimeStatuses(id), 5_000);

    return () => {
      loadGeneration += 1;
      window.clearInterval(timer);
    };
  });
</script>

{#if profile && actions}
  <section class="page-scroll">
    <header class="page-header">
      <div class="flex items-start justify-between gap-4">
        <div>
          <p class="page-kicker">工作区</p>
          <h2 class="page-title">{profile.name}</h2>
        </div>
        <div class="flex shrink-0 flex-wrap justify-end gap-2">
          <button
            type="button"
            class="tx-btn-ghost"
            disabled={statusRefreshing || reconnectBusy}
            onclick={() => void refreshRuntimeStatuses()}
          >
            {statusRefreshing ? "刷新中…" : "刷新状态"}
          </button>
          <button
            type="button"
            class="tx-btn-primary"
            disabled={reconnectBusy || mcpBusy || actionsBusy || statusRefreshing}
            onclick={() => void reconnectAll()}
          >
            {reconnectBusy ? "重连中…" : "重新连接所有"}
          </button>
          <button
            type="button"
            class="tx-btn-ghost text-[var(--danger)]"
            onclick={() => void removeWorkspace()}
          >
            删除工作区
          </button>
        </div>
      </div>

      <div class="mt-4">
        <WorkspaceMetaForm
          name={profile.name}
          path={profile.path}
          onSave={saveWorkspaceName}
          onUpdatePath={saveWorkspacePath}
        />
      </div>

      <div class="mt-4">
        <ChatGptSessionPrompt />
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
            statusMessage={mcpStatusMessage}
            port={profile.runtime.local_port}
            portEditable={true}
            busy={mcpBusy}
            tunnelType={profile.tunnel.type}
            localEndpoint={mcpLocal || mcpLocalEndpoint(profile.runtime.local_port)}
            publicEndpoint={mcpPublic}
            publicLabel="公网 MCP"
            onToggle={toggleMcp}
            onPortChange={saveMcpPort}
          />
          <GptQuickCopy
            workspaceId={workspaceId!}
            service="mcp"
            {profile}
            publicMcpEndpoint={mcpPublic}
            {frpProfiles}
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
                autoStart={profile.runtime.auto_start ?? true}
                autoRecover={profile.runtime.auto_recover ?? true}
                allowedCommands={profile.runtime.allowed_commands ?? ""}
                workspaceLocalEntries={profile.runtime.workspace_local_entries ?? true}
                workspaceScriptExtensions={profile.runtime.workspace_script_extensions ?? ".exe,.bat,.cmd,.ps1"}
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
            <HealthPanel workspaceId={workspaceId!} onFailure={healthFailureNotice} />
          </div>
        {/if}
      {:else}
        <div class="mt-4 flex flex-col gap-3">
          <ServicePanel
            title="Actions"
            subtitle="OpenAPI 网关 · ChatGPT Actions"
            status={actionsStatus}
            statusMessage={actionsStatusMessage}
            port={actions.local_port}
            portEditable={true}
            busy={actionsBusy}
            tunnelType={actions.tunnel_type}
            localEndpoint={actionsLocal || actionsLocalEndpoint(actions.local_port)}
            publicEndpoint={actionsPublic || actionsOpenApiUrl(profile, frpProfiles)}
            publicLabel="OpenAPI"
            onToggle={toggleActions}
            onPortChange={saveActionsPort}
          />
          <GptQuickCopy
            workspaceId={workspaceId!}
            service="actions"
            {profile}
            {frpProfiles}
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
                autoStart={actions.auto_start ?? true}
                autoRecover={actions.auto_recover ?? true}
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
            <HealthPanel workspaceId={workspaceId!} onFailure={healthFailureNotice} />
          </div>
        {/if}
      {/if}
    </div>

    <footer class="border-t border-[var(--color-border)] px-8 py-4 text-xs text-[var(--color-text-muted)]">
      MCP 默认端口 28766，Actions 默认 8787，可同时运行。
    </footer>
  </section>
{/if}
