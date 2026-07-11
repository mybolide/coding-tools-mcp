<script lang="ts">
  import CopyButton from "$lib/components/CopyButton.svelte";
  import { getSecret, regenerateSecret } from "$lib/api/secrets";
  import type { ActionsAuthDraft } from "$lib/types";

  export const ACTIONS_AUTH_OPTIONS = [
    { value: "api_key", label: "API Key / Bearer" },
    { value: "none", label: "不启用认证" },
    { value: "oauth", label: "OAuth" },
  ] as const;

  export type { ActionsAuthDraft } from "$lib/types";

  interface Props {
    workspaceId: string;
    authType: string;
    oauthClientId: string;
    oauthScopes: string;
    openapiUrl: string;
    privacyUrl: string;
    oauthAuthorizeUrl: string;
    oauthTokenUrl: string;
    onSave: (draft: ActionsAuthDraft) => void | Promise<void>;
  }

  let {
    workspaceId,
    authType,
    oauthClientId,
    oauthScopes,
    openapiUrl,
    privacyUrl,
    oauthAuthorizeUrl,
    oauthTokenUrl,
    onSave,
  }: Props = $props();

  let draftAuthType = $state("api_key");
  let draftOauthClientId = $state("");
  let draftOauthScopes = $state("");
  let apiKey = $state("");
  let oauthClientSecret = $state("");
  let oauthPassword = $state("");
  let oauthTokenSecret = $state("");
  let loadingKey = $state(true);
  let loadingOAuthSecret = $state(true);
  let loadingOAuthPassword = $state(true);
  let loadingOAuthTokenSecret = $state(true);
  let regenerating = $state(false);
  let regeneratingOAuthSecret = $state(false);
  let regeneratingOAuthPassword = $state(false);
  let regeneratingOAuthTokenSecret = $state(false);
  let saving = $state(false);

  const dirty = $derived(
    draftAuthType !== authType ||
      draftOauthClientId !== oauthClientId ||
      draftOauthScopes !== oauthScopes,
  );
  const showApiKey = $derived(draftAuthType === "api_key");
  const showOAuth = $derived(draftAuthType === "oauth");

  $effect(() => {
    draftAuthType = authType;
    draftOauthClientId = oauthClientId;
    draftOauthScopes = oauthScopes;
  });

  $effect(() => {
    workspaceId;
    void loadSecrets();
  });

  async function loadSecrets() {
    loadingKey = true;
    loadingOAuthSecret = true;
    loadingOAuthPassword = true;
    loadingOAuthTokenSecret = true;
    try {
      const [key, secret, password, tokenSecret] = await Promise.all([
        getSecret(workspaceId, "actions_api_key"),
        getSecret(workspaceId, "actions_oauth_client_secret"),
        getSecret(workspaceId, "actions_oauth_password"),
        getSecret(workspaceId, "actions_oauth_token_secret"),
      ]);
      apiKey = key ?? "";
      oauthClientSecret = secret ?? "";
      oauthPassword = password ?? "";
      oauthTokenSecret = tokenSecret ?? "";
    } finally {
      loadingKey = false;
      loadingOAuthSecret = false;
      loadingOAuthPassword = false;
      loadingOAuthTokenSecret = false;
    }
  }

  async function save() {
    if (saving || !dirty) return;
    saving = true;
    try {
      await onSave({
        authType: draftAuthType,
        oauthClientId: draftOauthClientId.trim(),
        oauthScopes: draftOauthScopes.trim(),
      });
    } finally {
      saving = false;
    }
  }

  async function regenerate() {
    if (regenerating) return;
    regenerating = true;
    try {
      apiKey = await regenerateSecret(workspaceId, "actions_api_key");
    } finally {
      regenerating = false;
    }
  }

  async function regenerateOAuthSecret() {
    if (regeneratingOAuthSecret) return;
    regeneratingOAuthSecret = true;
    try {
      oauthClientSecret = await regenerateSecret(workspaceId, "actions_oauth_client_secret");
    } finally {
      regeneratingOAuthSecret = false;
    }
  }

  async function regenerateOAuthPassword() {
    if (regeneratingOAuthPassword) return;
    regeneratingOAuthPassword = true;
    try {
      oauthPassword = await regenerateSecret(workspaceId, "actions_oauth_password");
    } finally {
      regeneratingOAuthPassword = false;
    }
  }

  async function regenerateOAuthTokenSecret() {
    if (regeneratingOAuthTokenSecret) return;
    regeneratingOAuthTokenSecret = true;
    try {
      oauthTokenSecret = await regenerateSecret(workspaceId, "actions_oauth_token_secret");
    } finally {
      regeneratingOAuthTokenSecret = false;
    }
  }
</script>

<form
  class="grid gap-3"
  onsubmit={(event) => {
    event.preventDefault();
    void save();
  }}
>
  <div class="grid gap-2 rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] p-3">
    <p class="text-xs font-medium text-[var(--color-text-secondary)]">GPT Actions 接入</p>
    <label class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">OpenAPI Schema URL</span>
      <div class="flex gap-2">
        <input
          type="text"
          readonly
          class="min-w-0 flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-xs"
          value={openapiUrl || "配置隧道后显示公网地址，否则用本地地址"}
        />
        {#if openapiUrl}
          <CopyButton value={openapiUrl} label="复制" />
        {/if}
      </div>
    </label>
    <label class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">隐私政策 URL</span>
      <div class="flex gap-2">
        <input
          type="text"
          readonly
          class="min-w-0 flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-xs"
          value={privacyUrl || "同上"}
        />
        {#if privacyUrl}
          <CopyButton value={privacyUrl} label="复制" />
        {/if}
      </div>
    </label>
    <p class="text-xs text-[var(--color-text-muted)]">
      在 GPT 编辑器 → Actions → Import from URL，粘贴 OpenAPI 地址；隐私政策填上方 URL。GPT 与 Apps
      不能同时使用，请选 Actions。
    </p>
  </div>

  <label class="grid gap-1">
    <span class="text-xs text-[var(--color-text-muted)]">认证方式</span>
    <select
      class="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 text-sm"
      bind:value={draftAuthType}
    >
      {#each ACTIONS_AUTH_OPTIONS as option}
        <option value={option.value}>{option.label}</option>
      {/each}
    </select>
  </label>

  {#if showApiKey}
    <label class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">API Key（Bearer）</span>
      <div class="flex gap-2">
        <input
          type="text"
          readonly
          class="min-w-0 flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-sm"
          value={loadingKey ? "加载中…" : apiKey}
        />
        {#if apiKey}
          <CopyButton value={apiKey} label="复制" />
        {/if}
        <button
          type="button"
          class="shrink-0 rounded-md border border-[var(--color-border)] px-2.5 py-1 text-xs text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-surface-hover)] disabled:opacity-50"
          disabled={regenerating || loadingKey}
          onclick={() => void regenerate()}
        >
          {regenerating ? "生成中…" : "重新生成"}
        </button>
      </div>
    </label>
    <p class="text-xs text-[var(--color-text-muted)]">
      在 GPT Actions 认证里选 API Key → Bearer，Key 填这里的值。
    </p>
  {:else if showOAuth}
    <label class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">OAuth Client ID（填到 GPT）</span>
      <div class="flex gap-2">
        <input
          type="text"
          class="min-w-0 flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-sm"
          bind:value={draftOauthClientId}
        />
        {#if draftOauthClientId}
          <CopyButton value={draftOauthClientId} label="复制" />
        {/if}
      </div>
    </label>
    <label class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">OAuth Client Secret（填到 GPT）</span>
      <div class="flex gap-2">
        <input
          type="text"
          readonly
          class="min-w-0 flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-sm"
          value={loadingOAuthSecret ? "加载中…" : oauthClientSecret}
        />
        {#if oauthClientSecret}
          <CopyButton value={oauthClientSecret} label="复制" />
        {/if}
        <button
          type="button"
          class="shrink-0 rounded-md border border-[var(--color-border)] px-2.5 py-1 text-xs text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-surface-hover)] disabled:opacity-50"
          disabled={regeneratingOAuthSecret || loadingOAuthSecret}
          onclick={() => void regenerateOAuthSecret()}
        >
          {regeneratingOAuthSecret ? "生成中…" : "重新生成"}
        </button>
      </div>
    </label>
    <label class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">OAuth Password（服务端校验）</span>
      <div class="flex gap-2">
        <input
          type="text"
          readonly
          class="min-w-0 flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-sm"
          value={loadingOAuthPassword ? "加载中…" : oauthPassword}
        />
        {#if oauthPassword}
          <CopyButton value={oauthPassword} label="复制" />
        {/if}
        <button
          type="button"
          class="shrink-0 rounded-md border border-[var(--color-border)] px-2.5 py-1 text-xs"
          disabled={regeneratingOAuthPassword || loadingOAuthPassword}
          onclick={() => void regenerateOAuthPassword()}
        >
          {regeneratingOAuthPassword ? "生成中…" : "重新生成"}
        </button>
      </div>
    </label>
    <label class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">OAuth Token Secret（JWT 签名）</span>
      <div class="flex gap-2">
        <input
          type="text"
          readonly
          class="min-w-0 flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-sm"
          value={loadingOAuthTokenSecret ? "加载中…" : oauthTokenSecret}
        />
        {#if oauthTokenSecret}
          <CopyButton value={oauthTokenSecret} label="复制" />
        {/if}
        <button
          type="button"
          class="shrink-0 rounded-md border border-[var(--color-border)] px-2.5 py-1 text-xs"
          disabled={regeneratingOAuthTokenSecret || loadingOAuthTokenSecret}
          onclick={() => void regenerateOAuthTokenSecret()}
        >
          {regeneratingOAuthTokenSecret ? "生成中…" : "重新生成"}
        </button>
      </div>
    </label>
    <label class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">Authorization URL（填到 GPT）</span>
      <div class="flex gap-2">
        <input
          type="text"
          readonly
          class="min-w-0 flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-xs"
          value={oauthAuthorizeUrl}
        />
        {#if oauthAuthorizeUrl}
          <CopyButton value={oauthAuthorizeUrl} label="复制" />
        {/if}
      </div>
    </label>
    <label class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">Token URL（填到 GPT）</span>
      <div class="flex gap-2">
        <input
          type="text"
          readonly
          class="min-w-0 flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 font-mono text-xs"
          value={oauthTokenUrl}
        />
        {#if oauthTokenUrl}
          <CopyButton value={oauthTokenUrl} label="复制" />
        {/if}
      </div>
    </label>
    <label class="grid gap-1">
      <span class="text-xs text-[var(--color-text-muted)]">Scope（填到 GPT，空格分隔）</span>
      <input
        type="text"
        class="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 text-sm"
        placeholder="例如：coding-tools"
        bind:value={draftOauthScopes}
      />
    </label>
    <p class="text-xs text-[var(--color-text-muted)]">
      GPT 编辑器会生成 Callback URL（<code>https://chatgpt.com/aip/g-…/oauth/callback</code>），无需在本应用配置。Token
      交换方式选默认即可。
    </p>
  {:else}
    <p class="text-xs text-[var(--color-text-muted)]">
      不校验请求认证；GPT 侧选 None。仅建议本机调试，公网暴露请用 API Key 或 OAuth。
    </p>
  {/if}

  <div class="flex justify-end pt-1">
    <button
      type="submit"
      class="rounded-md bg-[var(--color-accent)] px-3 py-1.5 text-sm font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
      disabled={saving || !dirty}
    >
      {saving ? "保存中…" : "保存配置"}
    </button>
  </div>
</form>
