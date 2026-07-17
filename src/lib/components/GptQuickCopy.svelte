<script lang="ts">
  import ChatGptSessionPrompt from "$lib/components/ChatGptSessionPrompt.svelte";
  import CopyFieldRow from "$lib/components/CopyFieldRow.svelte";
  import { getSecret, getSharedSecret } from "$lib/api/secrets";
  import type { AuthConfig, WorkspaceProfile } from "$lib/types";
  import {
    actionsOAuthAuthorizeUrl,
    actionsOAuthTokenUrl,
    actionsOpenApiUrl,
    actionsPrivacyUrl,
    actionsConfig,
  } from "$lib/types";

  interface Props {
    workspaceId: string;
    service: "mcp" | "actions";
    profile: WorkspaceProfile;
    publicMcpEndpoint?: string;
    frpProfiles?: { id: string; name: string; server: string; serverPort: number }[];
  }

  let { workspaceId, service, profile, publicMcpEndpoint = "", frpProfiles = [] }: Props = $props();

  let loading = $state(true);
  let secrets = $state<Record<string, string>>({});

  const actions = $derived(actionsConfig(profile));
  const auth = $derived(profile.auth);

  async function loadSecrets() {
    loading = true;
    try {
      if (service === "mcp") {
        const useShared = auth.use_shared_secrets ?? false;
        const fetchSecret = async (key: string, sharedKey: string) => {
          const value = useShared
            ? await getSharedSecret(sharedKey as Parameters<typeof getSharedSecret>[0])
            : await getSecret(workspaceId, key as Parameters<typeof getSecret>[1]);
          return value ?? "";
        };
        if (auth.type === "oauth") {
          const clientId = useShared
            ? ((await getSharedSecret("oauth_client_id")) ?? "")
            : auth.oauth_client_id;
          secrets = {
            oauth_client_id: clientId,
            oauth_client_secret: await fetchSecret("oauth_client_secret", "oauth_client_secret"),
            oauth_password: await fetchSecret("oauth_password", "oauth_password"),
          };
        } else if (auth.type === "bearer") {
          secrets = {
            bearer_token: await fetchSecret("bearer_token", "bearer_token"),
          };
        } else {
          secrets = {};
        }
      } else {
        const useShared = actions.use_shared_secrets ?? false;
        const fetchSecret = async (key: string, sharedKey: string) => {
          const value = useShared
            ? await getSharedSecret(sharedKey as Parameters<typeof getSharedSecret>[0])
            : await getSecret(workspaceId, key as Parameters<typeof getSecret>[1]);
          return value ?? "";
        };
        if (actions.auth_type === "api_key") {
          secrets = { actions_api_key: await fetchSecret("actions_api_key", "actions_api_key") };
        } else if (actions.auth_type === "oauth") {
          secrets = {
            actions_oauth_client_secret: await fetchSecret(
              "actions_oauth_client_secret",
              "actions_oauth_client_secret",
            ),
          };
        } else {
          secrets = {};
        }
      }
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    workspaceId;
    service;
    auth.type;
    auth.oauth_client_id;
    auth.use_shared_secrets;
    actions.auth_type;
    actions.oauth_client_id;
    actions.oauth_scopes;
    actions.use_shared_secrets;
    void loadSecrets();
  });
</script>

<article class="tx-card p-5">
  <div class="mb-4">
    <p class="tx-section-label">GPT 配置</p>
    <p class="mt-1 text-xs text-[var(--color-text-muted)]">
      {service === "mcp"
        ? "复制以下内容到 ChatGPT → 设置 → 连接器 / MCP"
        : "复制以下内容到 GPT 编辑器 → Actions"}
    </p>
  </div>

  <div class="grid gap-3">
    {#if service === "mcp"}
      <CopyFieldRow
        label="公网 MCP 地址"
        value={publicMcpEndpoint}
        hint="GPT 连接器里填这个 URL"
      />
      {#if auth.type === "oauth"}
        <CopyFieldRow label="OAuth Client ID" value={secrets.oauth_client_id ?? auth.oauth_client_id} {loading} />
        <CopyFieldRow
          label="OAuth Client Secret"
          value={secrets.oauth_client_secret ?? ""}
          {loading}
        />
        <CopyFieldRow
          label="授权口令"
          value={secrets.oauth_password ?? ""}
          hint="ChatGPT 首次授权时输入"
          {loading}
        />
      {:else if auth.type === "bearer"}
        <CopyFieldRow label="Bearer Token" value={secrets.bearer_token ?? ""} {loading} />
      {:else}
        <p class="text-xs text-[var(--color-text-muted)]">当前未启用认证，仅本机调试可用。</p>
      {/if}
    {:else}
      <CopyFieldRow
        label="OpenAPI Schema URL"
        value={actionsOpenApiUrl(profile, frpProfiles)}
        hint="Actions → Import from URL"
      />
      <CopyFieldRow
        label="隐私政策 URL"
        value={actionsPrivacyUrl(profile, frpProfiles)}
        hint="GPT Actions 隐私政策字段"
      />
      {#if actions.auth_type === "api_key"}
        <CopyFieldRow
          label="API Key（Bearer）"
          value={secrets.actions_api_key ?? ""}
          hint="Actions 认证选 API Key → Bearer"
          {loading}
        />
      {:else if actions.auth_type === "oauth"}
        <CopyFieldRow label="OAuth Client ID" value={actions.oauth_client_id ?? ""} />
        <CopyFieldRow
          label="OAuth Client Secret"
          value={secrets.actions_oauth_client_secret ?? ""}
          {loading}
        />
        <CopyFieldRow
          label="Authorization URL"
          value={actionsOAuthAuthorizeUrl(profile, frpProfiles)}
        />
        <CopyFieldRow label="Token URL" value={actionsOAuthTokenUrl(profile, frpProfiles)} />
        <CopyFieldRow label="Scope" value={actions.oauth_scopes ?? ""} hint="空格分隔" />
      {:else}
        <p class="text-xs text-[var(--color-text-muted)]">当前未启用认证，公网暴露请改用 API Key 或 OAuth。</p>
      {/if}
    {/if}
  </div>

  {#if service === "mcp"}
    <div class="mt-4 border-t border-[var(--color-border)] pt-4">
      <ChatGptSessionPrompt />
    </div>
  {/if}
</article>
